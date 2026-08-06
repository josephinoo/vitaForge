pub mod download;
pub mod github;
mod head;
pub mod installed;
pub mod notify;
pub mod pkg;
mod promoter;
mod sfo;
mod bgdl;
mod licensing;

use crate::data::AppEntry;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

const WORK_DIR: &str = "ux0:data/vitaforge";
const TEMP_VPK: &str = "ux0:data/vitaforge/tmp.vpk";
const EXTRACT_DIR: &str = "ux0:data/vitaforge/vpk_install";
const TEMP_DATA: &str = "ux0:data/vitaforge/tmp_data.zip";
const TEMP_PKG: &str = "ux0:data/vitaforge/tmp.pkg";
const PKG_STAGE_DIR: &str = "ux0:data/vitaforge/pkg_stage";

const DATA_STAGE_DIR: &str = "ux0:data/vitaforge/data_stage";
const DATA_ROOT: &str = "ux0:data";
const PSP_GAME_DIR: &str = "ux0:pspemu/PSP/GAME";

const PLUGIN_DIR: &str = "ux0:data/vitaforge/plugins";

#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Resolving,
    DownloadingData { received: u64, total: Option<u64> },
    Downloading { received: u64, total: Option<u64> },
    /// In-app PKG decryption/extraction, as opposed to `Extracting` (VPK/ZIP).
    Decrypting,
    Extracting,
    Installing,
    /// Handed off to the system's native background downloader (BGDL) —
    /// visible in Notifications/LiveArea, but not finished installing yet.
    Queued,
    Done,
    Failed(String),
}

impl Progress {
    pub fn label(&self) -> String {
        match self {
            Progress::Resolving => "Checking latest…".to_owned(),
            Progress::Downloading { received, total } => match total {
                Some(total) if *total > 0 => {
                    format!("[2/3] Downloading {}%", (*received * 100 / *total).min(100))
                }
                _ => format!("[2/3] Downloading {:.1} MB", *received as f64 / (1024.0 * 1024.0)),
            },
            Progress::DownloadingData { received, total } => match total {
                Some(total) if *total > 0 => {
                    format!("[1/3] Game data {}%", (*received * 100 / *total).min(100))
                }
                _ => format!("[1/3] Game data {:.1} MB", *received as f64 / (1024.0 * 1024.0)),
            },
            Progress::Decrypting => "[3/3] Decrypting…".to_owned(),
            Progress::Extracting => "[3/3] Extracting…".to_owned(),
            Progress::Installing => "[3/3] Installing…".to_owned(),
            Progress::Queued => "Queued — check Notifications".to_owned(),
            Progress::Done => "Installed".to_owned(),
            Progress::Failed(err) => format!("Failed: {err}"),
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Progress::Queued | Progress::Done | Progress::Failed(_))
    }
}

pub fn start(entry: AppEntry) -> watch::Receiver<Progress> {
    let (tx, rx) = watch::channel(Progress::Resolving);
    tokio::spawn(async move {
        let result = run(&entry, &tx).await;
        let final_state = match result {
            Ok(state) => state,
            Err(err) => {
                eprintln!("install failed: {err:#}");
                let _ = std::fs::remove_dir_all(EXTRACT_DIR);
                let _ = std::fs::remove_file(TEMP_VPK);

                installed::clear_pending_install(&installed::index_key(&entry));
                Progress::Failed(format!("{err}"))
            }
        };
        let _ = tx.send(final_state);
    });
    rx
}

async fn run(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
    match entry.platform {
        crate::data::Platform::Vita => run_vita(entry, tx).await,
        crate::data::Platform::Psp => run_psp(entry, tx).await,
        crate::data::Platform::Plugin => run_plugin(entry, tx).await,
        crate::data::Platform::NpsVita => run_nps_vita(entry, tx).await,
        crate::data::Platform::NpsPsp | crate::data::Platform::NpsPsx => run_psp(entry, tx).await,
    }
}

async fn run_nps_vita(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this NPS game");
    }

    match queue_bgdl_vita(entry, tx).await {
        Ok(progress) => Ok(progress),
        Err(err) => {
            eprintln!("BGDL background queue failed for {}: {err:#}, falling back to in-app PKG install", entry.name);
            std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
            let pkg_path = Path::new(TEMP_PKG);
            download(&entry.download_url, pkg_path, tx).await?;
            install_vita_pkg(entry, tx, pkg_path).await
        }
    }
}

