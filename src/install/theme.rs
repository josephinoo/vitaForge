//! Theme installs (psvt.ovh zips): extract to
//! `ux0:customtheme/<Name>`, then register in the shell DB (`theme_db`).
use super::theme_db::{self, ThemeRow};
use super::installed::InstallState;
use super::{Progress, TEMP_VPK, WORK_DIR, download, extract};
use crate::data::AppEntry;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::watch;

const THEME_ROOT: &str = "ux0:customtheme";
/// Written next to `theme.xml` on install; holds the zip's hash so a newer upload shows as an update.
const MARKER: &str = "vitaforge.txt";

fn folder_name(entry: &AppEntry) -> String {
    let name: String = entry
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '_' } else { c }) // the shell's own and imago's folders never contain spaces
        .take(48)
        .collect();
    let name = name.trim();
    if name.is_empty() { format!("theme-{}", entry.id) } else { name.to_owned() }
}

fn theme_dir(entry: &AppEntry) -> PathBuf {
    Path::new(THEME_ROOT).join(folder_name(entry))
}

fn marker_hash(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(MARKER)).ok()?;
    text.lines().find_map(|l| l.strip_prefix("hash=")).map(|h| h.trim().to_lowercase())
}

pub fn state(entry: &AppEntry) -> InstallState {
    match marker_hash(&theme_dir(entry)) {
        // Opening the file, not stat: an emulator's stat can say "exists" for a missing path, and a
        // false Installed puts every theme in the Library.
        None if std::fs::File::open(theme_dir(entry).join("theme.xml")).is_ok() => InstallState::Installed,
        None => InstallState::Absent,
        Some(h) if h == entry.hash.to_lowercase() => InstallState::Installed,
        Some(_) => InstallState::Outdated,
    }
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Zips that wrap everything in one folder get unwrapped so `theme.xml` sits at the top.
fn flatten_wrapper(stage: &Path) -> Result<()> {
    if stage.join("theme.xml").exists() {
        return Ok(());
    }
    let mut dirs = std::fs::read_dir(stage)?.flatten().filter(|e| e.path().is_dir());
    let (Some(inner), None) = (dirs.next(), dirs.next()) else { bail!("the download has no theme.xml") };
    let inner = inner.path();
    if !inner.join("theme.xml").exists() {
        bail!("the download has no theme.xml");
    }
    for item in std::fs::read_dir(&inner)?.flatten() {
        std::fs::rename(item.path(), stage.join(item.file_name()))?;
    }
    let _ = std::fs::remove_dir(&inner);
    Ok(())
}

pub async fn run(entry: &AppEntry, tx: &watch::Sender<Progress>, cancel: &Arc<AtomicBool>) -> Result<Progress> {
    let _ = tx.send(Progress::Resolving);
    if entry.download_url.is_empty() {
        bail!("no download link for this theme");
    }
    std::fs::create_dir_all(WORK_DIR).context("couldn't create the work folder")?;
    std::fs::create_dir_all(THEME_ROOT).context("couldn't create the theme folder")?;
    download(&entry.download_url, Path::new(TEMP_VPK), tx, cancel).await?;
    let final_dir = theme_dir(entry);
    let stage = PathBuf::from(format!("{}.vf-new", final_dir.display()));
    let old = PathBuf::from(format!("{}.vf-old", final_dir.display()));
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_dir_all(&old);
    let _ = tx.send(Progress::Extracting { done: 0, total: 0, file: 0, files: 0 });
    let (tx_w, cancel_w, stage_w) = (tx.clone(), cancel.clone(), stage.clone());
    let extracted = tokio::task::spawn_blocking(move || {
        extract(Path::new(TEMP_VPK), &stage_w, &cancel_w, |done, total, file, files| {
            let _ = tx_w.send(Progress::Extracting { done, total, file, files });
        })
    })
    .await
    .context("the extract worker crashed")?;
    let _ = std::fs::remove_file(TEMP_VPK);
    let result = extracted.and_then(|_| finish(entry, &stage, &final_dir, &old));
    let _ = std::fs::remove_dir_all(&stage);
    result?;
    let _ = std::fs::remove_dir_all(&old);
    Ok(Progress::Done)
}

fn finish(entry: &AppEntry, stage: &Path, final_dir: &Path, old: &Path) -> Result<()> {
    flatten_wrapper(stage)?;
    let xml = std::fs::read_to_string(stage.join("theme.xml")).context("couldn't read theme.xml")?;
    std::fs::write(stage.join(MARKER), format!("id={}\nhash={}\n", entry.id, entry.hash.to_lowercase()))?;
    let had_old = final_dir.exists();
    if had_old {
        std::fs::rename(final_dir, old).context("couldn't move the previous version aside")?;
    }
    if let Err(err) = std::fs::rename(stage, final_dir) {
        if had_old {
            let _ = std::fs::rename(old, final_dir);
        }
        return Err(err).context("couldn't put the theme in place");
    }
    let dir_id = final_dir.to_string_lossy().into_owned();
    let title = theme_db::xml_text(&xml, &["m_title", "m_default"]).unwrap_or_else(|| entry.name.clone());
    let provider = theme_db::xml_text(&xml, &["m_provider", "m_default"]).unwrap_or_default();
    let ver = theme_db::xml_text(&xml, &["m_contentVer"]).unwrap_or_else(|| "01.00".into());
    let row = ThemeRow { dir: &dir_id, title: &title, provider: &provider, content_ver: &ver, size: dir_size(final_dir) as i64 };
    if let Err(err) = theme_db::register(&row) {
        let _ = std::fs::remove_dir_all(final_dir);
        if had_old {
            let _ = std::fs::rename(old, final_dir);
        }
        return Err(err);
    }
    Ok(())
}

pub fn uninstall(entry: &AppEntry) -> Result<()> {
    let dir = theme_dir(entry);
    if dir.parent() != Some(Path::new(THEME_ROOT)) {
        bail!("refusing to delete outside {THEME_ROOT}");
    }
    theme_db::unregister(&dir.to_string_lossy())?;
    std::fs::remove_dir_all(&dir).context("couldn't delete the theme folder")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flattens_a_single_wrapper_folder() {
        let root = std::env::temp_dir().join(format!("vf-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Inner")).unwrap();
        std::fs::write(root.join("Inner/theme.xml"), "<x/>").unwrap();
        flatten_wrapper(&root).unwrap();
        assert!(root.join("theme.xml").exists());
        assert!(flatten_wrapper(&root.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
