use md5::{Digest, Md5};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::data::{AppEntry, Platform};
const APP_ROOTS: &[&str] = &["ux0:app", "ur0:app", "uma0:app"];
const PSP_ROOTS: &[&str] = &["ux0:pspemu/PSP/GAME", "ur0:pspemu/PSP/GAME"];
const META_ROOTS: &[&str] = &["ux0:appmeta", "ur0:appmeta"];
const WORK_DIR: &str = "ux0:data/vitaforge";
const HASH_CACHE_DIR: &str = "ux0:data/vitaforge/hashes";
const SCAN_LOG: &str = "ux0:data/vitaforge/scan.log";
const HASH_LEN: usize = 32;
const RESCAN_INTERVAL: Duration = Duration::from_secs(60);
const INSTALLED_CACHE_FILE: &str = "ux0:data/vitaforge/installed_cache.json";
const HASH_CHUNK: usize = 64 * 1024;
const HASH_BATCH: usize = 8;
const HASH_GAP: Duration = Duration::from_millis(15);
const FIRST_SCAN_HASH_GAP: Duration = Duration::from_millis(40);
struct Wanted {
    key: String,
    folder: String,
    psp: bool,
    hash: String,
    hash2: String,
    catalog_version: String,
}
impl Wanted {
    fn from_entry(entry: &AppEntry) -> Option<Self> {
        let psp = matches!(entry.platform, Platform::Psp | Platform::NpsPsp | Platform::NpsPsx);
        let folder = if psp { entry.id.trim().to_owned() } else { key_of(&entry.titleid) };
        if folder.is_empty() {
            return None;
        }
        Some(Self {
            key: index_key(entry),
            folder,
            psp,
            hash: entry.hash.clone(),
            hash2: entry.hash2.clone(),
            catalog_version: entry.version.clone(),
        })
    }
    fn roots(&self) -> &'static [&'static str] {
        if self.psp { PSP_ROOTS } else { APP_ROOTS }
    }
    fn markers(&self) -> &'static [&'static str] {
        if self.psp {
            &["hash.vdb", "EBOOT.PBP", "eboot.pbp"]
        } else {
            &["hash.vdb", "eboot.bin", "sce_sys/param.sfo"]
        }
    }
}
pub fn index_key(entry: &AppEntry) -> String {
    if matches!(entry.platform, Platform::Psp | Platform::NpsPsp | Platform::NpsPsx) {
        format!("PSP-{}", entry.id.trim())
    } else {
        key_of(&entry.titleid)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    Absent,
    Outdated,
    Installed,
}
#[derive(Debug, Clone, Default)]
pub struct InstalledVersionInfo {
    pub app_ver: Option<String>,
    pub installed_at: Option<String>, // "YYYY-MM-DD", formatted here so the UI stays date-library-free
}
fn installed_at(titleid: &str) -> Option<String> {
    let modified = std::fs::metadata(hash_file_path(titleid)).ok()?.modified().ok()?;
    let secs = modified.duration_since(std::time::SystemTime::UNIX_EPOCH).ok()?.as_secs();
    Some(format_ymd(secs))
}
fn format_ymd(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
#[derive(Clone)]
pub struct InstalledIndex {
    states: Arc<Mutex<HashMap<String, InstallState>>>,
    scanned_at: Arc<Mutex<Option<Instant>>>,
    scanning: Arc<AtomicBool>,
    summary: Arc<Mutex<String>>,
    version_cache: Arc<Mutex<HashMap<String, Option<String>>>>,
    counts: Arc<Mutex<(usize, usize)>>,
}
fn count_states(states: &HashMap<String, InstallState>) -> (usize, usize) {
    let mut installed = 0;
    let mut outdated = 0;
    for state in states.values() {
        match state {
            InstallState::Installed => installed += 1,
            InstallState::Outdated => outdated += 1,
            InstallState::Absent => {}
        }
    }
    (installed + outdated, outdated)
}
fn load_installed_cache() -> HashMap<String, InstallState> {
    let mut map = HashMap::new();
    let Ok(content) = std::fs::read_to_string(INSTALLED_CACHE_FILE) else {
        return map;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let state = match val {
                "installed" => InstallState::Installed,
                "outdated" => InstallState::Outdated,
                _ => continue,
            };
            map.insert(key.to_string(), state);
        }
    }
    map
}
fn save_installed_cache(states: &HashMap<String, InstallState>) {
    let _ = std::fs::create_dir_all(WORK_DIR);
    let mut lines = Vec::new();
    for (k, v) in states {
        let val_str = match v {
            InstallState::Installed => "installed",
            InstallState::Outdated => "outdated",
            InstallState::Absent => continue,
        };
        lines.push(format!("{k}={val_str}"));
    }
    let _ = std::fs::write(INSTALLED_CACHE_FILE, lines.join("\n"));
}
impl InstalledIndex {
    pub fn new() -> Self {
        let cached_states = load_installed_cache();
        let summary_text = if cached_states.is_empty() {
            concat!("b", env!("BUILD_STAMP"), " · scanning…").to_owned()
        } else {
            format!("b{} · {} ready", env!("BUILD_STAMP"), cached_states.len())
        };
        let counts = count_states(&cached_states);
        Self {
            states: Arc::new(Mutex::new(cached_states)),
            scanned_at: Arc::new(Mutex::new(None)),
            scanning: Arc::new(AtomicBool::new(false)),
            summary: Arc::new(Mutex::new(summary_text)),
            version_cache: Arc::new(Mutex::new(HashMap::new())),
            counts: Arc::new(Mutex::new(counts)),
        }
    }
    pub fn summary(&self) -> String {
        self.summary.lock().unwrap().clone()
    }
    pub fn counts(&self) -> (usize, usize) {
        *self.counts.lock().unwrap()
    }
    pub fn state(&self, entry: &AppEntry) -> InstallState {
        self.state_by_key(&index_key(entry))
    }
    pub fn state_by_key(&self, key: &str) -> InstallState {
        if key.is_empty() {
            return InstallState::Absent;
        }
        self.states.lock().unwrap().get(key).copied().unwrap_or(InstallState::Absent)
    }
    pub fn installed_info(&self, ctx: &egui::Context, entry: &AppEntry) -> Option<InstalledVersionInfo> {
        let key = index_key(entry);
        if key.is_empty() || self.state_by_key(&key) == InstallState::Absent {
            return None;
        }
        if let Some(app_ver) = self.version_cache.lock().unwrap().get(&key) {
            return Some(InstalledVersionInfo { app_ver: app_ver.clone(), installed_at: installed_at(&key) });
        }
        self.version_cache.lock().unwrap().insert(key.clone(), None); // placeholder: lookup in flight
        let psp = matches!(entry.platform, Platform::Psp | Platform::NpsPsp | Platform::NpsPsx);
        let folder = if psp { entry.id.trim().to_owned() } else { key_of(&entry.titleid) };
        let index = self.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let roots: &[&str] = if psp { PSP_ROOTS } else { APP_ROOTS };
            let app_ver = roots.iter().find_map(|root| {
                super::sfo::read_app_ver(&PathBuf::from(format!("{root}/{folder}/sce_sys/param.sfo")))
            });
            index.version_cache.lock().unwrap().insert(key, app_ver);
            ctx.request_repaint();
        });
        Some(InstalledVersionInfo { app_ver: None, installed_at: installed_at(&index_key(entry)) })
    }
    pub fn force_refresh(&self, ctx: &egui::Context, entries: &[AppEntry]) {
        *self.scanned_at.lock().unwrap() = None;
        self.refresh(ctx, entries);
    }
    pub fn refresh(&self, ctx: &egui::Context, entries: &[AppEntry]) -> bool {
        if entries.is_empty() {
            return false;
        }
        if self.scanning.load(Ordering::Acquire) {
            return true;
        }
        {
            let scanned_at = self.scanned_at.lock().unwrap();
            if scanned_at.is_some_and(|at| at.elapsed() < RESCAN_INTERVAL) {
                return true;
            }
        }
        self.scanning.store(true, Ordering::Release);
        let wanted: Vec<Wanted> = entries.iter().filter_map(Wanted::from_entry).collect();
        let hash_gap = if self.states.lock().unwrap().is_empty() { FIRST_SCAN_HASH_GAP } else { HASH_GAP };
        let index = self.clone();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("installed-scan".to_owned())
            .stack_size(512 * 1024)
            .spawn(move || {
                scan(
                    &wanted,
                    hash_gap,
                    |found, authoritative| {
                        index.publish(found, authoritative);
                        ctx.request_repaint();
                    },
                    |summary| {
                        *index.summary.lock().unwrap() = summary;
                        ctx.request_repaint();
                    },
                );
                *index.scanned_at.lock().unwrap() = Some(Instant::now());
                index.scanning.store(false, Ordering::Release);
                ctx.request_repaint();
            });
        if let Err(err) = spawned {
            eprintln!("couldn't start the installed scan: {err}");
            self.scanning.store(false, Ordering::Release);
            return false;
        }
        true
    }
    fn publish(&self, found: HashMap<String, InstallState>, authoritative: bool) {
        let snapshot = {
            let mut states = self.states.lock().unwrap();
            if authoritative {
                *states = found;
            } else {
                states.extend(found);
            }
            *self.counts.lock().unwrap() = count_states(&states);
            states.clone()
        };
        save_installed_cache(&snapshot);
    }
    pub fn mark_installed(&self, key: &str) {
        if key.is_empty() {
            return;
        }
        let mut states = self.states.lock().unwrap();
        states.insert(key.to_owned(), InstallState::Installed);
        *self.counts.lock().unwrap() = count_states(&states);
        save_installed_cache(&states);
    }
}
impl Default for InstalledIndex {
    fn default() -> Self {
        Self::new()
    }
}
pub fn hash_file_path(titleid: &str) -> PathBuf {
    PathBuf::from(HASH_CACHE_DIR).join(format!("{}.hash", titleid.trim().to_uppercase()))
}
pub fn stamp_pending_install(titleid: &str, hash: &str, extract_dir: Option<&Path>) {
    if titleid.is_empty() {
        return;
    }
    let hash = hash.trim();
    let lower_hash = if hash.len() == HASH_LEN { hash.to_lowercase() } else { String::new() };
    let dir = PathBuf::from(HASH_CACHE_DIR);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(hash_file_path(titleid), &lower_hash);
    if let Some(extract_dir) = extract_dir
        && !lower_hash.is_empty()
    {
        let _ = std::fs::write(extract_dir.join("hash.vdb"), &lower_hash);
    }
}
pub fn clear_pending_install(titleid: &str) {
    if !titleid.is_empty() {
        let _ = std::fs::remove_file(hash_file_path(titleid));
    }
}
fn ledger() -> HashMap<String, Option<String>> {
    let mut out = HashMap::new();
    let Ok(dir) = std::fs::read_dir(HASH_CACHE_DIR) else { return out };
    for entry in dir.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        let Some(titleid) = name.strip_suffix(".hash") else { continue };
        let hash = read_hash_file(&entry.path());
        out.insert(key_of(titleid), hash);
    }
    out
}
fn scan(
    wanted: &[Wanted],
    hash_gap: Duration,
    mut publish: impl FnMut(HashMap<String, InstallState>, bool),
    report: impl Fn(String),
) {
    let mut log = Vec::new();
    note(&mut log, format!("build {}", env!("BUILD_STAMP")));
    let (listing, listed) = list_app_dirs(&mut log);
    let by_listing = listed && !listing.is_empty();
    if !by_listing {
        note(&mut log, "nothing could be listed, asking by title id instead".to_owned());
    }
    let mut found: HashMap<String, PathBuf> = HashMap::new();
    for entry in wanted {
        let dir = if by_listing {
            listing.get(&key_of(&entry.folder)).cloned()
        } else {
            locate(entry)
        };
        if let Some(dir) = dir {
            found.insert(entry.key.clone(), dir);
        }
    }
    let mut asked_system = false;
    if found.is_empty() {
        let titleids: Vec<String> =
            wanted.iter().filter(|entry| !entry.psp).map(|entry| entry.folder.clone()).collect();
        let started = Instant::now();
        match super::promoter::installed_titles(&titleids) {
            Some(installed) => {
                asked_system = true;
                note(
                    &mut log,
                    format!(
                        "the system reports {} of {} installed, in {} ms",
                        installed.len(),
                        titleids.len(),
                        started.elapsed().as_millis()
                    ),
                );
                for titleid in installed {
                    found.insert(titleid.clone(), PathBuf::from(format!("{}/{titleid}", APP_ROOTS[0])));
                }
            }
            None => note(&mut log, "the system would not say what is installed".to_owned()),
        }
    }
    let authoritative = !found.is_empty();
    note(
        &mut log,
        format!("{} folders listed, {} catalog entries installed", listing.len(), found.len()),
    );
    let first_pass: HashMap<String, InstallState> =
        found.keys().map(|key| (key.clone(), InstallState::Installed)).collect();
    publish(first_pass, authoritative);
    let ledger = ledger();
    let expected: HashMap<&str, (&str, &str)> = wanted
        .iter()
        .map(|entry| (entry.key.as_str(), (entry.hash.as_str(), entry.hash2.as_str())))
        .collect();
    let mut from_ledger = HashMap::new();
    for (key, stamped) in &ledger {
        if authoritative && !found.contains_key(key) {
            let _ = std::fs::remove_file(hash_file_path(key));
            continue;
        }
        let state = match (stamped, expected.get(key.as_str())) {
            (Some(stamped), Some((e1, e2))) => compare(stamped, e1, e2),
            _ => InstallState::Installed,
        };
        from_ledger.insert(key.clone(), state);
    }
    note(&mut log, format!("{} titles installed from vitaforge", from_ledger.len()));
    report(format!(
        "b{} · found {} · dirs {} · vf {} · {}",
        env!("BUILD_STAMP"),
        found.len(),
        listing.len(),
        from_ledger.len(),
        match (by_listing, asked_system) {
            (true, _) => "listed",
            (_, true) => "system",
            _ => "asked",
        }
    ));
    if !from_ledger.is_empty() {
        publish(from_ledger, false);
    }
    write_log(&log);
    let mut batch = HashMap::new();
    for entry in wanted {
        if entry.hash.len() != HASH_LEN && entry.hash2.len() != HASH_LEN {
            continue;
        }
        if ledger.get(&entry.key).is_some_and(|stamped| stamped.is_some()) {
            continue;
        }
        let Some(app_dir) = found.get(&entry.key) else { continue };
        std::thread::sleep(hash_gap);
        if hash_state(&entry.key, app_dir, &entry.hash, &entry.hash2) == InstallState::Outdated {
            batch.insert(entry.key.clone(), InstallState::Outdated);
        }
        if batch.len() >= HASH_BATCH {
            publish(std::mem::take(&mut batch), false);
        }
    }
    if !batch.is_empty() {
        publish(batch, false);
    }
    let mut version_batch = HashMap::new();
    for entry in wanted {
        if entry.hash.len() == HASH_LEN || entry.hash2.len() == HASH_LEN {
            continue; // already covered by the hash-based comparison above
        }
        let Some(app_dir) = found.get(&entry.key) else { continue };
        let Some(installed_ver) = super::sfo::read_app_ver(&app_dir.join("sce_sys/param.sfo")) else { continue };
        if version_is_older(&installed_ver, &entry.catalog_version) {
            version_batch.insert(entry.key.clone(), InstallState::Outdated);
        }
    }
    if !version_batch.is_empty() {
        publish(version_batch, false);
    }
}
fn version_is_older(installed: &str, catalog: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        let nums: Vec<u64> = v
            .trim()
            .trim_start_matches(['v', 'V'])
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();
        (!nums.is_empty()).then_some(nums)
    }
    match (parts(installed), parts(catalog)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}
