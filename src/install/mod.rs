mod bgdl;
pub mod download;
pub mod github;
mod head;
pub mod installed;
pub mod lifecycle;
mod licensing;
pub mod notify;
mod pbp;
pub mod pkg;
mod promoter;
pub mod requirements;
mod sfo;
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
/// Pre-per-game single slot; removed on sight so it can't eat 3 GB forever.
const LEGACY_TEMP_DATA: &str = "ux0:data/vitaforge/tmp_data.zip";
/// Each game's data download lives in its own folder, so starting another
/// game never throws away a half-finished one.
const TEMP_DATA_DIR: &str = "ux0:data/vitaforge/tmp_data";
fn temp_data_path(entry: &AppEntry) -> PathBuf {
    let key: String = entry.id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    Path::new(TEMP_DATA_DIR).join(if key.is_empty() { "unknown".to_owned() } else { key }).join("data.zip")
}
const TEMP_PKG: &str = "ux0:data/vitaforge/tmp.pkg";
const DATA_STAGE_DIR: &str = "ux0:data/vitaforge/data_stage";
const DATA_ROOT: &str = "ux0:data";
const PSP_GAME_DIR: &str = "ux0:pspemu/PSP/GAME";
const PLUGIN_DIR: &str = "ux0:data/vitaforge/plugins";
const TAI_CONFIG_BACKUP_SUFFIX: &str = ".vitaforge.bak";
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Resolving,
    DownloadingData {
        received: u64,
        total: Option<u64>,
        elapsed_secs: u32,
    },
    Downloading {
        received: u64,
        total: Option<u64>,
        elapsed_secs: u32,
    },
    /// Post-download work on the finished file (joining parts, md5).
    Finishing {
        step: &'static str,
        percent: u32,
    },

    VerificationComplete,
    Decrypting,
    Extracting {
        done: u32,
        total: u32,
        file: u32,
        files: u32,
    },
    Installing {
        elapsed_secs: u32,
    },

    Queued,
    Done,
    RebootRequired,
    Restarting,
    Failed(String),
}
pub const CANCELLED_MESSAGE: &str = "cancelled";
pub const PLUGIN_PENDING_MESSAGE: &str = "plugin pending validation";
fn format_clock(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}
fn transfer_detail(received: u64, total: Option<u64>, elapsed_secs: u32) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    let mut detail = match total.filter(|total| *total > 0) {
        Some(total) => format!("{}% · {:.1}/{:.1} MiB", (received as f64 / total as f64 * 100.0).min(100.0) as u32, received as f64 / MIB, total as f64 / MIB),
        None => format!("{:.1} MiB", received as f64 / MIB),
    };
    if elapsed_secs >= 2 && received > 0 {
        detail.push_str(&format!(" · {:.2} MiB/s", received as f64 / MIB / elapsed_secs as f64));
        if let Some(total) = total.filter(|total| *total > received) {
            let remaining = (total - received) as f64 * elapsed_secs as f64 / received as f64;
            detail.push_str(&format!(" · ~{} left", format_clock(remaining as u64)));
        }
    }
    detail
}
impl Progress {
    pub fn label_for(&self, lang: crate::app::i18n::Language) -> String {
        let text = |id: &str| lang.tr(id);
        match self {
            Self::Resolving => text("progress-resolving"),
            Self::Downloading { received, total, elapsed_secs }
            | Self::DownloadingData { received, total, elapsed_secs } => {
                let phase = if matches!(self, Self::DownloadingData { .. }) {
                    text("progress-downloading-data")
                } else {
                    text("progress-downloading-app")
                };
                let detail = transfer_detail(*received, *total, *elapsed_secs);
                format!("{phase} · {}", detail.replace(" left", &format!(" {}", text("progress-left"))))
            }
            Self::Finishing { step, percent } => {
                let phase = match *step {
                    "Joining parts" => text("progress-joining"),
                    "Verifying" => text("progress-verifying"),
                    other => other.to_owned(),
                };
                format!("{phase} · {}%", (*percent).min(100))
            }
            Self::VerificationComplete => text("progress-verified"),
            Self::Decrypting => text("progress-decrypting"),
            Self::Extracting { done, total, file, files: 0 } if *total > 0 => format!(
                "{} · {}% · {done}/{total} MiB · {file} {}",
                text("progress-extracting"),
                (*done as u64 * 100 / *total as u64).min(100),
                text("progress-files"),
            ),
            Self::Extracting { done, total, file, files } if *total > 0 => format!(
                "{} · {}% · {done}/{total} MiB · {} {file}/{files}",
                text("progress-extracting"),
                (*done as u64 * 100 / *total as u64).min(100),
                text("progress-files"),
            ),
            Self::Extracting { .. } => text("progress-preparing-extract"),
            Self::Installing { elapsed_secs } => format!("{} · {} {}", text("progress-installing"), format_clock(*elapsed_secs as u64), text("progress-elapsed")),
            Self::Queued => text("progress-queued"),
            Self::Done => text("progress-done"),
            Self::RebootRequired => text("progress-reboot"),
            Self::Restarting => text("progress-restarting"),
            Self::Failed(err) if err == CANCELLED_MESSAGE => text("progress-cancelled"),
            Self::Failed(err) if err == PLUGIN_PENDING_MESSAGE => lang.plugin_pending().to_owned(),
            Self::Failed(err) => format!("{}: {err}", text("progress-failed")),
        }
    }
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Progress::Downloading { received, total, .. }
            | Progress::DownloadingData { received, total, .. } => match total {
                Some(total) if *total > 0 => Some((*received as f32 / *total as f32).clamp(0.0, 1.0)),
                _ => None,
            },
            Progress::Finishing { percent, .. } => Some((*percent as f32 / 100.0).clamp(0.0, 1.0)),
            Progress::Extracting { done, total, .. } if *total > 0 => {
                Some((*done as f32 / *total as f32).clamp(0.0, 1.0))
            }
            Progress::Queued | Progress::Done | Progress::RebootRequired | Progress::Restarting => {
                Some(1.0)
            }
            _ => None,
        }
    }
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            Progress::Queued
                | Progress::Done
                | Progress::RebootRequired
                | Progress::Restarting
                | Progress::Failed(_)
        )
    }
    pub fn is_cancellable(&self) -> bool {
        !self.is_finished() && !matches!(self, Progress::Installing { .. })
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
async fn queue_livearea_install(
    title: &str,
    url: &str,
    rif: Option<Vec<u8>>,
    bgdl_type: u32,
) -> Result<()> {
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
    wait.await
        .unwrap_or_else(|_| bail!("the BGDL queue was dropped before it ran"))
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
        let Some(pending) = PENDING_BGDL.lock().unwrap().pop_front() else {
            return;
        };
        #[cfg(target_os = "vita")]
        let result = bgdl::start_bgdl(
            &pending.title,
            &pending.url,
            pending.rif.as_deref(),
            pending.bgdl_type,
        );
        #[cfg(not(target_os = "vita"))]
        let result: Result<()> = Err(anyhow::anyhow!("BGDL is only available on PS Vita"));
        let _ = pending.done.send(result);
    }
}
pub fn log_file(msg: &str) {
    let _ = std::fs::create_dir_all("ux0:data/vitaforge");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("ux0:data/vitaforge/vitaforge.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{} [LOG] {msg}", crate::app::sysinfo::log_stamp());
    }
}
fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!(CANCELLED_MESSAGE);
    }
    Ok(())
}
fn required_bytes(size_bytes: u64, data_size_bytes: u64) -> u64 {
    size_bytes.saturating_add(data_size_bytes).saturating_mul(2)
}
fn shortfall(size_bytes: u64, data_size_bytes: u64, storage: Option<(u64, u64)>) -> Option<(u64, u64)> {
    let required = required_bytes(size_bytes, data_size_bytes);
    if required == 0 {
        return None;
    }
    let (used, total) = storage?;
    let available = total.saturating_sub(used);
    (available < required).then_some((required, available))
}
/// `(needed, available)` when the memory card cannot hold this install, so the
/// page can say so before the download starts.
pub fn space_shortfall(entry: &AppEntry) -> Option<(u64, u64)> {
    if entry.platform != crate::data::Platform::Vita {
        return None;
    }
    shortfall(
        entry.size_bytes,
        entry.data_size_bytes,
        crate::app::sysinfo::storage("ux0:"),
    )
}
fn check_free_space(
    size_bytes: u64,
    data_size_bytes: u64,
    storage: Option<(u64, u64)>,
) -> Result<()> {
    if let Some((required, available)) = shortfall(size_bytes, data_size_bytes, storage) {
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
mod keep_awake {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Keeps the Vita awake and the PS button locked while an install runs;
    /// suspend mid-transfer corrupts the download. Released on drop.
    pub struct Guard {
        alive: Arc<AtomicBool>,
    }

    impl Guard {
        pub fn new() -> Self {
            let alive = Arc::new(AtomicBool::new(true));
            #[cfg(target_os = "vita")]
            {
                lock_shell(true);
                let flag = alive.clone();
                tokio::spawn(async move {
                    while flag.load(Ordering::Relaxed) {
                        tick();
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                });
                tick();
            }
            Self { alive }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            self.alive.store(false, Ordering::Relaxed);
            #[cfg(target_os = "vita")]
            lock_shell(false);
        }
    }

    #[cfg(target_os = "vita")]
    fn tick() {
        use vitasdk_sys::*;
        unsafe {
            sceKernelPowerTick(SCE_KERNEL_POWER_TICK_DISABLE_AUTO_SUSPEND);
            sceKernelPowerTick(SCE_KERNEL_POWER_TICK_DISABLE_OLED_OFF);
            sceKernelPowerTick(SCE_KERNEL_POWER_TICK_DISABLE_OLED_DIMMING);
        }
    }

    #[cfg(target_os = "vita")]
    fn lock_shell(lock: bool) {
        use std::sync::Once;
        use vitasdk_sys::*;
        static INIT: Once = Once::new();
        INIT.call_once(|| unsafe {
            sceShellUtilInitEvents(0);
        });
        let types = [
            SCE_SHELL_UTIL_LOCK_TYPE_PS_BTN,
            SCE_SHELL_UTIL_LOCK_TYPE_POWEROFF_MENU,
        ];
        for ty in types {
            unsafe {
                if lock {
                    sceShellUtilLock(ty);
                } else {
                    sceShellUtilUnlock(ty);
                }
            }
        }
    }
}
const MANUAL_DIR: &str = "ux0:data/vitaforge/manual";
const DOWNLOADS: [&str; 3] = [TEMP_VPK, LEGACY_TEMP_DATA, TEMP_PKG];
fn sidecar(path: &str) -> String {
    format!("{path}.url")
}
fn discard_downloads() {
    for path in DOWNLOADS {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(sidecar(path));
        download::remove_parts(Path::new(path));
    }
}
/// A partial file is only resumed when it was started for this exact url;
/// anything else is another download's leftovers.
fn claim_partial(dest: &Path, url: &str) {
    let note = sidecar(&dest.to_string_lossy());
    if std::fs::read_to_string(&note).is_ok_and(|saved| saved == url) {
        return;
    }
    let _ = std::fs::remove_file(dest);
    download::remove_parts(dest);
    let _ = std::fs::write(&note, url);
}
/// A download that arrived whole but would not unpack is moved somewhere safe
/// instead of being thrown away, so it can be unpacked by hand.
fn preserve_for_manual(entry: &AppEntry) -> Option<String> {
    let label = if entry.titleid.trim().is_empty() {
        entry.id.trim()
    } else {
        entry.titleid.trim()
    };
    let dir = format!("{MANUAL_DIR}/{label}");
    let mut moved = false;
    for path in DOWNLOADS {
        if !Path::new(path).exists() {
            continue;
        }
        let _ = std::fs::remove_file(sidecar(path));
        if std::fs::create_dir_all(&dir).is_err() {
            break;
        }
        let name = Path::new(path).file_name()?.to_string_lossy().into_owned();
        let target = format!("{dir}/{name}");
        let _ = std::fs::remove_file(&target);
        if std::fs::rename(path, &target).is_ok() {
            moved = true;
        }
    }
    moved.then_some(dir)
}
/// A plain-text report of a failed install, saved where it can be sent along.
pub fn write_report(entry: &AppEntry, reason: &str) -> Result<String> {
    let label = if entry.titleid.trim().is_empty() {
        entry.id.trim()
    } else {
        entry.titleid.trim()
    };
    let dir = "ux0:data/vitaforge/reports";
    std::fs::create_dir_all(dir).context("couldn't create the reports folder")?;
    let path = format!("{dir}/{label}.txt");
    let free = crate::app::sysinfo::storage("ux0:")
        .map(|(used, total)| crate::data::cache_manager::format_bytes(total.saturating_sub(used)))
        .unwrap_or_else(|| "unknown".to_owned());
    let body = format!(
        "VitaForge {} install report\napp: {} ({})\ntitle id: {}\nversion: {}\nplatform: {:?}\nsource: {}\nurl: {}\nfree space on ux0: {}\nerror: {}\n\nlog: ux0:data/vitaforge/vitaforge.log\n",
        env!("CARGO_PKG_VERSION"),
        entry.name,
        entry.id,
        entry.titleid,
        entry.version,
        entry.platform,
        entry.source_catalog,
        entry.download_url,
        free,
        reason,
    );
    std::fs::write(&path, body).context("couldn't write the report")?;
    log_file(&format!("Report written to {path} for '{}'", entry.name));
    Ok(path)
}
pub fn start(entry: AppEntry) -> (watch::Receiver<Progress>, Arc<AtomicBool>) {
    log_file(&format!(
        "Install requested for '{}' (platform: {:?}, url: '{}')",
        entry.name, entry.platform, entry.download_url
    ));
    let (tx, rx) = watch::channel(Progress::Resolving);
    let tx_failure = tx.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = cancel.clone();
    // Own thread at default (UI-level) priority like VitaDB-Downloader: on the
    // below-UI tokio workers the TLS decrypt starved behind the 60Hz repaint.
    let install = async move {
        let _awake = keep_awake::Guard::new();
        let result = run(&entry, &tx, &cancel_worker).await;
        let final_state = match result {
            Ok(state) => {
                log_file(&format!(
                    "Install completed successfully for '{}': {:?}",
                    entry.name, state
                ));
                state
            }
            Err(err) => {
                log_file(&format!("Install FAILED for '{}': {err:#}", entry.name));
                eprintln!("install failed: {err:#}");
                let cancelled = err.to_string() == CANCELLED_MESSAGE;
                let last = tx.borrow().clone();
                let mid_download = matches!(
                    last,
                    Progress::Resolving
                        | Progress::Downloading { .. }
                        | Progress::DownloadingData { .. }
                );
                let mut message = format!("{err}");
                let _ = std::fs::remove_dir_all(EXTRACT_DIR);
                let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
                if mid_download && !cancelled {
                    // Keep the partial file: the next try resumes where this one stopped.
                } else if matches!(last, Progress::Extracting { .. }) && !cancelled {
                    if let Some(dir) = preserve_for_manual(&entry) {
                        message.push_str(&format!(" - file kept in {dir}"));
                    }
                } else {
                    discard_downloads();
                }
                installed::clear_pending_install(&installed::index_key(&entry));
                Progress::Failed(message)
            }
        };
        let _ = tx.send(final_state);
    };
    std::thread::Builder::new()
        .name("install".into())
        .stack_size(1024 * 1024)
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build the install runtime")
                    .block_on(install)
            }));
            if result.is_err() && !tx_failure.borrow().is_finished() {
                log_file("Install thread panicked; see ux0:data/vitaforge/panic.log");
                let _ = tx_failure.send(Progress::Failed(
                    "installer stopped unexpectedly; see ux0:data/vitaforge/panic.log".to_owned(),
                ));
            }
        })
        .expect("failed to spawn the install thread");
    (rx, cancel)
}
async fn run(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<Progress> {
    match entry.platform {
        crate::data::Platform::Vita if is_self(entry) => run_self_update(entry, tx, cancel).await,
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
async fn install_pbp_pkg(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    pkg_path: &Path,
    kind: pkg::PkgKind,
) -> Result<Progress> {
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
    let disc_id =
        pbp::read_disc_id(&eboot_path).unwrap_or_else(|| entry.titleid.trim().to_string());
    let final_id = if disc_id.len() >= 9 {
        &disc_id[..9]
    } else {
        entry.titleid.trim()
    };

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
        std::fs::create_dir_all("ux0:pspemu/PSP/LICENSE")
            .context("couldn't create the PSP license folder")?;
        std::fs::write(
            format!("ux0:pspemu/PSP/LICENSE/{content_id}.rif"),
            &psp_license,
        )
        .context("couldn't write the PSP license file")?;
    }
    installed::stamp_pending_install(
        &installed::index_key(entry),
        &entry.hash,
        &entry.version,
        None,
    );
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
    queue_livearea_install(
        &entry.name,
        &entry.download_url,
        license,
        bgdl::BGDL_TYPE_GAME,
    )
    .await
    .context("Failed to queue background download (BGDL)")?;
    installed::stamp_pending_install(
        &installed::index_key(entry),
        &entry.hash,
        &entry.version,
        None,
    );
    Ok(Progress::Queued)
}
async fn run_psp(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<Progress> {
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
    let _ = tx.send(Progress::Extracting { done: 0, total: 0, file: 0, files: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(Path::new(TEMP_VPK), &dest, &cancel_worker, |done, total, file, files| {
            let _ = tx_worker.send(Progress::Extracting { done, total, file, files });
        })
    })
    .await
    .context("the extract worker crashed")??;
    log_file("vpk extracted, installing");
    let _ = std::fs::remove_file(TEMP_VPK);
    installed::stamp_pending_install(
        &installed::index_key(entry),
        &entry.hash,
        &entry.version,
        None,
    );
    Ok(Progress::Done)
}
async fn stage_bgdl_icon(entry: &AppEntry) {
    let Some(url) = entry.icon_url.as_deref().or(entry.cover_url.as_deref()) else {
        return;
    };
    let request = crate::net::client()
        .get(url)
        .header("User-Agent", "VitaForge")
        .send();
    let Ok(response) = request.await else { return };
    let Ok(bytes) = response.bytes().await else {
        return;
    };
    let _ = std::fs::create_dir_all("ux0:data/vitaforge");
    let _ = std::fs::write("ux0:data/vitaforge/bgdl_icon.png", &bytes);
}
async fn run_plugin(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this plugin");
    }
    let Some(plan) = plugin_install_plan(entry)? else {
        bail!(PLUGIN_PENDING_MESSAGE);
    };
    std::fs::create_dir_all(PLUGIN_DIR).context("couldn't create the plugins folder")?;
    let downloaded = PathBuf::from(PLUGIN_DIR).join("download.tmp");
    let _ = std::fs::remove_file(&downloaded);
    download(&entry.download_url, &downloaded, tx, cancel).await?;
    check_cancelled(cancel)?;
    let result = place_plugin_files(&downloaded, &plan, cancel);
    let _ = std::fs::remove_file(&downloaded);
    let _ = std::fs::remove_dir_all(PathBuf::from(PLUGIN_DIR).join("unpacked"));
    result?;
    if update_tai_config(&plan)? {
        log_file(&format!(
            "Activated plugin '{}' in {}: {:?}",
            entry.name,
            plan.config_path.display(),
            plan.lines
        ));
    }
    Ok(Progress::RebootRequired)
}

fn place_plugin_files(downloaded: &Path, plan: &PluginInstallPlan, cancel: &AtomicBool) -> Result<()> {
    let mut magic = [0u8; 4];
    let is_zip = std::fs::File::open(downloaded)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic))
        .is_ok()
        && magic == *b"PK\x03\x04";
    let sources: Vec<PathBuf> = if is_zip {
        let unpacked = PathBuf::from(PLUGIN_DIR).join("unpacked");
        let _ = std::fs::remove_dir_all(&unpacked);
        extract(downloaded, &unpacked, cancel, |_, _, _, _| {})?;
        let mut found = Vec::new();
        collect_files(&unpacked, &unpacked, &mut found)?;
        let found: Vec<PathBuf> = found.into_iter().map(|relative| unpacked.join(relative)).collect();
        plan.binaries
            .iter()
            .map(|binary| {
                let name = binary.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                found
                    .iter()
                    .find(|f| f.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == name))
                    .cloned()
                    .with_context(|| format!("the plugin archive has no {name}"))
            })
            .collect::<Result<_>>()?
    } else if plan.binaries.len() == 1 {
        vec![downloaded.to_path_buf()]
    } else {
        bail!("the download is a single file but this plugin needs {} modules", plan.binaries.len());
    };
    for (source, binary) in sources.iter().zip(&plan.binaries) {
        if let Some(parent) = binary.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(binary);
        std::fs::copy(source, binary).with_context(|| format!("couldn't write {}", binary.display()))?;
    }
    Ok(())
}

