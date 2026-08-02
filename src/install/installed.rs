//! Which catalog entries are on the device, and whether they are up to date.
//!
//! We hash the installed `eboot.bin` and compare it against the MD5 the catalog
//! ships, which works even for apps we did not install ourselves. Cached in
//! `hash.vdb` next to the app, so the read is only paid once.

use md5::{Digest, Md5};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::data::AppEntry;

const APP_ROOT: &str = "ux0:app";
/// 32 lowercase hex characters, no newline.
const HASH_FILE: &str = "hash.vdb";
const HASH_LEN: usize = 32;
/// A leftover directory proves nothing; the executable does.
const EXECUTABLE: &str = "eboot.bin";
/// Hashing reads the whole executable, so collapse bursts of requests.
const RESCAN_INTERVAL: Duration = Duration::from_millis(750);
const HASH_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    Absent,
    /// Installed, but not the build the catalog now ships.
    Outdated,
    Installed,
}

#[derive(Clone)]
pub struct InstalledIndex {
    states: Arc<Mutex<HashMap<String, InstallState>>>,
    scanned_at: Arc<Mutex<Option<Instant>>>,
    scanning: Arc<AtomicBool>,
}

impl InstalledIndex {
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            scanned_at: Arc::new(Mutex::new(None)),
            scanning: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn state(&self, titleid: &str) -> InstallState {
        if titleid.is_empty() {
            return InstallState::Absent;
        }
        self.states.lock().unwrap().get(titleid).copied().unwrap_or(InstallState::Absent)
    }

    /// Rescans unless one just ran. Far too slow for the UI thread, so it goes to
    /// the blocking pool and repaints when it lands.
    pub fn refresh(&self, ctx: &egui::Context, entries: &[AppEntry]) {
        if entries.is_empty() || self.scanning.load(Ordering::Acquire) {
            return;
        }
        {
            let scanned_at = self.scanned_at.lock().unwrap();
            if scanned_at.is_some_and(|at| at.elapsed() < RESCAN_INTERVAL) {
                return;
            }
        }
        self.scanning.store(true, Ordering::Release);

        // Only what the scan needs, so the worker never holds the catalog.
        let wanted: Vec<(String, String)> = entries
            .iter()
            .filter(|entry| !entry.titleid.is_empty())
            .map(|entry| (entry.titleid.clone(), entry.hash.clone()))
            .collect();

        let index = self.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let found = tokio::task::spawn_blocking(move || scan(&wanted)).await.unwrap_or_default();
            *index.states.lock().unwrap() = found;
            *index.scanned_at.lock().unwrap() = Some(Instant::now());
            index.scanning.store(false, Ordering::Release);
            ctx.request_repaint();
        });
    }

    /// Flips the badge as soon as an install succeeds, without waiting for a rescan.
    pub fn mark_installed(&self, titleid: &str) {
        if titleid.is_empty() {
            return;
        }
        self.states.lock().unwrap().insert(titleid.to_owned(), InstallState::Installed);
    }
}

impl Default for InstalledIndex {
    fn default() -> Self {
        Self::new()
    }
}

pub fn hash_file_path(dir: &Path) -> PathBuf {
    dir.join(HASH_FILE)
}

/// Writes the hash into a folder that is about to be promoted, so it ships as
/// part of the package rather than being added to a folder the system owns.
pub fn stamp_pending_install(extract_dir: &Path, hash: &str) {
    let hash = hash.trim();
    if hash.len() != HASH_LEN {
        return;
    }
    if let Err(err) = std::fs::write(hash_file_path(extract_dir), hash.to_lowercase()) {
        eprintln!("couldn't stamp the package with its hash: {err}");
    }
}

fn scan(wanted: &[(String, String)]) -> HashMap<String, InstallState> {
    // One readdir up front, so absent entries never touch the filesystem again.
    let present: HashSet<String> = match std::fs::read_dir(APP_ROOT) {
        Ok(dir) => dir.flatten().filter_map(|entry| entry.file_name().into_string().ok()).collect(),
        Err(_) => return HashMap::new(),
    };

    let mut states = HashMap::new();
    for (titleid, expected) in wanted {
        if !present.contains(titleid) {
            continue;
        }
        let state = resolve(titleid, expected);
        if state != InstallState::Absent {
            states.insert(titleid.clone(), state);
        }
    }
    states
}

fn resolve(titleid: &str, expected: &str) -> InstallState {
    let dir = Path::new(APP_ROOT).join(titleid);
    let executable = dir.join(EXECUTABLE);
    if !executable.is_file() {
        return InstallState::Absent;
    }
    // There, but with nothing to compare it against.
    if expected.len() != HASH_LEN {
        return InstallState::Installed;
    }

    let stamp = hash_file_path(&dir);
    if let Some(cached) = read_hash(&stamp) {
        return compare(&cached, expected);
    }

    let Some(digest) = md5_file(&executable) else {
        return InstallState::Installed;
    };
    // Cached, so the full read is only ever paid once.
    if let Err(err) = std::fs::write(&stamp, &digest) {
        eprintln!("couldn't cache the hash for {titleid}: {err}");
    }
    compare(&digest, expected)
}

fn compare(actual: &str, expected: &str) -> InstallState {
    if actual.eq_ignore_ascii_case(expected) {
        InstallState::Installed
    } else {
        InstallState::Outdated
    }
}

fn read_hash(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = [0u8; HASH_LEN];
    file.read_exact(&mut buffer).ok()?;
    let hash = std::str::from_utf8(&buffer).ok()?;
    hash.chars().all(|c| c.is_ascii_hexdigit()).then(|| hash.to_lowercase())
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