fn locate(entry: &Wanted) -> Option<PathBuf> {
    for root in entry.roots() {
        let dir = format!("{root}/{}", entry.folder);
        for marker in entry.markers() {
            let path = format!("{dir}/{marker}");
            if vita_fs::exists(&path) || Path::new(&path).exists() {
                return Some(PathBuf::from(dir));
            }
        }
    }
    if !entry.psp {
        for root in META_ROOTS {
            let meta = format!("{root}/{}", entry.folder);
            if vita_fs::exists(&meta) || Path::new(&meta).exists() {
                return Some(PathBuf::from(format!("{}/{}", APP_ROOTS[0], entry.folder)));
            }
        }
    }
    None
}
fn note(log: &mut Vec<String>, line: String) {
    eprintln!("installed scan: {line}");
    log.push(line);
}
fn write_log(lines: &[String]) {
    let _ = std::fs::create_dir_all(WORK_DIR);
    let _ = std::fs::write(SCAN_LOG, lines.join("\n"));
}
fn key_of(titleid: &str) -> String {
    titleid.trim().to_uppercase()
}
#[cfg(target_os = "vita")]
pub mod vita_fs {
    use std::ffi::CString;
    pub fn list_dir(path: &str) -> Result<Vec<String>, i32> {
        let Ok(path) = CString::new(path) else { return Err(-1) };
        let mut names = Vec::new();
        unsafe {
            let fd = vitasdk_sys::sceIoDopen(path.as_ptr());
            if fd < 0 {
                return Err(fd);
            }
            loop {
                let mut dirent: vitasdk_sys::SceIoDirent = std::mem::zeroed();
                if vitasdk_sys::sceIoDread(fd, &mut dirent) <= 0 {
                    break;
                }
                let name = name_of(&dirent);
                if !name.is_empty() && name != "." && name != ".." {
                    names.push(name);
                }
            }
            vitasdk_sys::sceIoDclose(fd);
        }
        Ok(names)
    }
    pub fn exists(path: &str) -> bool {
        stat_code(path) >= 0
    }
    pub fn stat_code(path: &str) -> i32 {
        let Ok(path) = CString::new(path) else { return -1 };
        let mut stat: vitasdk_sys::SceIoStat = unsafe { std::mem::zeroed() };
        unsafe { vitasdk_sys::sceIoGetstat(path.as_ptr(), &mut stat) }
    }
    pub fn mkdir(path: &str, mode: i32) -> i32 {
        let Ok(path) = CString::new(path) else { return -1 };
        unsafe { vitasdk_sys::sceIoMkdir(path.as_ptr(), mode as vitasdk_sys::SceMode) }
    }
    pub fn rmdir(path: &str) -> i32 {
        let Ok(path) = CString::new(path) else { return -1 };
        unsafe { vitasdk_sys::sceIoRmdir(path.as_ptr()) }
    }
    fn name_of(dirent: &vitasdk_sys::SceIoDirent) -> String {
        let bytes: Vec<u8> = dirent
            .d_name
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}
#[cfg(not(target_os = "vita"))]
pub mod vita_fs {
    pub fn list_dir(_path: &str) -> Result<Vec<String>, i32> {
        Err(-1)
    }
    pub fn exists(_path: &str) -> bool {
        false
    }
    pub fn stat_code(_path: &str) -> i32 {
        -1
    }
    pub fn mkdir(_path: &str, _mode: i32) -> i32 {
        -1
    }
    pub fn rmdir(_path: &str) -> i32 {
        -1
    }
}
fn list_app_dirs(log: &mut Vec<String>) -> (HashMap<String, PathBuf>, bool) {
    let mut present: HashMap<String, PathBuf> = HashMap::new();
    let mut listed = false;
    for root in APP_ROOTS.iter().chain(PSP_ROOTS) {
        let (names, how) = match vita_fs::list_dir(root) {
            Ok(names) => (names, "sceIo".to_owned()),
            Err(code) => {
                match std::fs::read_dir(root).or_else(|err| {
                    std::fs::read_dir(format!("{root}/")).map_err(|_| err)
                }) {
                    Ok(dir) => (
                        dir.flatten().filter_map(|e| e.file_name().into_string().ok()).collect(),
                        format!("readdir after sceIo {code:#x}"),
                    ),
                    Err(err) => {
                        note(log, format!("couldn't list {root}: sceIo {code:#x}, readdir {err}"));
                        continue;
                    }
                }
            }
        };
        listed = true;
        note(log, format!("{root} -> {} folders ({how})", names.len()));
        for name in names {
            present.entry(key_of(&name)).or_insert_with(|| PathBuf::from(root).join(&name));
        }
    }
    (present, listed)
}
fn hash_state(titleid: &str, app_dir: &Path, expected1: &str, expected2: &str) -> InstallState {
    if let Some(cached) = read_hash_file(&app_dir.join("hash.vdb"))
        .or_else(|| read_hash_file(&app_dir.join("HASH.VDB")))
        .or_else(|| read_hash_file(&hash_file_path(titleid)))
    {
        return compare(&cached, expected1, expected2);
    }
    let Some(executable) = find_executable(app_dir) else {
        return InstallState::Installed;
    };
    let Some(digest) = md5_file(&executable) else {
        return InstallState::Installed;
    };
    let _ = std::fs::create_dir_all(HASH_CACHE_DIR);
    let _ = std::fs::write(hash_file_path(titleid), &digest);
    let _ = std::fs::write(app_dir.join("hash.vdb"), &digest);
    compare(&digest, expected1, expected2)
}
fn find_executable(app_dir: &Path) -> Option<PathBuf> {
    for name in &["eboot.bin", "EBOOT.BIN", "Eboot.bin", "eboot.pbp", "EBOOT.PBP", "Eboot.pbp"] {
        let p = app_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(dir) = std::fs::read_dir(app_dir) {
        for entry in dir.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.eq_ignore_ascii_case("eboot.bin") || name.eq_ignore_ascii_case("eboot.pbp") {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}
fn compare(actual: &str, expected1: &str, expected2: &str) -> InstallState {
    if (expected1.len() == HASH_LEN && actual.eq_ignore_ascii_case(expected1))
        || (expected2.len() == HASH_LEN && actual.eq_ignore_ascii_case(expected2))
    {
        InstallState::Installed
    } else {
        InstallState::Outdated
    }
}
fn read_hash_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.len() >= HASH_LEN {
        let candidate = &trimmed[..HASH_LEN];
        if candidate.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(candidate.to_lowercase());
        }
    }
    None
}
fn md5_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Md5::new();
    let mut buffer = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format!("{:x}", hasher.finalize()))
}
#[cfg(test)]
mod tests {
    use super::{InstallState, count_states, format_ymd, version_is_older};
    use std::collections::HashMap;
    #[test]
    fn count_states_counts_outdated_as_installed_too() {
        let states = HashMap::from([
            ("A".to_owned(), InstallState::Installed),
            ("B".to_owned(), InstallState::Outdated),
            ("C".to_owned(), InstallState::Installed),
        ]);
        assert_eq!(count_states(&states), (3, 1));
    }
    #[test]
    fn format_ymd_matches_known_dates() {
        assert_eq!(format_ymd(0), "1970-01-01");
        assert_eq!(format_ymd(1_609_459_200), "2021-01-01");
    }
    #[test]
    fn older_installed_version_is_detected() {
        assert!(version_is_older("1.9", "1.10"));
        assert!(version_is_older("v1.0.0", "v1.0.1"));
    }
    #[test]
    fn newer_or_equal_installed_version_is_never_flagged() {
        assert!(!version_is_older("1.10", "1.9"));
        assert!(!version_is_older("2.0", "2.0"));
        assert!(!version_is_older("3.0", "1.0"));
    }
    #[test]
    fn unparseable_versions_never_produce_a_false_outdated() {
        assert!(!version_is_older("custom-build", "1.0"));
        assert!(!version_is_older("1.0", "latest"));
        assert!(!version_is_older("", ""));
    }
}