struct PluginInstallPlan {
    binaries: Vec<PathBuf>,
    config_path: PathBuf,
    lines: Vec<(String, String)>,
}

fn plugin_install_plan(entry: &AppEntry) -> Result<Option<PluginInstallPlan>> {
    let (Some(path), Some(sections), Some(lines)) = (
        entry.plugin_install_path.as_deref(),
        entry.plugin_config_section.as_deref(),
        entry.plugin_config_line.as_deref(),
    ) else {
        return Ok(None);
    };
    parse_plugin_plan(path, sections, lines)
}

fn parse_plugin_plan(path: &str, sections: &str, lines: &str) -> Result<Option<PluginInstallPlan>> {
    if sections.trim().eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let sections: Vec<&str> = sections.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let lines: Vec<&str> = lines.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if sections.is_empty() || sections.len() != lines.len() {
        bail!("the plugin's taiHEN sections and lines don't match");
    }
    let install_dir = validated_tai_dir(path)?;
    let mut plan = PluginInstallPlan {
        binaries: Vec::new(),
        config_path: active_tai_config(),
        lines: Vec::new(),
    };
    for (section, line) in sections.into_iter().zip(lines) {
        if !section.starts_with('*') || section.len() > 64 || section.split_whitespace().count() != 1 {
            bail!("invalid taiHEN config section");
        }
        let binary = validated_tai_path(line)?;
        if !binary.starts_with(&install_dir) {
            bail!("plugin config line is outside its declared install path");
        }
        if !plan.binaries.contains(&binary) {
            plan.binaries.push(binary);
        }
        plan.lines.push((section.to_owned(), line.to_owned()));
    }
    Ok(Some(plan))
}

