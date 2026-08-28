pub mod download;
pub mod github;
mod head;
pub mod installed;
pub mod notify;
pub mod pkg;
mod promoter;
mod sfo;
mod pbp;
mod bgdl;
mod licensing;
use crate::data::AppEntry;
use anyhow::{Context, Result, bail};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};
const WORK_DIR: &str = "ux0:data/vitaforge";
const TEMP_VPK: &str = "ux0:data/vitaforge/tmp.vpk";
const EXTRACT_DIR: &str = "ux0:data/vitaforge/vpk_install";
const TEMP_DATA: &str = "ux0:data/vitaforge/tmp_data.zip";
const TEMP_PKG: &str = "ux0:data/vitaforge/tmp.pkg";
const DATA_STAGE_DIR: &str = "ux0:data/vitaforge/data_stage";
const DATA_ROOT: &str = "ux0:data";
const PSP_GAME_DIR: &str = "ux0:pspemu/PSP/GAME";
const PLUGIN_DIR: &str = "ux0:data/vitaforge/plugins";
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Resolving,
    DownloadingData { received: u64, total: Option<u64>, elapsed_secs: u32 },
    Downloading { received: u64, total: Option<u64>, elapsed_secs: u32 },

    Decrypting,
    Extracting { done: u32, total: u32 },
    Installing { elapsed_secs: u32 },

    Queued,
    Done,
    Failed(String),
}
pub const CANCELLED_MESSAGE: &str = "cancelled";
fn format_clock(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}
fn transfer_detail(received: u64, total: Option<u64>, elapsed_secs: u32) -> String {
    match total {
        Some(total) if total > 0 => {
            let percent = (received * 100 / total).min(100);
            if elapsed_secs >= 2 && received > 0 && received < total {
                let remaining = (total - received) as f64 * elapsed_secs as f64 / received as f64;
                format!("{percent}% · {} left", format_clock(remaining as u64))
            } else {
                format!("{percent}%")
            }
        }
        _ => format!(
            "{:.1} MB · {}",
            received as f64 / (1024.0 * 1024.0),
            format_clock(elapsed_secs as u64)
        ),
    }
}
impl Progress {
    pub fn label(&self) -> String {
        match self {
            Progress::Resolving => "Checking latest…".to_owned(),
            Progress::Downloading { received, total, elapsed_secs } => {
                format!("[2/3] Downloading {}", transfer_detail(*received, *total, *elapsed_secs))
            }
            Progress::DownloadingData { received, total, elapsed_secs } => {
                format!("[1/3] Game data {}", transfer_detail(*received, *total, *elapsed_secs))
            }
            Progress::Decrypting => "[3/3] Decrypting…".to_owned(),
            Progress::Extracting { done, total } if *total > 0 => {
                format!("[3/3] Extracting {done}/{total} ({}%)", (*done as u64 * 100 / *total as u64).min(100))
            }
            Progress::Extracting { .. } => "[3/3] Extracting…".to_owned(),
            Progress::Installing { elapsed_secs } if *elapsed_secs > 0 => {
                format!("[3/3] Installing… ({elapsed_secs}s)")
            }
            Progress::Installing { .. } => "[3/3] Installing…".to_owned(),
            Progress::Queued => "Queued — check Notifications".to_owned(),
            Progress::Done => "Installed".to_owned(),
            Progress::Failed(err) if err == CANCELLED_MESSAGE => "Cancelled".to_owned(),
            Progress::Failed(err) => format!("Failed: {err}"),
        }
    }
    pub fn is_finished(&self) -> bool {
        matches!(self, Progress::Queued | Progress::Done | Progress::Failed(_))
    }
    pub fn is_cancellable(&self) -> bool {
        !self.is_finished() && !matches!(self, Progress::Installing { .. })
    }
    pub fn step(&self) -> usize {
        match self {
            Progress::Resolving | Progress::DownloadingData { .. } | Progress::Downloading { .. } => 1,
            Progress::Decrypting | Progress::Extracting { .. } => 2,
            Progress::Installing { .. } | Progress::Queued | Progress::Done | Progress::Failed(_) => 3,
        }
    }
}

