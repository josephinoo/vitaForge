pub mod github;
mod head;
pub mod installed;
mod promoter;
mod sfo;

use crate::data::AppEntry;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

const WORK_DIR: &str = "ux0:data/vitaforge";
const TEMP_VPK: &str = "ux0:data/vitaforge/tmp.vpk";
const EXTRACT_DIR: &str = "ux0:data/vitaforge/vpk_install";
const TEMP_DATA: &str = "ux0:data/vitaforge/tmp_data.zip";

const DATA_STAGE_DIR: &str = "ux0:data/vitaforge/data_stage";
const DATA_ROOT: &str = "ux0:data";
const PSP_GAME_DIR: &str = "ux0:pspemu/PSP/GAME";

const PLUGIN_DIR: &str = "ux0:data/vitaforge/plugins";

#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Resolving,
    DownloadingData { received: u64, total: Option<u64> },
    Downloading { received: u64, total: Option<u64> },
    Extracting,
    Installing,
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
            Progress::Extracting => "[3/3] Extracting…".to_owned(),
            Progress::Installing => "[3/3] Installing…".to_owned(),
            Progress::Done => "Installed".to_owned(),
            Progress::Failed(err) => format!("Failed: {err}"),
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Progress::Done | Progress::Failed(_))
    }
}

pub fn start(entry: AppEntry) -> watch::Receiver<Progress> {
    let (tx, rx) = watch::channel(Progress::Resolving);
    tokio::spawn(async move {
        let result = run(&entry, &tx).await;
        let final_state = match result {
            Ok(()) => Progress::Done,
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

async fn run(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
    match entry.platform {
        crate::data::Platform::Vita => run_vita(entry, tx).await,
        crate::data::Platform::Psp => run_psp(entry, tx).await,
        crate::data::Platform::Plugin => run_plugin(entry, tx).await,
        crate::data::Platform::NpsVita => run_nps_vita(entry, tx).await,
        crate::data::Platform::NpsPsp | crate::data::Platform::NpsPsx => run_psp(entry, tx).await,
    }
}

async fn run_nps_vita(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this NPS game");
    }
    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    let _ = std::fs::remove_dir_all(EXTRACT_DIR);

    download(&entry.download_url, Path::new(TEMP_VPK), tx).await?;

    let _ = tx.send(Progress::Extracting);
    let extracted = {
        let src = Path::new(TEMP_VPK);
        let dest = Path::new(EXTRACT_DIR);
        tokio::task::spawn_blocking(move || extract(src, dest)).await
    };

    if extracted.is_ok_and(|r| r.is_ok()) {
        installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, Some(Path::new(EXTRACT_DIR)));
        head::write(Path::new(EXTRACT_DIR))?;
        let _ = tx.send(Progress::Installing);
        tokio::task::spawn_blocking(|| promoter::promote_package(EXTRACT_DIR))
            .await
            .context("install worker crashed")??;
    } else {
        // Direct PKG download - copy to packages folder for user
        let pkgs_dir = "ux0:data/vitaforge/pkgs";
        std::fs::create_dir_all(pkgs_dir).ok();
        let dest = format!("{}/{}.pkg", pkgs_dir, entry.titleid);
        let _ = std::fs::rename(TEMP_VPK, &dest)
            .or_else(|_| std::fs::copy(TEMP_VPK, &dest).map(|_| ()));
        installed::stamp_pending_install(&installed::index_key(entry), &entry.hash, None);
    }
    let _ = std::fs::remove_file(TEMP_VPK);
    Ok(())
}

async fn run_psp(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this app");
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
    Ok(())
}

async fn run_plugin(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
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
    Ok(())
}

async fn run_vita(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
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
    Ok(())
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
    progress: impl Fn(u64, Option<u64>) -> Progress + Copy,
) -> Result<()> {
    let mut last_err = anyhow::anyhow!("download not attempted");
    for attempt in 0u32..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
            eprintln!("retrying download (attempt {}): {url}", attempt + 1);
        }
        match download_attempt(url, dest, tx, progress).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
            }
        }
    }
    Err(last_err)
}

async fn download_attempt(
    url: &str,
    dest: &Path,
    tx: &watch::Sender<Progress>,
    progress: impl Fn(u64, Option<u64>) -> Progress,
) -> Result<()> {
    use futures_util::StreamExt;

    let response = crate::net::client()
        .get(url)
        .header("User-Agent", "VitaForge")
        .send()
        .await
        .context("couldn't reach the download server")?;
    if !response.status().is_success() {
        bail!("download server returned {}", response.status());
    }

    let total = response.content_length();
    let mut received = 0u64;
    let mut file = std::fs::File::create(dest).context("couldn't open the temp file")?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("the download was interrupted")?;
        std::io::Write::write_all(&mut file, &chunk).context("couldn't write the download")?;
        received += chunk.len() as u64;
        let _ = tx.send(progress(received, total));
    }
    Ok(())
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