fn active_tai_config() -> PathBuf {
    let ux0 = PathBuf::from("ux0:tai/config.txt");
    if ux0.exists() { ux0 } else { PathBuf::from("ur0:tai/config.txt") }
}

fn validated_tai_dir(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.contains(['\r', '\n']) || path.split(['/', '\\']).any(|part| part == "..") {
        bail!("invalid plugin install path");
    }
    let allowed = path.starts_with("ur0:tai/") || path.starts_with("ux0:tai/");
    if !allowed {
        bail!("plugin install path must be inside ur0:tai or ux0:tai");
    }
    Ok(PathBuf::from(path))
}

fn validated_tai_path(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.contains(['\r', '\n', '#']) || path.split(['/', '\\']).any(|part| part == "..") {
        bail!("invalid plugin config line");
    }
    if !(path.starts_with("ur0:tai/") || path.starts_with("ux0:tai/")) {
        bail!("plugin config line must point inside tai");
    }
    if !(path.ends_with(".skprx") || path.ends_with(".suprx")) {
        bail!("plugin config line must target a .skprx or .suprx file");
    }
    Ok(PathBuf::from(path))
}

fn update_tai_config(plan: &PluginInstallPlan) -> Result<bool> {
    let existing = match std::fs::read_to_string(&plan.config_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).context("couldn't read taiHEN config.txt"),
    };
    let mut updated = existing.clone();
    for (section, line) in &plan.lines {
        if let Some(next) = updated_tai_config(&updated, section, line)? {
            updated = next;
        }
    }
    if updated == existing {
        return Ok(false);
    }
    if plan.config_path.exists() {
        let backup = PathBuf::from(format!(
            "{}{}",
            plan.config_path.display(),
            TAI_CONFIG_BACKUP_SUFFIX
        ));
        if !backup.exists() {
            std::fs::copy(&plan.config_path, &backup)
                .context("couldn't back up taiHEN config.txt")?;
        }
    }
    let temporary = PathBuf::from(format!("{}.vitaforge.tmp", plan.config_path.display()));
    std::fs::write(&temporary, updated).context("couldn't write updated taiHEN config")?;
    std::fs::rename(&temporary, &plan.config_path).context("couldn't replace taiHEN config.txt")?;
    Ok(true)
}