/// Decrypts and extracts a downloaded `.pkg` in-app and promotes it directly
/// — no BGDL handoff, no leaving the app. Falls through to an `Err` (handled
/// by the caller, which retries via BGDL) for anything this extractor
/// doesn't support: PSM content, an unrecognized key type, or a malformed pkg.
async fn install_vita_pkg(entry: &AppEntry, tx: &watch::Sender<Progress>, pkg_path: &Path) -> Result<Progress> {
    let _ = tx.send(Progress::Decrypting);

    let content_id = licensing::resolve_content_id(entry).unwrap_or_default();
    let fake_license = licensing::create_fake_license(&content_id);

    let stage_dir = format!("{PKG_STAGE_DIR}/{}", entry.titleid.trim().to_uppercase());
    let _ = std::fs::remove_dir_all(&stage_dir);

    let pkg_path_owned = pkg_path.to_path_buf();
    let stage_dir_worker = stage_dir.clone();
    let header = tokio::task::spawn_blocking(move || {
        pkg::extract_vita(&pkg_path_owned, Path::new(&stage_dir_worker), &fake_license)
    })
    .await
    .context("the pkg extract worker crashed")??;
    let _ = std::fs::remove_file(pkg_path);

    // Cheap sanity check: if the catalog's content_id disagrees with what was
    // actually inside the pkg, the fake license above was built for the wrong
    // title and the promoter will very likely reject the package.
    if !content_id.is_empty() && content_id != header.content_id {
        eprintln!(
            "warning: catalog content_id '{content_id}' doesn't match the pkg's own '{}' for {}",
            header.content_id, entry.name
        );
    }

    let titleid = header.title_id.clone();
    if !titleid.is_empty() {
        for root in &["ux0:app", "ur0:app", "uma0:app"] {
            let path = format!("{root}/{titleid}");
            if installed::vita_fs::exists(&path) || Path::new(&path).exists() {
                if let Err(err) = std::fs::remove_dir_all(&path) {
                    eprintln!("couldn't clear the previous install at {path}: {err}");
                }
            }
        }
    }

    let _ = tx.send(Progress::Installing);
    let stage_dir_promote = stage_dir.clone();
    tokio::task::spawn_blocking(move || promoter::promote_package(&stage_dir_promote))
        .await
        .context("the install worker crashed")??;

    if Path::new(&stage_dir).exists() {
        let _ = std::fs::remove_dir_all(&stage_dir);
        bail!("the system didn't accept the package");
    }

    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
    Ok(Progress::Done)
}

/// Decrypts and extracts a downloaded PS1 `.pkg` in-app straight into
/// `ux0:pspemu/PSP/GAME/{title_id}` — no ISO conversion needed for PSX
/// content, just `EBOOT.PBP` + `DOCUMENT.DAT`. Falls through to `Err` (the
/// caller retries via BGDL) on anything unsupported.
async fn install_psx_pkg(entry: &AppEntry, tx: &watch::Sender<Progress>, pkg_path: &Path) -> Result<Progress> {
    let _ = tx.send(Progress::Decrypting);

    let dest = PathBuf::from(PSP_GAME_DIR).join(entry.titleid.trim());
    let pkg_path_owned = pkg_path.to_path_buf();
    let dest_worker = dest.clone();
    tokio::task::spawn_blocking(move || pkg::extract_psx(&pkg_path_owned, &dest_worker))
        .await
        .context("the pkg extract worker crashed")??;
    let _ = std::fs::remove_file(pkg_path);

    let content_id = licensing::resolve_content_id(entry)?;
    let psp_license = licensing::create_fake_license(&content_id);
    std::fs::create_dir_all("ux0:pspemu/PSP/LICENSE").context("couldn't create the PSP license folder")?;
    std::fs::write(format!("ux0:pspemu/PSP/LICENSE/{content_id}.rif"), &psp_license)
        .context("couldn't write the PSP license file")?;

    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
    Ok(Progress::Done)
}

/// Best-effort fallback for content this extractor doesn't (yet) support in
/// app: hands the download URL to the system's native background downloader,
/// same as before this feature existed. The install then finishes outside
/// VitaForge, visible in Notifications/LiveArea.
async fn queue_bgdl_vita(entry: &AppEntry, _tx: &watch::Sender<Progress>) -> Result<Progress> {
    stage_bgdl_icon(entry).await;

    let license = match licensing::resolve_content_id(entry) {
        Ok(content_id) => Some(licensing::create_fake_license(&content_id)),
        Err(err) => {
            eprintln!("no license generated for {}: {err:#}", entry.name);
            None
        }
    };

    bgdl::start_bgdl(&entry.name, &entry.download_url, license.as_deref(), bgdl::BGDL_TYPE_GAME)
        .context("Failed to queue background download (BGDL)")?;

    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
    Ok(Progress::Queued)
}

