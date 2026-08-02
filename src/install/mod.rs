mod github;
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

#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Resolving,
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
                    format!("Downloading {}%", (*received * 100 / *total).min(100))
                }
                _ => format!("Downloading {:.1} MB", *received as f64 / (1024.0 * 1024.0)),
            },
            Progress::Extracting => "Extracting…".to_owned(),
            Progress::Installing => "Installing…".to_owned(),
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
                Progress::Failed(format!("{err}"))
            }
        };
        let _ = tx.send(final_state);
    });
    rx
}

async fn run(entry: &AppEntry, tx: &watch::Sender<Progress>) -> Result<()> {
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

    download(&url, Path::new(TEMP_VPK), tx).await?;

    // Unzipping the vpk and promoting it are long synchronous jobs; on the
    // current-thread runtime they would freeze the UI until the install finished.
    let _ = tx.send(Progress::Extracting);
    tokio::task::spawn_blocking(|| extract(Path::new(TEMP_VPK), Path::new(EXTRACT_DIR)))
        .await
        .context("the extract worker crashed")??;
    let _ = std::fs::remove_file(TEMP_VPK);

    // Stamped before promoting so the hash ships inside the package.
    installed::stamp_pending_install(Path::new(EXTRACT_DIR), &entry.hash);

    head::write(Path::new(EXTRACT_DIR))?;

    let _ = tx.send(Progress::Installing);
    tokio::task::spawn_blocking(|| promoter::promote_package(EXTRACT_DIR))
        .await
        .context("the install worker crashed")??;

    if Path::new(EXTRACT_DIR).exists() {
        let _ = std::fs::remove_dir_all(EXTRACT_DIR);
        bail!("the system didn't accept the package");
    }
    Ok(())
}

async fn download(url: &str, dest: &Path, tx: &watch::Sender<Progress>) -> Result<()> {
    use futures_util::StreamExt;

    let response = crate::net::client()
        .get(url)
        .header("User-Agent", "vitaforge")
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
        let _ = tx.send(Progress::Downloading { received, total });
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