fn updated_tai_config(existing: &str, section: &str, line: &str) -> Result<Option<String>> {
    let mut current_section = None;
    for current in existing.lines().map(str::trim) {
        if current.starts_with('*') {
            current_section = Some(current);
        } else if current == line && current_section == Some(section) {
            return Ok(None);
        }
    }
    let line_ending = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut offset = 0;
    let mut insertion = None;
    let mut in_section = false;
    for current in existing.split_inclusive('\n') {
        let trimmed = current.trim();
        if trimmed == section {
            in_section = true;
        } else if in_section && trimmed.starts_with('*') {
            insertion = Some(offset);
            break;
        }
        offset += current.len();
    }
    if in_section && insertion.is_none() {
        insertion = Some(existing.len());
    }
    let mut updated = existing.to_owned();
    if let Some(offset) = insertion {
        let prefix = if offset > 0 && !updated[..offset].ends_with('\n') {
            line_ending
        } else {
            ""
        };
        updated.insert_str(offset, &format!("{prefix}{line}{line_ending}"));
    } else {
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push_str(line_ending);
        }
        updated.push_str(section);
        updated.push_str(line_ending);
        updated.push_str(line);
        updated.push_str(line_ending);
    }
    Ok(Some(updated))
}
/// vitaForge itself: the system promoter refuses to replace an app that is open,
/// so it cannot go through `run_vita`.
pub fn is_self(entry: &AppEntry) -> bool {
    entry.titleid.trim().eq_ignore_ascii_case(SELF_TITLE_ID)
}
const SELF_TITLE_ID: &str = "VITAFORGE";
const SELF_APP_DIR: &str = "ux0:app/VITAFORGE";
const SELF_STAGE_DIR: &str = "ux0:data/vitaforge/self_update";
const SELF_BACKUP_DIR: &str = "ux0:data/vitaforge/self_backup";
/// Files that make the app start: written last, so an interrupted update leaves
/// the old, working pair in place instead of a new eboot with old assets.
const SELF_KEY_FILES: [&str; 2] = ["eboot.bin", "sce_sys/param.sfo"];
/// The way VitaDB Downloader updates itself: unpack the new build, release the
/// app's own folder, copy the files over it and restart into them.
async fn run_self_update(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    let url = match entry.source.as_deref() {
        Some(repo) => github::latest_release(repo)
            .await
            .map(|release| release.vpk_url)
            .unwrap_or_else(|| entry.download_url.clone()),
        None => entry.download_url.clone(),
    };
    if url.is_empty() {
        bail!("no download link for this update");
    }
    check_free_space(entry.size_bytes, 0, crate::app::sysinfo::storage("ux0:"))?;
    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    let _ = std::fs::remove_dir_all(SELF_STAGE_DIR);
    download(&url, Path::new(TEMP_VPK), tx, cancel).await?;
    let _ = tx.send(Progress::Extracting { done: 0, total: 0, file: 0, files: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(
            Path::new(TEMP_VPK),
            Path::new(SELF_STAGE_DIR),
            &cancel_worker,
            |done, total, file, files| {
                let _ = tx_worker.send(Progress::Extracting { done, total, file, files });
            },
        )
    })
    .await
    .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);
    let _ = std::fs::remove_file(sidecar(TEMP_VPK));
    for required in SELF_KEY_FILES {
        if !Path::new(SELF_STAGE_DIR).join(required).is_file() {
            let _ = std::fs::remove_dir_all(SELF_STAGE_DIR);
            bail!("the update package has no {required}");
        }
    }
    check_cancelled(cancel)?;
    // Past this point the update is not abortable: a half-copied app would not start.
    let _ = tx.send(Progress::Installing { elapsed_secs: 0 });
    let applied = tokio::task::spawn_blocking(|| {
        lifecycle::release_own_folder();
        apply_self_update(
            Path::new(SELF_STAGE_DIR),
            Path::new(SELF_APP_DIR),
            Path::new(SELF_BACKUP_DIR),
        )
    })
    .await
    .context("the update worker crashed")?;
    let _ = std::fs::remove_dir_all(SELF_STAGE_DIR);
    applied?;
    installed::stamp_pending_install(
        &installed::index_key(entry),
        &entry.hash,
        &entry.version,
        Some(Path::new(SELF_APP_DIR)),
    );
    Ok(Progress::Restarting)
}
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        if item.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_path_buf());
        }
    }
    Ok(())
}
fn copy_over(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to).with_context(|| format!("couldn't write {}", to.display()))?;
    Ok(())
}
fn apply_self_update(stage: &Path, dest: &Path, backup: &Path) -> Result<()> {
    let mut files = Vec::new();
    collect_files(stage, stage, &mut files)?;
    let is_key = |path: &Path| SELF_KEY_FILES.iter().any(|key| path == Path::new(key));
    let _ = std::fs::remove_dir_all(backup);
    for key in SELF_KEY_FILES {
        let current = dest.join(key);
        if current.is_file() {
            copy_over(&current, &backup.join(key))?;
        }
    }
    let result = (|| -> Result<()> {
        for relative in files.iter().filter(|path| !is_key(path)) {
            copy_over(&stage.join(relative), &dest.join(relative))?;
        }
        for key in SELF_KEY_FILES {
            copy_over(&stage.join(key), &dest.join(key))?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        for key in SELF_KEY_FILES {
            let saved = backup.join(key);
            if saved.is_file() {
                let _ = copy_over(&saved, &dest.join(key));
            }
        }
        return Err(err).context("couldn't update vitaForge, the previous version was restored");
    }
    let _ = std::fs::remove_dir_all(backup);
    Ok(())
}
async fn run_vita(
    entry: &AppEntry,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<Progress> {
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
        let data_dest = entry
            .data_extract_path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(sanitize_data_dest)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from(DATA_ROOT));
        stage_data_and_vpk(data_url, &data_dest, &temp_data_path(entry), &url, tx, cancel).await?;
        log_file("game data staged, extracting the vpk");
    } else {
        download(&url, Path::new(TEMP_VPK), tx, cancel).await?;
    }
    let _ = tx.send(Progress::Extracting { done: 0, total: 0, file: 0, files: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    tokio::task::spawn_blocking(move || {
        extract(
            Path::new(TEMP_VPK),
            Path::new(EXTRACT_DIR),
            &cancel_worker,
            |done, total, file, files| {
                let _ = tx_worker.send(Progress::Extracting { done, total, file, files });
            },
        )
    })
    .await
    .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);
    let _ = std::fs::remove_file(sidecar(TEMP_VPK));
    log_file("vpk extracted");
    if url == entry.download_url {
        check_digest(entry, Path::new(EXTRACT_DIR)).await?;
    }
    installed::stamp_pending_install(
        &installed::index_key(entry),
        &entry.hash,
        &entry.version,
        Some(Path::new(EXTRACT_DIR)),
    );
    head::write(Path::new(EXTRACT_DIR))?;
    let titleid = entry.titleid.trim().to_uppercase();
    if !titleid.is_empty() {
        for root in &["ux0:app", "ur0:app", "uma0:app"] {
            let path = format!("{root}/{titleid}");
            if installed::vita_fs::exists(&path) || Path::new(&path).exists() {
                let path_owned = path.clone();
                let removed =
                    tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&path_owned)).await;
                if let Ok(Err(err)) = removed {
                    eprintln!("couldn't clear the previous install at {path}: {err}");
                }
            }
        }
    }
    let _ = tx.send(Progress::Installing { elapsed_secs: 0 });
    let tx_worker = tx.clone();
    log_file("promoting the package");
    tokio::task::spawn_blocking(move || promoter::promote_package(EXTRACT_DIR, &tx_worker))
        .await
        .context("the install worker crashed")??;
    log_file("package promoted");
    if Path::new(EXTRACT_DIR).exists() {
        let _ = std::fs::remove_dir_all(EXTRACT_DIR);
        bail!("the system didn't accept the package");
    }
    Ok(Progress::Done)
}
/// Compares the unpacked eboot with the digest the catalog published, so a
/// corrupt or swapped download is refused before it replaces a working app.
async fn check_digest(entry: &AppEntry, dir: &Path) -> Result<()> {
    let expected = entry.hash.trim().to_lowercase();
    if expected.len() != installed::HASH_LEN {
        return Ok(());
    }
    let eboot = dir.join("eboot.bin");
    let actual = tokio::task::spawn_blocking(move || installed::md5_file(&eboot))
        .await
        .ok()
        .flatten();
    match actual {
        Some(actual) if actual == expected => {
            log_file(&format!("Digest verified for '{}'", entry.name));
            Ok(())
        }
        Some(actual) => {
            log_file(&format!(
                "Digest MISMATCH for '{}': catalog {expected}, downloaded {actual}",
                entry.name
            ));
            bail!("the download doesn't match the catalog digest; the catalog may be updating, try again later")
        }
        None => {
            log_file(&format!("Digest check skipped for '{}': no eboot.bin", entry.name));
            Ok(())
        }
    }
}
async fn stage_data_and_vpk(
    data_url: &str,
    data_dest: &Path,
    dest: &Path,
    vpk_url: &str,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("couldn't create {}", dir.display()))?;
    }
    adopt_legacy_data(dest, data_url);
    claim_partial(dest, data_url);
    let started = std::time::Instant::now();
    let before = download::downloaded_len(dest);
    let fetched = download::fetch_parts(data_url, dest, tx, |received, total, elapsed_secs| {
        Progress::DownloadingData {
            received,
            total,
            elapsed_secs,
        }
    }, cancel)
    .await;
    let bytes = download::downloaded_len(dest).saturating_sub(before);
    let secs = started.elapsed().as_secs_f64().max(0.001);
    log_file(&format!(
        "download {data_url}: {} in {secs:.0}s = {:.0} KB/s ({})",
        crate::data::cache_manager::format_bytes(bytes),
        bytes as f64 / 1024.0 / secs,
        if fetched.is_ok() { "ok" } else { "failed" },
    ));
    let md5 = fetched.context("couldn't download the required game data")?;
    let extract_started = std::time::Instant::now();
    let _ = tx.send(Progress::Extracting { done: 0, total: 0, file: 0, files: 0 });
    let tx_worker = tx.clone();
    let cancel_worker = cancel.clone();
    let data_dest_owned = data_dest.to_path_buf();
    let dest_owned = dest.to_path_buf();
    let extract_task = tokio::task::spawn_blocking(move || {
        let parts = download::PartsReader::open(&dest_owned)
            .context("couldn't reopen the downloaded game data")?;
        let mut written = Vec::new();
        let mut hashing = download::HashingReader::new(parts);
        let extracted = extract_from(
            &mut hashing,
            &data_dest_owned,
            &cancel_worker,
            &mut written,
            |done, total, file, files| {
                let _ = tx_worker.send(Progress::Extracting { done, total, file, files });
            },
        );
        let verdict = match (&extracted, md5) {
            (Ok(()), Some(expected)) => {
                let actual = hashing.finish(|percent| {
                    if cancel_worker.load(Ordering::Relaxed) {
                        return Err(std::io::Error::other(CANCELLED_MESSAGE));
                    }
                    let _ = tx_worker.send(Progress::Finishing { step: "Verifying", percent });
                    Ok(())
                });
                log_file(&format!("md5 {}", if actual.as_ref().is_ok_and(|value| value == &expected) { "ok" } else { "failed" }));
                match actual {
                    Ok(actual) if actual == expected => Ok(()),
                    Ok(_) => Err(anyhow::anyhow!("the game data was corrupted (md5 mismatch), please try again")),
                    Err(err) => Err(err.into()),
                }
            }
            _ => extracted,
        };
        if verdict.is_err() {
            for path in &written {
                let _ = std::fs::remove_file(path);
            }
            if !matches!(&verdict, Err(e) if e.to_string() == CANCELLED_MESSAGE) {
                discard_data_download(&dest_owned);
            }
        }
        if verdict.is_ok() {
            let _ = tx_worker.send(Progress::VerificationComplete);
        }
        log_file(&format!("game data worker: releasing {} tracked file paths", written.len()));
        drop(written);
        log_file(&format!("game data worker done: {}", if verdict.is_ok() { "ok" } else { "failed" }));
        verdict
    });
    // Overlap VPK network download with Game Data ZIP extraction on disk
    let vpk_res = download(vpk_url, Path::new(TEMP_VPK), tx, cancel).await;
    log_file(&format!("vpk download {}, waiting for game data", if vpk_res.is_ok() { "ok" } else { "failed" }));
    log_file("awaiting the game data worker");
    let extract_res = extract_task.await.context("the data extract worker crashed")?;
    log_file(&format!("game data extract + md5: {:.0}s", extract_started.elapsed().as_secs_f64()));
    extract_res?;
    discard_data_download(dest);
    vpk_res
}
/// Moves an old single-slot download of this same url into the per-game folder
/// (keeps a half-done 3 GB download); any other legacy leftovers are deleted.
fn adopt_legacy_data(dest: &Path, data_url: &str) {
    let legacy = Path::new(LEGACY_TEMP_DATA);
    let note = sidecar(LEGACY_TEMP_DATA);
    if std::fs::read_to_string(&note).is_ok_and(|saved| saved == data_url) && !dest.exists() {
        for i in 0..download::PARALLEL_PARTS {
            let _ = std::fs::rename(download::part_path(legacy, i), download::part_path(dest, i));
        }
        let _ = std::fs::rename(&note, sidecar(&dest.to_string_lossy()));
    }
    discard_data_download(legacy);
}
fn discard_data_download(dest: &Path) {
    let _ = std::fs::remove_file(dest);
    let _ = std::fs::remove_file(sidecar(&dest.to_string_lossy()));
    download::remove_parts(dest);
    if let Some(dir) = dest.parent().filter(|d| d.starts_with(TEMP_DATA_DIR)) {
        let _ = std::fs::remove_dir(dir);
    }
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
async fn download(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<Progress>,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    download_with(url, dest, tx, cancel, |received, total, elapsed_secs| {
        Progress::Downloading {
            received,
            total,
            elapsed_secs,
        }
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
    claim_partial(dest, url);
    let started = std::time::Instant::now();
    let before = download::downloaded_len(dest);
    let result = download::fetch_file(url, dest, tx, progress, cancel).await;
    let bytes = download::downloaded_len(dest).saturating_sub(before);
    let secs = started.elapsed().as_secs_f64().max(0.001);
    log_file(&format!(
        "download {url}: {} in {secs:.0}s = {:.0} KB/s ({})",
        crate::data::cache_manager::format_bytes(bytes),
        bytes as f64 / 1024.0 / secs,
        if result.is_ok() { "ok" } else { "failed" },
    ));
    result
}
fn extract(
    archive: &Path,
    dest: &Path,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u32, u32, u32, u32),
) -> Result<()> {
    let mut file = std::fs::File::open(archive).context("couldn't reopen the downloaded file")?;
    extract_from(&mut file, dest, cancel, &mut Vec::new(), on_progress)
}
/// Unpacks a zip read from `source` into `dest`, recording every file it
/// creates in `written` so a failed or rejected extract can be rolled back.
fn extract_from(
    source: &mut (impl std::io::Read + std::io::Seek),
    dest: &Path,
    cancel: &AtomicBool,
    written: &mut Vec<PathBuf>,
    mut on_progress: impl FnMut(u32, u32, u32, u32),
) -> Result<()> {
    match extract_streaming(source, dest, cancel, written, &mut on_progress) {
        Err(err) if err.downcast_ref::<zip::result::ZipError>().is_some_and(|e| matches!(e, zip::result::ZipError::UnsupportedArchive(_))) => {
            log_file(&format!("streaming extract unsupported ({err}), using the zip index"));
            source.seek(std::io::SeekFrom::Start(0))?;
        }
        other => return other,
    }
    extract_indexed(source, dest, cancel, written, on_progress)
}
fn extract_streaming(
    source: &mut (impl std::io::Read + std::io::Seek),
    dest: &Path,
    cancel: &AtomicBool,
    written: &mut Vec<PathBuf>,
    on_progress: &mut impl FnMut(u32, u32, u32, u32),
) -> Result<()> {
    use std::io::{Read, SeekFrom, Write};
    struct Counted<'a, R>(R, &'a std::cell::Cell<u64>);
    impl<R: Read> Read for Counted<'_, R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.0.read(buf)?;
            self.1.set(self.1.get() + n as u64);
            Ok(n)
        }
    }
    struct Entry {
        name: String,
        method: u16,
        crc: u32,
        compressed: u64,
        size: u64,
        offset: u64,
    }
    let unsupported = |why: &'static str| anyhow::Error::from(zip::result::ZipError::UnsupportedArchive(why));
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    const MB: u64 = 1024 * 1024;

    let total_bytes = source.seek(SeekFrom::End(0))?;
    let tail_len = total_bytes.min(64 * 1024 + 22);
    source.seek(SeekFrom::Start(total_bytes - tail_len))?;
    let mut tail = vec![0u8; tail_len as usize];
    source.read_exact(&mut tail)?;
    let eocd = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
        .ok_or_else(|| anyhow::anyhow!("the download isn't a valid zip"))?;
    let (count, cd_size, cd_offset) = (u16_at(&tail, eocd + 10), u32_at(&tail, eocd + 12), u32_at(&tail, eocd + 16));
    if count == 0xFFFF || cd_size == u32::MAX || cd_offset == u32::MAX {
        return Err(unsupported("zip64"));
    }
    source.seek(SeekFrom::Start(cd_offset as u64))?;
    let mut cd = vec![0u8; cd_size as usize];
    source.read_exact(&mut cd)?;
    let mut entries = Vec::with_capacity(count as usize);
    let mut at = 0;
    while at + 46 <= cd.len() && u32_at(&cd, at) == 0x0201_4b50 {
        let (name_len, extra_len, comment_len) = (u16_at(&cd, at + 28) as usize, u16_at(&cd, at + 30) as usize, u16_at(&cd, at + 32) as usize);
        let (compressed, size, offset) = (u32_at(&cd, at + 20), u32_at(&cd, at + 24), u32_at(&cd, at + 42));
        if u16_at(&cd, at + 8) & 1 != 0 {
            return Err(unsupported("encrypted entry"));
        }
        if compressed == u32::MAX || size == u32::MAX || offset == u32::MAX {
            return Err(unsupported("zip64 entry"));
        }
        let name = cd.get(at + 46..at + 46 + name_len).ok_or_else(|| anyhow::anyhow!("the zip index is truncated"))?;
        entries.push(Entry {
            name: String::from_utf8_lossy(name).into_owned(),
            method: u16_at(&cd, at + 10),
            crc: u32_at(&cd, at + 16),
            compressed: compressed as u64,
            size: size as u64,
            offset: offset as u64,
        });
        at += 46 + name_len + extra_len + comment_len;
    }
    if entries.len() != count as usize {
        return Err(unsupported("central directory doesn't match its entry count"));
    }
    if let Some(entry) = entries.iter().find(|e| e.method != 0 && e.method != 8) {
        log_file(&format!("zip entry {} uses compression method {}", entry.name, entry.method));
        return Err(unsupported("compression method other than store/deflate"));
    }
    entries.sort_by_key(|e| e.offset);
    let files = entries.iter().filter(|e| !e.name.ends_with('/')).count() as u32;
    let total = total_bytes.div_ceil(MB) as u32;

    source.seek(SeekFrom::Start(0))?;
    let consumed = std::cell::Cell::new(0u64);
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, Counted(&mut *source, &consumed));
    let mut pos = 0u64;
    let mut last_reported = std::time::Instant::now();
    let mut created_dirs = std::collections::HashSet::new();
    let mut file = 0u32;
    enum Chunk {
        Open(PathBuf),
        Data(Vec<u8>),
    }
    let (tx, rx) = std::sync::mpsc::sync_channel::<Chunk>(8);
    std::thread::scope(|scope| {
        let writer = scope.spawn(move || -> Result<()> {
            let mut out = None;
            for chunk in rx {
                match chunk {
                    Chunk::Open(path) => {
                        out = Some(std::fs::File::create(&path)
                            .with_context(|| format!("couldn't write {}", path.display()))?);
                    }
                    Chunk::Data(data) => {
                        if let Some(out) = out.as_mut() {
                            out.write_all(&data)?;
                        }
                    }
                }
            }
            Ok(())
        });
        let read_result = (|| -> Result<()> {
            for entry in &entries {
                check_cancelled(cancel)?;
                let gap = entry.offset.checked_sub(pos).ok_or_else(|| unsupported("overlapping zip entries"))?;
                std::io::copy(&mut (&mut reader).take(gap), &mut std::io::sink())?;
                let mut header = [0u8; 30];
                reader.read_exact(&mut header)?;
                if u32_at(&header, 0) != 0x0403_4b50 {
                    bail!("the zip is corrupted (bad local header for {})", entry.name);
                }
                let skip = u16_at(&header, 26) as u64 + u16_at(&header, 28) as u64;
                std::io::copy(&mut (&mut reader).take(skip), &mut std::io::sink())?;
                pos = entry.offset + 30 + skip + entry.compressed;

                let relative = Path::new(&entry.name);
                if !relative.components().all(|c| matches!(c, std::path::Component::Normal(_))) {
                    bail!("the zip has an unsafe path: {}", entry.name);
                }
                let out_path = dest.join(relative);
                let raw = (&mut reader).take(entry.compressed);
                if entry.name.ends_with('/') {
                    std::io::copy(&mut { raw }, &mut std::io::sink())?;
                    if created_dirs.insert(out_path.clone()) {
                        std::fs::create_dir_all(&out_path)?;
                    }
                    continue;
                }
                if let Some(parent) = out_path.parent()
                    && !created_dirs.contains(parent)
                {
                    std::fs::create_dir_all(parent)?;
                    created_dirs.insert(parent.to_path_buf());
                }
                written.push(out_path.clone());
                file += 1;
                if tx.send(Chunk::Open(out_path)).is_err() {
                    return Ok(());
                }
                let mut data: Box<dyn Read + '_> = if entry.method == 8 {
                    Box::new(flate2::bufread::DeflateDecoder::new(std::io::BufReader::with_capacity(64 * 1024, raw)))
                } else {
                    Box::new(raw)
                };
                let mut crc = crc32fast::Hasher::new();
                let mut size = 0u64;
                loop {
                    let mut buf = vec![0u8; 256 * 1024];
                    let n = data.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    buf.truncate(n);
                    crc.update(&buf);
                    size += n as u64;
                    if tx.send(Chunk::Data(buf)).is_err() {
                        return Ok(());
                    }
                    if last_reported.elapsed() >= std::time::Duration::from_millis(250) {
                        check_cancelled(cancel)?;
                        last_reported = std::time::Instant::now();
                        on_progress((consumed.get() / MB) as u32, total, file, files);
                    }
                }
                std::io::copy(&mut data, &mut std::io::sink())?;
                if size != entry.size || crc.finalize() != entry.crc {
                    bail!("the game data was corrupted ({} failed its crc check), please try again", entry.name);
                }
            }
            Ok(())
        })();
        drop(tx);
        let write_result = writer.join().map_err(|_| anyhow::anyhow!("the extract writer crashed"))?;
        read_result.and(write_result)
    })?;
    on_progress(total, total, files, files);
    Ok(())
}
fn extract_indexed(
    source: &mut (impl std::io::Read + std::io::Seek),
    dest: &Path,
    cancel: &AtomicBool,
    written: &mut Vec<PathBuf>,
    mut on_progress: impl FnMut(u32, u32, u32, u32),
) -> Result<()> {
    use std::io::{Read, Write};
    // ZIP preparation visits every local header and returns to the index, discarding
    // the buffer twice per entry; keep read-ahead small (large reads bypass it).
    let buffered = std::io::BufReader::with_capacity(4 * 1024, source);
    let mut zip = zip::ZipArchive::new(buffered).context("the download isn't a valid vpk (zip)")?;
    // Progress is in MB written, so a single 1 GB entry still moves the bar.
    const MB: u64 = 1024 * 1024;
    let total_bytes = zip.decompressed_size().unwrap_or(0) as u64;
    let total = total_bytes.div_ceil(MB) as u32;
    let mut done_bytes = 0u64;
    let mut last_reported = std::time::Instant::now();
    let mut created_dirs = std::collections::HashSet::new();
    let mut copy_buf = vec![0u8; 256 * 1024];
    let files = zip.len() as u32;
    for index in 0..zip.len() {
        check_cancelled(cancel)?;
        if last_reported.elapsed() >= std::time::Duration::from_millis(250) {
            last_reported = std::time::Instant::now();
            on_progress((done_bytes / MB) as u32, total, index as u32, files);
        }
        let mut entry = zip.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out_path: PathBuf = dest.join(relative);
        if entry.is_dir() {
            if created_dirs.insert(out_path.clone()) {
                std::fs::create_dir_all(&out_path)?;
            }
            continue;
        }
        if let Some(parent) = out_path.parent()
            && !created_dirs.contains(parent)
        {
            std::fs::create_dir_all(parent)?;
            created_dirs.insert(parent.to_path_buf());
        }
        let mut out = std::fs::File::create(&out_path)
            .with_context(|| format!("couldn't write {}", out_path.display()))?;
        written.push(out_path.clone());
        loop {
            let n = entry.read(&mut copy_buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&copy_buf[..n])?;
            done_bytes += n as u64;
            if last_reported.elapsed() >= std::time::Duration::from_millis(250) {
                check_cancelled(cancel)?;
                last_reported = std::time::Instant::now();
                on_progress((done_bytes / MB) as u32, total, index as u32, files);
            }
        }
    }
    on_progress(total, total, files, files);
    Ok(())
}
#[cfg(test)]
mod parts_extract_tests {
    use super::*;
    use md5::{Digest, Md5};
    use std::io::Write;

