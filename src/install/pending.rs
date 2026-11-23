use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const PENDING_DIR: &str = "ux0:data/vitaforge/pending";

#[derive(Deserialize, Debug)]
pub struct PendingManifest {
    pub title_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub hash: String,
    pub staged_dir: String,
}

/// Scans `ux0:data/vitaforge/pending/*.json` written by vitaForge-client over USB
/// and promotes each staged VPK into LiveArea.
pub fn process_all() -> usize {
    let dir = Path::new(PENDING_DIR);
    process_dir(dir, |manifest, staged| {
        super::installed::stamp_pending_install(
            &manifest.title_id,
            &manifest.hash,
            &manifest.version,
            Some(staged),
        );
        super::head::write(staged)?;
        let (tx, _rx) = tokio::sync::watch::channel(super::Progress::Installing { elapsed_secs: 0 });
        super::promoter::promote_package(&manifest.staged_dir, &tx)?;
        if staged.exists() {
            let _ = std::fs::remove_dir_all(staged);
            bail!("the system didn't accept the package");
        }
        Ok(())
    })
}

fn process_dir<F>(pending_dir: &Path, mut promote: F) -> usize
where
    F: FnMut(&PendingManifest, &Path) -> Result<()>,
{
    let entries = match std::fs::read_dir(pending_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut installed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match process_one(pending_dir, &path, &mut promote) {
            Ok(name) => {
                super::log_file(&format!("pending-usb: installed {name}"));
                installed += 1;
            }
            Err(err) => {
                super::log_file(&format!("pending-usb: failed {}: {err:#}", path.display()));
            }
        }
    }
    installed
}

fn process_one<F>(pending_dir: &Path, manifest_path: &Path, promote: &mut F) -> Result<String>
where
    F: FnMut(&PendingManifest, &Path) -> Result<()>,
{
    let raw = std::fs::read_to_string(manifest_path).context("read pending manifest")?;
    let manifest: PendingManifest = serde_json::from_str(&raw).context("parse pending manifest")?;
    if manifest.title_id.trim().is_empty()
        || manifest.title_id.contains('/')
        || manifest.title_id.contains('\\')
        || manifest.title_id.contains("..")
    {
        bail!("invalid title_id '{}'", manifest.title_id);
    }
    let staged: PathBuf = pending_dir.join(&manifest.title_id);
    if !staged.is_dir() {
        let _ = std::fs::remove_file(manifest_path);
        bail!("staged dir missing: {}", staged.display());
    }
    let res = promote(&manifest, &staged);
    let _ = std::fs::remove_file(manifest_path);
    if res.is_err() {
        let _ = std::fs::remove_dir_all(&staged);
    }
    res?;
    Ok(if manifest.title.is_empty() {
        manifest.title_id
    } else {
        format!("{} ({})", manifest.title, manifest.title_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_valid_pending_and_cleans_manifest() {
        let dir = std::env::temp_dir().join(format!("vf-pending-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("ABCD12345")).unwrap();
        std::fs::write(
            dir.join("ABCD12345.json"),
            r#"{"title_id":"ABCD12345","title":"Test","version":"1.0","hash":"","staged_dir":"ux0:data/vitaforge/pending/ABCD12345"}"#,
        )
        .unwrap();

        let mut seen = Vec::new();
        let count = process_dir(&dir, |m, staged| {
            seen.push((m.title_id.clone(), staged.to_path_buf()));
            std::fs::remove_dir_all(staged)?;
            Ok(())
        });

        assert_eq!(count, 1);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "ABCD12345");
        assert!(!dir.join("ABCD12345.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