async fn run_psp(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this app");
    }

    if entry.platform.is_nps() {
        let has_nopspemudrm = licensing::is_module_present("NoPspEmuDrm_kern");

        if !has_nopspemudrm {
            bail!(
                "This PSP/PS1 game needs the NoPspEmuDrm_kern plugin. Install it from the \
                 Plugins tab and restart the console before trying again."
            );
        }

        stage_bgdl_icon(entry).await;

        let content_id = licensing::resolve_content_id(entry)?;
        let psp_license = licensing::create_fake_license(&content_id);
        let bgdl_result = bgdl::start_bgdl(&entry.name, &entry.download_url, Some(&psp_license), bgdl::BGDL_TYPE_PSP);

        if bgdl_result.is_ok() {
            installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
            return Ok(Progress::Queued);
        }

        eprintln!("BGDL queueing failed for PSP/PS1 game {}, falling back to in-app install", entry.name);

        if entry.platform == crate::data::Platform::NpsPsx {
            std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
            let pkg_path = Path::new(TEMP_PKG);
            download(&entry.download_url, pkg_path, tx).await?;

            if let Ok(state) = install_psx_pkg(entry, tx, pkg_path).await {
                return Ok(state);
            }
            let _ = std::fs::remove_file(pkg_path);
        }

        bail!("Failed to install PSP/PS1 game: BGDL service unavailable");
    }

    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    download(&entry.download_url, Path::new(TEMP_VPK), tx).await?;

    let dest = PathBuf::from(PSP_GAME_DIR).join(&entry.id);
    let _ = tx.send(Progress::Extracting);
    tokio::task::spawn_blocking(move || extract(Path::new(TEMP_VPK), &dest))
        .await
        .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);

    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
    Ok(Progress::Done)
}

/// Best-effort: writes the entry's artwork to `ux0:bgdl/icon0.png` so the
/// LiveArea bubble BGDL creates isn't blank. Failure here must never block
/// the install — the icon is cosmetic.
async fn stage_bgdl_icon(entry: &AppEntry) {
    let Some(url) = entry.icon_url.as_deref().or(entry.cover_url.as_deref()) else { return };
    let request = crate::net::client().get(url).header("User-Agent", "VitaForge").send();
    let Ok(response) = request.await else { return };
    let Ok(bytes) = response.bytes().await else { return };
    if std::fs::create_dir_all("ux0:bgdl").is_ok() {
        let _ = std::fs::write("ux0:bgdl/icon0.png", &bytes);
    }
}

async fn run_plugin(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
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
    download(&entry.download_url, &dest, tx).await?;
    Ok(Progress::Done)
}

async fn run_vita(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<Progress> {
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

    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    let _ = std::fs::remove_dir_all(EXTRACT_DIR);
    let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);

    if let Some(data_url) = entry.data_url.as_deref() {
        stage_data(data_url, tx).await?;
    }

    download(&url, Path::new(TEMP_VPK), tx).await?;

    let _ = tx.send(Progress::Extracting);
    tokio::task::spawn_blocking(|| extract(Path::new(TEMP_VPK), Path::new(EXTRACT_DIR)))
        .await
        .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);

    installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, Some(Path::new(EXTRACT_DIR)));

    head::write(Path::new(EXTRACT_DIR))?;

    let titleid = entry.titleid.trim().to_uppercase();
    if !titleid.is_empty() {
        for root in &["ux0:app", "ur0:app", "uma0:app"] {
            let path = format!("{root}/{titleid}");
            if installed::vita_fs::exists(&path) || Path::new(&path).exists() {
                if let Err(err) = std::fs::remove_dir_all(&path) {
                    eprintln!("couldn't clear the previous install at {path}: {err}");
                }
            }
        }
    }

    let _ = tx.send(Progress::Installing);
    tokio::task::spawn_blocking(|| promoter::promote_package(EXTRACT_DIR))
        .await
        .context("the install worker crashed")??;

    if Path::new(EXTRACT_DIR).exists() {
        let _ = std::fs::remove_dir_all(EXTRACT_DIR);
        let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
        bail!("the system didn't accept the package");
    }

    if Path::new(DATA_STAGE_DIR).exists() {
        tokio::task::spawn_blocking(|| merge_into(Path::new(DATA_STAGE_DIR), Path::new(DATA_ROOT)))
            .await
            .context("the data worker crashed")??;
        let _ = std::fs::remove_dir_all(DATA_STAGE_DIR);
    }
    Ok(Progress::Done)
}

async fn stage_data(url: &str, tx: &watch::Sender<Progress>) -> Result<()> {
    let dest = Path::new(TEMP_DATA);
    download_with(url, dest, tx, |received, total| Progress::DownloadingData { received, total })
        .await
        .context("couldn't download the required game data")?;

    let _ = tx.send(Progress::Extracting);
    tokio::task::spawn_blocking(|| extract(Path::new(TEMP_DATA), Path::new(DATA_STAGE_DIR)))
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

async fn download(url: &str, dest: &Path, tx: &watch::Sender<Progress>) -> Result<()> {
    download_with(url, dest, tx, |received, total| Progress::Downloading { received, total }).await
}

async fn download_with(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<Progress>,
    progress: fn(u64, Option<u64>) -> Progress,
) -> Result<()> {
    let mut sink = download::FileSink::open(dest)?;
    let mut reporter = download::ProgressReporter::new(tx, progress);
    download::fetch_to(url, &mut sink, &mut reporter).await
}

fn extract(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).context("couldn't reopen the downloaded file")?;
    let mut zip = zip::ZipArchive::new(file).context("the download isn't a valid vpk (zip)")?;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else { continue };
        let out_path: PathBuf = dest.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path)
            .with_context(|| format!("couldn't write {}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}