    struct CountedReader<R> {
        inner: R,
        bytes: u64,
    }

    impl<R: std::io::Read> std::io::Read for CountedReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.bytes += n as u64;
            Ok(n)
        }
    }

    impl<R: std::io::Seek> std::io::Seek for CountedReader<R> {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn remaining_hash_reports_progress_and_can_be_cancelled() {
        let bytes = vec![7u8; 2 * 1024 * 1024];
        let mut progress = Vec::new();
        let digest = download::HashingReader::new(std::io::Cursor::new(&bytes))
            .finish(|percent| { progress.push(percent); Ok(()) })
            .unwrap();
        assert_eq!(digest, format!("{:x}", Md5::digest(&bytes)));
        assert_eq!(progress.first(), Some(&0));
        assert!(progress.contains(&50));
        assert_eq!(progress.last(), Some(&100));
        let err = download::HashingReader::new(std::io::Cursor::new(&bytes))
            .finish(|_| Err(std::io::Error::other(CANCELLED_MESSAGE)))
            .unwrap_err();
        assert_eq!(err.to_string(), CANCELLED_MESSAGE);
    }

    #[test]
    fn extracts_and_hashes_a_zip_split_into_parts() {
        let root = std::env::temp_dir().join(format!("vf_parts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut zip_bytes = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for i in 0..40u32 {
                w.start_file(format!("game/f{i}.bin"), opts).unwrap();
                w.write_all(&vec![i as u8; 997 * (i as usize + 1)]).unwrap();
            }
            w.finish().unwrap();
        }
        let dest = root.join("tmp_data.zip");
        let chunk = zip_bytes.len() / download::PARALLEL_PARTS;
        for i in 0..download::PARALLEL_PARTS {
            let end = if i + 1 == download::PARALLEL_PARTS { zip_bytes.len() } else { (i + 1) * chunk };
            std::fs::write(download::part_path(&dest, i), &zip_bytes[i * chunk..end]).unwrap();
        }
        let out = root.join("out");
        let mut source = CountedReader { inner: download::PartsReader::open(&dest).unwrap(), bytes: 0 };
        let mut hashing = download::HashingReader::new(&mut source);
        let mut written = Vec::new();
        extract_from(&mut hashing, &out, &AtomicBool::new(false), &mut written, |_, _, _, _| {}).unwrap();
        // only the central directory may be left for finish() to re-read
        assert!(zip_bytes.len() as u64 - hashing.hashed < 8 * 1024, "hashed {}", hashing.hashed);
        assert_eq!(hashing.finish(|_| Ok(())).unwrap(), format!("{:x}", Md5::digest(&zip_bytes)));
        assert!(source.bytes < zip_bytes.len() as u64 * 3, "excessive ZIP reads: {} for {} bytes", source.bytes, zip_bytes.len());
        assert_eq!(written.len(), 40);
        assert_eq!(std::fs::read(out.join("game/f39.bin")).unwrap(), vec![39u8; 997 * 40]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn streams_deflated_entries_with_data_descriptors() {
        let out = std::env::temp_dir().join(format!("vf_dd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let body = vec![b'x'; 300_000];
        let mut deflated = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflated.write_all(&body).unwrap();
        let deflated = deflated.finish().unwrap();
        let crc = crc32fast::hash(&body);
        let name = b"data/a.bin";
        let mut zip = Vec::new();
        let le16 = |v: &mut Vec<u8>, n: u16| v.extend_from_slice(&n.to_le_bytes());
        let le32 = |v: &mut Vec<u8>, n: u32| v.extend_from_slice(&n.to_le_bytes());
        le32(&mut zip, 0x0403_4b50);
        for n in [20, 1 << 3, 8, 0, 0] { le16(&mut zip, n); }
        for _ in 0..3 { le32(&mut zip, 0); }
        le16(&mut zip, name.len() as u16);
        le16(&mut zip, 0);
        zip.extend_from_slice(name);
        zip.extend_from_slice(&deflated);
        for n in [0x0807_4b50, crc, deflated.len() as u32, body.len() as u32] { le32(&mut zip, n); }
        let cd_offset = zip.len() as u32;
        le32(&mut zip, 0x0201_4b50);
        for n in [20, 20, 1 << 3, 8, 0, 0] { le16(&mut zip, n); }
        for n in [crc, deflated.len() as u32, body.len() as u32] { le32(&mut zip, n); }
        for n in [name.len() as u16, 0, 0, 0, 0] { le16(&mut zip, n); }
        le32(&mut zip, 0);
        le32(&mut zip, 0);
        zip.extend_from_slice(name);
        let cd_size = zip.len() as u32 - cd_offset;
        le32(&mut zip, 0x0605_4b50);
        for n in [0, 0, 1, 1] { le16(&mut zip, n); }
        le32(&mut zip, cd_size);
        le32(&mut zip, cd_offset);
        le16(&mut zip, 0);
        let mut hashing = download::HashingReader::new(std::io::Cursor::new(&zip));
        let mut written = Vec::new();
        extract_from(&mut hashing, &out, &AtomicBool::new(false), &mut written, |_, _, _, _| {}).unwrap();
        assert_eq!(hashing.finish(|_| Ok(())).unwrap(), format!("{:x}", Md5::digest(&zip)));
        assert_eq!(std::fs::read(out.join("data/a.bin")).unwrap(), body);
        let mut bad = zip.clone();
        bad[60] ^= 0xFF;
        assert!(extract_from(&mut std::io::Cursor::new(&bad), &out, &AtomicBool::new(false), &mut Vec::new(), |_, _, _, _| {}).is_err());
        let _ = std::fs::remove_dir_all(&out);
    }
}
#[cfg(test)]
mod self_update_tests {
    use super::apply_self_update;

    fn write(root: &std::path::Path, name: &str, body: &str) {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn copies_the_new_build_over_the_old_one() {
        let root = std::env::temp_dir().join(format!("vitaforge_self_{}", std::process::id()));
        let (stage, dest, backup) = (root.join("stage"), root.join("app"), root.join("bak"));
        write(&dest, "eboot.bin", "old");
        write(&dest, "sce_sys/param.sfo", "old");
        write(&dest, "keep.txt", "keep");
        write(&stage, "eboot.bin", "new");
        write(&stage, "sce_sys/param.sfo", "new");
        write(&stage, "sce_sys/livearea/contents/a.png", "art");
        apply_self_update(&stage, &dest, &backup).unwrap();
        assert_eq!(std::fs::read_to_string(dest.join("eboot.bin")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dest.join("sce_sys/param.sfo")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dest.join("keep.txt")).unwrap(), "keep");
        assert!(dest.join("sce_sys/livearea/contents/a.png").is_file());
        assert!(!backup.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restores_the_old_eboot_when_a_copy_fails() {
        let root = std::env::temp_dir().join(format!("vitaforge_self_fail_{}", std::process::id()));
        let (stage, dest, backup) = (root.join("stage"), root.join("app"), root.join("bak"));
        write(&dest, "eboot.bin", "old");
        write(&dest, "sce_sys/param.sfo", "old");
        write(&dest, "assets", "a file where the update wants a folder");
        write(&stage, "eboot.bin", "new");
        write(&stage, "sce_sys/param.sfo", "new");
        write(&stage, "assets/x.bin", "x");
        assert!(apply_self_update(&stage, &dest, &backup).is_err());
        assert_eq!(std::fs::read_to_string(dest.join("eboot.bin")).unwrap(), "old");
        let _ = std::fs::remove_dir_all(&root);
    }
}
#[cfg(test)]
mod progress_tests {
    use super::Progress;

    #[test]
    fn fraction_tracks_transfers_and_extraction() {
        let down = Progress::Downloading { received: 25, total: Some(100), elapsed_secs: 1 };
        assert_eq!(down.fraction(), Some(0.25));
        let unknown = Progress::Downloading { received: 25, total: None, elapsed_secs: 1 };
        assert_eq!(unknown.fraction(), None);
        assert_eq!(Progress::Extracting { done: 1, total: 4, file: 0, files: 0 }.fraction(), Some(0.25));
        assert_eq!(Progress::Installing { elapsed_secs: 3 }.fraction(), None);
        assert_eq!(Progress::Done.fraction(), Some(1.0));
        assert_eq!(Progress::VerificationComplete.fraction(), None);
        assert!(!Progress::VerificationComplete.is_finished());
        assert_eq!(Progress::VerificationComplete.label_for(crate::app::i18n::Language::Spanish), "Verificación completada · esperando al instalador…");
        assert_eq!(Progress::Finishing { step: "Verifying", percent: 40 }.fraction(), Some(0.4));
        assert_eq!(Progress::Finishing { step: "Verifying", percent: 120 }.fraction(), Some(1.0));
        assert_eq!(Progress::Finishing { step: "Verifying", percent: 40 }.label_for(crate::app::i18n::Language::Spanish), "Verificando archivo · 40%");
    }
}
#[cfg(test)]
mod tests {
    use super::{
        check_free_space, parse_plugin_plan, required_bytes, sanitize_data_dest, transfer_detail, updated_tai_config,
        validated_tai_path,
    };
    #[test]
    fn shows_percent_and_eta_once_there_is_history() {
        assert_eq!(transfer_detail(5 * 1024 * 1024, Some(10 * 1024 * 1024), 10), "50% · 5.0/10.0 MiB · 0.50 MiB/s · ~0:10 left");
    }
    #[test]
    fn holds_back_the_eta_until_the_sample_is_meaningful() {
        assert_eq!(transfer_detail(10, Some(100), 0), "10% · 0.0/0.0 MiB");
        assert_eq!(transfer_detail(0, Some(100), 5), "0% · 0.0/0.0 MiB");
        assert_eq!(transfer_detail(100, Some(100), 5), "100% · 0.0/0.0 MiB · 0.00 MiB/s");
    }
    #[test]
    fn falls_back_to_size_and_elapsed_without_a_content_length() {
        assert_eq!(transfer_detail(2 * 1024 * 1024, None, 65), "2.0 MiB · 0.03 MiB/s");
        assert_eq!(transfer_detail(0, Some(0), 3), "0.0 MiB");
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
    #[test]
    fn inserts_a_plugin_before_the_next_taihen_section() {
        let original = "*KERNEL\nur0:tai/old.skprx\n*ALL\nur0:tai/old.suprx\n";
        let updated = updated_tai_config(original, "*KERNEL", "ur0:tai/new.skprx")
            .unwrap()
            .expect("config should change");
        assert_eq!(
            updated,
            "*KERNEL\nur0:tai/old.skprx\nur0:tai/new.skprx\n*ALL\nur0:tai/old.suprx\n"
        );
    }
    #[test]
    fn does_not_duplicate_an_existing_plugin_line() {
        assert!(
            updated_tai_config("*ALL\nur0:tai/foo.suprx\n", "*ALL", "ur0:tai/foo.suprx")
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn adds_one_module_under_a_second_section() {
        let original = "*main\nur0:tai/wifiturbo.suprx\n*NPXS10015\nur0:tai/henkaku.suprx\n";
        let updated = updated_tai_config(original, "*NPXS10015", "ur0:tai/wifiturbo.suprx")
            .unwrap()
            .expect("the second section still needs the line");
        assert_eq!(
            updated,
            "*main\nur0:tai/wifiturbo.suprx\n*NPXS10015\nur0:tai/henkaku.suprx\nur0:tai/wifiturbo.suprx\n"
        );
    }
    #[test]
    fn multi_module_plan_keeps_each_file_once() {
        let plan = parse_plugin_plan(
            "ur0:tai/",
            "*KERNEL\n*main\n*NPXS10015",
            "ur0:tai/wifiturbo_kernel.skprx\nur0:tai/wifiturbo.suprx\nur0:tai/wifiturbo.suprx",
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.binaries.len(), 2);
        assert_eq!(plan.lines.len(), 3);
        assert!(parse_plugin_plan("ur0:tai/", "*KERNEL\n*main", "ur0:tai/wifiturbo.suprx").is_err());
    }
    #[test]
    fn plugin_lines_are_limited_to_taihen_modules() {
        assert!(validated_tai_path("ur0:tai/foo.skprx").is_ok());
        assert!(validated_tai_path("ux0:tai/foo.suprx").is_ok());
        assert!(validated_tai_path("ur0:tai/../evil.skprx").is_err());
        assert!(validated_tai_path("ur0:tai/foo.txt").is_err());
    }
}