struct PendingBgdl {
    title: String,
    url: String,
    rif: Option<Vec<u8>>,
    bgdl_type: u32,
    done: oneshot::Sender<Result<()>>,
}
static PENDING_BGDL: Mutex<VecDeque<PendingBgdl>> = Mutex::new(VecDeque::new());
static PENDING_BGDL_DELAY: Mutex<u32> = Mutex::new(0);
async fn queue_livearea_install(title: &str, url: &str, rif: Option<Vec<u8>>, bgdl_type: u32) -> Result<()> {
    let (done, wait) = oneshot::channel();
    {
        let mut queue = PENDING_BGDL.lock().unwrap();
        queue.push_back(PendingBgdl {
            title: title.to_owned(),
            url: url.to_owned(),
            rif,
            bgdl_type,
            done,
        });
        let mut delay = PENDING_BGDL_DELAY.lock().unwrap();
        if *delay < 2 {
            *delay = 2;
        }
    }
    wait.await.unwrap_or_else(|_| bail!("the BGDL queue was dropped before it ran"))
}
pub fn process_pending_bgdl() {
    {
        let mut delay = PENDING_BGDL_DELAY.lock().unwrap();
        if *delay > 0 {
            *delay -= 1;
            return;
        }
    }
    loop {
        let Some(pending) = PENDING_BGDL.lock().unwrap().pop_front() else { return };
        let result = bgdl::start_bgdl(
            &pending.title,
            &pending.url,
            pending.rif.as_deref(),
            pending.bgdl_type,
        );
        let _ = pending.done.send(result);
    }
}
pub fn log_file(msg: &str) {
    let _ = std::fs::create_dir_all("ux0:data/vitaforge");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("ux0:data/vitaforge/vitaforge.log") {
        use std::io::Write;
        let _ = writeln!(f, "[LOG] {msg}");
    }
}
fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!(CANCELLED_MESSAGE);
    }
    Ok(())
}
fn required_bytes(size_bytes: u64, data_size_bytes: u64) -> u64 {
    size_bytes
        .saturating_add(data_size_bytes)
        .saturating_mul(2)
}
fn check_free_space(size_bytes: u64, data_size_bytes: u64, storage: Option<(u64, u64)>) -> Result<()> {
    let required = required_bytes(size_bytes, data_size_bytes);
    if required == 0 {
        return Ok(());
    }
    let Some((used, total)) = storage else {
        return Ok(());
    };
    let available = total.saturating_sub(used);
    if available < required {
        bail!(
            "not enough free space: {} required, {} available",
            crate::data::cache_manager::format_bytes(required),
            crate::data::cache_manager::format_bytes(available)
        );
    }
    Ok(())
}
fn sanitize_data_dest(path: &str) -> Result<PathBuf> {
    let allowed = ["ux0:", "ur0:", "uma0:"];
    if !allowed.iter().any(|root| path.starts_with(root)) {
        bail!("game data extract path '{path}' must start with ux0:, ur0: or uma0:");
    }
    if path.split(['/', '\\']).any(|part| part == "..") {
        bail!("game data extract path '{path}' must not contain '..'");
    }
    Ok(PathBuf::from(path))
}
pub fn start(entry: AppEntry) -> (watch::Receiver<Progress>, Arc<AtomicBool>) {
    log_file(&format!("Install requested for '{}' (platform: {:?}, url: '{}')", entry.name, entry.platform, entry.download_url));
    let (tx, rx) = watch::channel(Progress::Resolving);
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    let _ = std::fs::remove_file(TEMP_VPK);
    let _ = std::fs::remove_file(TEMP_DATA);
    let _ = std::fs::remove_file(TEMP_PKG);
    tokio::spawn(async move {
        let result = run(&entry, &tx, &cancel_worker).await;
        let final_state = match result {
            Ok(state) => {
                log_file(&format!("Install completed successfully for '{}': {:?}", entry.name, state));
                state
            }
            Err(err) => {
                log_file(&format!("Install FAILED for '{}': {err:#}", entry.name));
                eprintln!("install failed: {err:#}");
                let _ = std::fs::remove_dir_all(EXTRACT_DIR);
                let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
                let _ = std::fs::remove_file(TEMP_VPK);
                let _ = std::fs::remove_file(TEMP_DATA);
                let _ = std::fs::remove_file(TEMP_PKG);
                installed::clear_pending_install(&installed::index_key(&entry));
                Progress::Failed(format!("{err}"))
            }
        };
        let _ = tx.send(final_state);
    });
    (rx, cancel)
}
async fn run(entry: &AppEntry, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<Progress> {
    match entry.platform {
        crate::data::Platform::Vita => run_vita(entry, tx, cancel).await,
        crate::data::Platform::Psp => run_psp(entry, tx, cancel).await,
        crate::data::Platform::Plugin => run_plugin(entry, tx, cancel).await,
        crate::data::Platform::NpsVita => run_nps_vita(entry, tx).await,
        crate::data::Platform::NpsPsp | crate::data::Platform::NpsPsx => {
            bail!("PSP/PS1 installs are temporarily unavailable")
        }
    }
}
async fn run_nps_vita(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this NPS game");
    }
    queue_bgdl_vita(entry, tx).await
}
async fn install_pbp_pkg(entry: &AppEntry, tx: &watch::Sender<Progress>, pkg_path: &Path, kind: pkg::PkgKind) -> Result<Progress> {
    let _ = tx.send(Progress::Decrypting);
    let staging_dir = PathBuf::from(EXTRACT_DIR).join(format!("pbp_{:?}", kind));
    let pkg_path_owned = pkg_path.to_path_buf();
    let staging_dir_worker = staging_dir.clone();
    let extract_fn = match kind {
        pkg::PkgKind::Psx => pkg::extract_psx,
        pkg::PkgKind::Psp => pkg::extract_psp,
        _ => bail!("invalid pkg kind for pbp install"),
    };
    tokio::task::spawn_blocking(move || extract_fn(&pkg_path_owned, &staging_dir_worker))
        .await
        .context("the pkg extract worker crashed")??;
    let _ = std::fs::remove_file(pkg_path);

    let eboot_path = staging_dir.join("EBOOT.PBP");
    let disc_id = pbp::read_disc_id(&eboot_path).unwrap_or_else(|| entry.titleid.trim().to_string());
    let final_id = if disc_id.len() >= 9 { &disc_id[..9] } else { entry.titleid.trim() };

    let dest = PathBuf::from(PSP_GAME_DIR).join(final_id);
    std::fs::create_dir_all(PSP_GAME_DIR)?;
    let _ = std::fs::remove_dir_all(&dest);
    if std::fs::rename(&staging_dir, &dest).is_err() {
        std::fs::create_dir_all(&dest)?;
        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let dest_path = dest.join(&file_name);
            if entry.metadata()?.is_dir() {
                merge_into(&entry.path(), &dest_path)?;
            } else {
                std::fs::copy(entry.path(), &dest_path)?;
            }
        }
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    if licensing::is_module_present("NoPspEmuDrm_kern") {
        let content_id = licensing::resolve_content_id(entry)?;
        let psp_license = licensing::create_fake_license(&content_id);
        std::fs::create_dir_all("ux0:pspemu/PSP/LICENSE").context("couldn't create the PSP license folder")?;
        std::fs::write(format!("ux0:pspemu/PSP/LICENSE/{content_id}.rif"), &psp_license)
            .context("couldn't write the PSP license file")?;
    }
    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, &entry.version, None);
    Ok(Progress::Done)
}
async fn queue_bgdl_vita(entry: &AppEntry, _tx: &watch::Sender<Progress>) -> Result<Progress> {
    let license = licensing::get_license_for_entry(entry);
    stage_bgdl_icon(entry).await;
    log_file(&format!(
        "Queueing BGDL for '{}' with url '{}', resolved content_id={:?}",
        entry.name,
        entry.download_url,
        licensing::resolve_content_id(entry).ok(),
    ));
    queue_livearea_install(&entry.name, &entry.download_url, license, bgdl::BGDL_TYPE_GAME)
        .await
        .context("Failed to queue background download (BGDL)")?;
    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, &entry.version, None);
    Ok(Progress::Queued)
}
async fn run_psp(entry: &AppEntry, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this app");
    }
    if entry.platform.is_nps() {
        std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
        let pkg_path = Path::new(TEMP_PKG);
        download(&entry.download_url, pkg_path, tx, cancel).await?;
        let kind = match entry.platform {
            crate::data::Platform::NpsPsx => pkg::PkgKind::Psx,
            crate::data::Platform::NpsPsp => pkg::PkgKind::Psp,
            _ => bail!("unexpected platform in NPS block"),
        };
        return install_pbp_pkg(entry, tx, pkg_path, kind).await;
    }
    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    download(&entry.download_url, Path::new(TEMP_VPK), tx, cancel).await?;
    let dest = PathBuf::from(PSP_GAME_DIR).join(&entry.id);
    let _ = tx.send(Progress::Extracting { done: 0, total: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(Path::new(TEMP_VPK), &dest, &cancel_worker, |done, total| {
            let _ = tx_worker.send(Progress::Extracting { done, total });
        })
    })
    .await
    .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);
    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, &entry.version, None);
    Ok(Progress::Done)
}
async fn stage_bgdl_icon(entry: &AppEntry) {
    let Some(url) = entry.icon_url.as_deref().or(entry.cover_url.as_deref()) else { return };
    let request = crate::net::client().get(url).header("User-Agent", "VitaForge").send();
    let Ok(response) = request.await else { return };
    let Ok(bytes) = response.bytes().await else { return };
    let _ = std::fs::create_dir_all("ux0:data/vitaforge");
    let _ = std::fs::write("ux0:data/vitaforge/bgdl_icon.png", &bytes);
}
async fn run_plugin(entry: &AppEntry, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this plugin");
    }
    let name = entry
        .download_url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("plugin.skprx");
    std::fs::create_dir_all(PLUGIN_DIR).context("couldn't create the plugins folder")?;
    let dest = PathBuf::from(PLUGIN_DIR).join(name);
    download(&entry.download_url, &dest, tx, cancel).await?;
    Ok(Progress::Done)
}
async fn run_vita(entry: &AppEntry, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    let url = match entry.source.as_deref() {
        Some(repo) => match github::latest_release(repo).await {
            Some(release) => {
                println!("using github release {} for {}", release.tag, entry.name);
                release.vpk_url
            }
            None => entry.download_url.clone(),
        },
        None => entry.download_url.clone(),
    };
    if url.is_empty() {
        bail!("no download link for this app");
    }
    check_free_space(
        entry.size_bytes,
        entry.data_size_bytes,
        crate::app::sysinfo::storage("ux0:"),
    )?;
    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    let _ = std::fs::remove_dir_all(EXTRACT_DIR);
    let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
    if let Some(data_url) = entry.data_url.as_deref() {
        stage_data(data_url, tx, cancel).await?;
    }
    download(&url, Path::new(TEMP_VPK), tx, cancel).await?;
    let _ = tx.send(Progress::Extracting { done: 0, total: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(Path::new(TEMP_VPK), Path::new(EXTRACT_DIR), &cancel_worker, |done, total| {
            let _ = tx_worker.send(Progress::Extracting { done, total });
        })
    })
    .await
    .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);
    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, &entry.version, Some(Path::new(EXTRACT_DIR)));
    head::write(Path::new(EXTRACT_DIR))?;
    let titleid = entry.titleid.trim().to_uppercase();
    if !titleid.is_empty() {
        for root in &["ux0:app", "ur0:app", "uma0:app"] {
            let path = format!("{root}/{titleid}");
            if installed::vita_fs::exists(&path) || Path::new(&path).exists() {
                let path_owned = path.clone();
                let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&path_owned)).await;
                if let Ok(Err(err)) = removed {
                    eprintln!("couldn't clear the previous install at {path}: {err}");
                }
            }
        }
    }
    let _ = tx.send(Progress::Installing { elapsed_secs: 0 });
    let tx_worker = tx.clone();
    tokio::task::spawn_blocking(move || promoter::promote_package(EXTRACT_DIR, &tx_worker))
        .await
        .context("the install worker crashed")??;
    if Path::new(EXTRACT_DIR).exists() {
        let _ = std::fs::remove_dir_all(EXTRACT_DIR);
        let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
        bail!("the system didn't accept the package");
    }
    if Path::new(DATA_STAGE_DIR).exists() {
        let data_dest = entry
            .data_extract_path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(sanitize_data_dest)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from(DATA_ROOT));
        tokio::task::spawn_blocking(move || merge_into(Path::new(DATA_STAGE_DIR), &data_dest))
            .await
            .context("the data worker crashed")??;
        let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
    }
    Ok(Progress::Done)
}
async fn stage_data(url: &str, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<()> {
    let dest = Path::new(TEMP_DATA);
    download_with(url, dest, tx, cancel, |received, total, elapsed_secs| Progress::DownloadingData {
        received,
        total,
        elapsed_secs,
    })
        .await
        .context("couldn't download the required game data")?;
    let _ = tx.send(Progress::Extracting { done: 0, total: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(Path::new(TEMP_DATA), Path::new(DATA_STAGE_DIR), &cancel_worker, |done, total| {
            let _ = tx_worker.send(Progress::Extracting { done, total });
        })
    })
    .await
    .context("the data extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_DATA);
    Ok(())
}
fn merge_into(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_into(&entry.path(), &target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if target.exists() {
                let _ = std::fs::remove_file(&target);
            }
            if std::fs::rename(entry.path(), &target).is_err() {
                std::fs::copy(entry.path(), &target)
                    .with_context(|| format!("couldn't copy {}", target.display()))?;
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}
async fn download(url: &str, dest: &Path, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<()> {
    download_with(url, dest, tx, cancel, |received, total, elapsed_secs| Progress::Downloading {
        received,
        total,
        elapsed_secs,
    })
    .await
}
async fn download_with(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
    progress: fn(u64, Option<u64>, u32) -> Progress,
) -> Result<()> {
    let mut sink = download::FileSink::open(dest)?;
    let mut reporter = download::ProgressReporter::new(tx, progress);
    download::fetch_to(url, &mut sink, &mut reporter, cancel).await
}
fn extract(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> Result<()> {
    let file = std::fs::File::open(archive).context("couldn't reopen the downloaded file")?;
    let buffered = std::io::BufReader::with_capacity(256 * 1024, file);
    let mut zip = zip::ZipArchive::new(buffered).context("the download isn't a valid vpk (zip)")?;
    let total = zip.len() as u32;
    let mut last_reported = std::time::Instant::now();
    let mut created_parent: Option<PathBuf> = None;
    for index in 0..zip.len() {
        check_cancelled(cancel)?;
        let mut entry = zip.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else { continue };
        let out_path: PathBuf = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent()
            && created_parent.as_deref() != Some(parent)
        {
            std::fs::create_dir_all(parent)?;
            created_parent = Some(parent.to_path_buf());
        }
        let out = std::fs::File::create(&out_path)
            .with_context(|| format!("couldn't write {}", out_path.display()))?;
        let mut out = std::io::BufWriter::with_capacity(256 * 1024, out);
        std::io::copy(&mut entry, &mut out)?;
        use std::io::Write;
        out.flush()?;
        if last_reported.elapsed() >= std::time::Duration::from_millis(250) {
            last_reported = std::time::Instant::now();
            on_progress(index as u32 + 1, total);
        }
    }
    on_progress(total, total);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{check_free_space, required_bytes, sanitize_data_dest, transfer_detail};
    #[test]
    fn shows_percent_and_eta_once_there_is_history() {
        assert_eq!(transfer_detail(50, Some(100), 10), "50% · 0:10 left");
    }
    #[test]
    fn holds_back_the_eta_until_the_sample_is_meaningful() {
        assert_eq!(transfer_detail(10, Some(100), 0), "10%");
        assert_eq!(transfer_detail(0, Some(100), 5), "0%");
        assert_eq!(transfer_detail(100, Some(100), 5), "100%");
    }
    #[test]
    fn falls_back_to_size_and_elapsed_without_a_content_length() {
        assert_eq!(transfer_detail(2 * 1024 * 1024, None, 65), "2.0 MB · 1:05");
        assert_eq!(transfer_detail(0, Some(0), 3), "0.0 MB · 0:03");
    }
    #[test]
    fn accepts_allowed_roots() {
        assert!(sanitize_data_dest("ux0:data/SOMEGAME").is_ok());
        assert!(sanitize_data_dest("ur0:data/SOMEGAME").is_ok());
        assert!(sanitize_data_dest("uma0:data/SOMEGAME").is_ok());
    }
    #[test]
    fn rejects_missing_volume_root() {
        assert!(sanitize_data_dest("data/SOMEGAME").is_err());
        assert!(sanitize_data_dest("/etc/passwd").is_err());
        assert!(sanitize_data_dest("").is_err());
    }
    #[test]
    fn rejects_path_traversal() {
        assert!(sanitize_data_dest("ux0:data/../app").is_err());
        assert!(sanitize_data_dest("ux0:data/SOMEGAME/../../app").is_err());
    }
    #[test]
    fn required_space_accounts_for_download_and_extraction() {
        assert_eq!(required_bytes(100, 50), 300);
        assert_eq!(required_bytes(0, 0), 0);
        assert_eq!(required_bytes(u64::MAX, 1), u64::MAX);
    }
    #[test]
    fn skips_space_check_when_sizes_or_storage_are_unknown() {
        assert!(check_free_space(0, 0, Some((100, 100))).is_ok());
        assert!(check_free_space(100, 50, None).is_ok());
    }
    #[test]
    fn compares_required_space_with_available_space() {
        assert!(check_free_space(100, 50, Some((700, 1_000))).is_ok());
        let error = check_free_space(100, 50, Some((701, 1_000))).unwrap_err();
        assert_eq!(
            error.to_string(),
            "not enough free space: 300 B required, 299 B available"
        );
    }
}
