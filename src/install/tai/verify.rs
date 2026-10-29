//! Safe module replacement: a download is checked before it can replace a
//! working module, and the old file comes back if anything later fails.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Rejects empty files and, for taiHEN modules, anything that isn't an ELF
/// (a saved HTML error page would otherwise end up in config.txt).
pub fn check_module(path: &Path, expected_size: Option<u64>) -> Result<()> {
    check_as(path, path, expected_size)
}

/// Same, judging the file by the name it will have (`staged` is usually a `.tmp`).
fn check_as(path: &Path, final_name: &Path, expected_size: Option<u64>) -> Result<()> {
    let len = std::fs::metadata(path).with_context(|| format!("couldn't read {}", path.display()))?.len();
    if len == 0 {
        bail!("the downloaded plugin is empty");
    }
    if let Some(expected) = expected_size.filter(|n| *n > 0) {
        if len != expected {
            bail!("the downloaded plugin is {len} bytes, expected {expected}");
        }
    }
    let name = final_name.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".skprx") || name.ends_with(".suprx") {
        let mut magic = [0u8; 4];
        std::fs::File::open(path)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic))
            .map_err(|_| anyhow::anyhow!("the downloaded plugin is too short to be a module"))?;
        if magic != *b"\x7fELF" {
            bail!("the downloaded file isn't a valid plugin module");
        }
    }
    Ok(())
}

/// A module put in place; `commit` keeps it, `rollback` brings the old one back.
#[derive(Debug)]
pub struct Swap {
    target: PathBuf,
    old: Option<PathBuf>,
}

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", target.display()))
}

/// Verifies `staged`, then swaps it in for `target`, keeping the previous file aside.
pub fn install_module(staged: &Path, target: &Path, expected_size: Option<u64>) -> Result<Swap> {
    check_as(staged, target, expected_size)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let incoming = sibling(target, ".vitaforge.new");
    let _ = std::fs::remove_file(&incoming);
    std::fs::copy(staged, &incoming).with_context(|| format!("couldn't write {}", target.display()))?;
    let old = if target.exists() {
        let old = sibling(target, ".vitaforge.old");
        let _ = std::fs::remove_file(&old);
        if let Err(err) = std::fs::rename(target, &old) {
            let _ = std::fs::remove_file(&incoming);
            return Err(err).with_context(|| format!("couldn't move {} aside", target.display()));
        }
        Some(old)
    } else {
        None
    };
    if let Err(err) = std::fs::rename(&incoming, target) {
        let _ = std::fs::remove_file(&incoming);
        if let Some(old) = &old {
            let _ = std::fs::rename(old, target);
        }
        return Err(err).with_context(|| format!("couldn't replace {}", target.display()));
    }
    Ok(Swap { target: target.to_path_buf(), old })
}

impl Swap {
    pub fn commit(self) {
        if let Some(old) = self.old {
            let _ = std::fs::remove_file(old);
        }
    }

    pub fn rollback(self) {
        let _ = std::fs::remove_file(&self.target);
        if let Some(old) = self.old {
            let _ = std::fs::rename(old, &self.target);
        }
    }
}

pub fn commit_all(swaps: Vec<Swap>) {
    swaps.into_iter().for_each(Swap::commit);
}

pub fn rollback_all(swaps: Vec<Swap>) {
    swaps.into_iter().rev().for_each(Swap::rollback);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vitaforge-verify-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn elf() -> Vec<u8> {
        let mut bytes = b"\x7fELF".to_vec();
        bytes.extend_from_slice(&[1; 60]);
        bytes
    }

    #[test]
    fn rejects_bad_downloads() {
        let d = dir("check");
        let empty = d.join("a.skprx");
        std::fs::write(&empty, b"").unwrap();
        assert!(check_module(&empty, None).unwrap_err().to_string().contains("empty"));
        let html = d.join("b.suprx");
        std::fs::write(&html, b"<html>404</html>").unwrap();
        assert!(check_module(&html, None).unwrap_err().to_string().contains("valid plugin"));
        let short = d.join("c.suprx");
        std::fs::write(&short, b"\x7f").unwrap();
        assert!(check_module(&short, None).is_err());
        let good = d.join("d.skprx");
        std::fs::write(&good, elf()).unwrap();
        assert!(check_module(&good, None).is_ok());
        assert!(check_module(&good, Some(64)).is_ok());
        assert!(check_module(&good, Some(10)).unwrap_err().to_string().contains("expected 10"));
        assert!(check_module(&good, Some(0)).is_ok(), "0 means unknown");
        let data = d.join("e.bin");
        std::fs::write(&data, b"anything").unwrap();
        assert!(check_module(&data, None).is_ok(), "only modules need the ELF header");
        assert!(check_module(&d.join("missing.skprx"), None).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn replaces_and_commit_drops_the_old_file() {
        let d = dir("commit");
        let (staged, target) = (d.join("s.tmp"), d.join("tai/p.suprx"));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&staged, elf()).unwrap();
        let swap = install_module(&staged, &target, None).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), elf());
        assert!(sibling(&target, ".vitaforge.old").exists());
        swap.commit();
        assert!(!sibling(&target, ".vitaforge.old").exists());
        assert!(!sibling(&target, ".vitaforge.new").exists());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rollback_restores_the_previous_module() {
        let d = dir("rollback");
        let (staged, target) = (d.join("s.tmp"), d.join("p.suprx"));
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&staged, elf()).unwrap();
        install_module(&staged, &target, None).unwrap().rollback();
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        assert!(!sibling(&target, ".vitaforge.old").exists());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rollback_of_a_fresh_install_removes_it() {
        let d = dir("fresh");
        let (staged, target) = (d.join("s.tmp"), d.join("new.skprx"));
        std::fs::write(&staged, elf()).unwrap();
        let first = install_module(&staged, &target, None).unwrap();
        assert!(target.exists());
        first.rollback();
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_bad_download_never_touches_the_working_module() {
        let d = dir("bad");
        let (staged, target) = (d.join("s.tmp"), d.join("p.skprx"));
        std::fs::write(&target, elf()).unwrap();
        std::fs::write(&staged, b"<html>").unwrap();
        assert!(install_module(&staged, &target, None).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), elf());
        let wrongly_named = d.join("page.tmp");
        std::fs::write(&wrongly_named, b"<html>").unwrap();
        assert!(install_module(&wrongly_named, &target, None).is_err(), "judged by the target's name");
        assert!(!sibling(&target, ".vitaforge.old").exists());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn rollback_all_undoes_every_module() {
        let d = dir("all");
        let staged = d.join("s.tmp");
        std::fs::write(&staged, elf()).unwrap();
        let (a, b) = (d.join("a.suprx"), d.join("b.suprx"));
        std::fs::write(&a, b"old-a").unwrap();
        let swaps = vec![install_module(&staged, &a, None).unwrap(), install_module(&staged, &b, None).unwrap()];
        rollback_all(swaps);
        assert_eq!(std::fs::read(&a).unwrap(), b"old-a");
        assert!(!b.exists());
        let _ = std::fs::remove_dir_all(d);
    }
}
