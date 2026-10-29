//! Reads and writes the config.txt taiHEN is actually using, with a backup
//! before every change.

use super::config::TaiConfig;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEEP_BACKUPS: usize = 15;

/// One writer at a time: the installer thread and the Settings page both edit.
static TAI_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug)]
pub struct Store {
    pub ux0: PathBuf,
    pub ur0: PathBuf,
    pub backups: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    /// Seconds since the epoch, from the file name.
    pub taken_at: u64,
    pub reason: String,
}

impl Backup {
    /// `2026-10-30 14:05` (UTC: the Vita keeps no time zone we can read here).
    pub fn label(&self) -> String {
        let days = (self.taken_at / 86_400) as i64;
        let secs = self.taken_at % 86_400;
        let (y, m, d) = civil_from_days(days);
        format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, secs % 3600 / 60)
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn read_or_empty(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("couldn't read {}", path.display())),
    }
}

fn write_atomically(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = PathBuf::from(format!("{}.vitaforge.tmp", path.display()));
    {
        let mut file = std::fs::File::create(&temporary).context("couldn't write the new config.txt")?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path).context("couldn't replace config.txt")?;
    if read_or_empty(path)? != text {
        bail!("config.txt didn't read back as written");
    }
    Ok(())
}

impl Store {
    pub fn system() -> Self {
        Self {
            ux0: PathBuf::from("ux0:tai/config.txt"),
            ur0: PathBuf::from("ur0:tai/config.txt"),
            backups: PathBuf::from("ur0:tai/vitaforge_backups"),
        }
    }

    /// taiHEN loads ux0:tai/config.txt when it exists and ignores ur0's then.
    pub fn active_path(&self) -> &Path {
        if self.ux0.exists() { &self.ux0 } else { &self.ur0 }
    }

    pub fn both_exist(&self) -> bool {
        self.ux0.exists() && self.ur0.exists()
    }

    pub fn load(&self) -> Result<TaiConfig> {
        Ok(TaiConfig::parse(&read_or_empty(self.active_path())?))
    }

    /// Loads the active file, lets `change` edit it and writes it back if it
    /// changed, after copying the old text into the backups folder.
    pub fn edit<T>(&self, reason: &str, change: impl FnOnce(&mut TaiConfig) -> Result<T>) -> Result<(T, bool)> {
        let _guard = TAI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.active_path().to_path_buf();
        let before = read_or_empty(&path)?;
        let mut config = TaiConfig::parse(&before);
        let value = change(&mut config)?;
        let after = config.render();
        if after == before {
            return Ok((value, false));
        }
        self.backup(&path, &before, reason)?;
        write_atomically(&path, &after)?;
        Ok((value, true))
    }

    fn backup(&self, path: &Path, text: &str, reason: &str) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.backups).context("couldn't create the backups folder")?;
        let reason: String = reason
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .take(32)
            .collect();
        // Two edits in the same second still get two backups, in order.
        let stamp = self.list_backups().first().map_or(0, |newest| newest.taken_at + 1).max(now());
        std::fs::write(self.backups.join(format!("config-{stamp}-{reason}.txt")), text)
            .context("couldn't back up config.txt")?;
        for old in self.list_backups().into_iter().skip(KEEP_BACKUPS) {
            let _ = std::fs::remove_file(old.path);
        }
        Ok(())
    }

    /// Newest first.
    pub fn list_backups(&self) -> Vec<Backup> {
        let Ok(dir) = std::fs::read_dir(&self.backups) else { return Vec::new() };
        let mut backups: Vec<Backup> = dir
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let rest = name.strip_prefix("config-")?.strip_suffix(".txt")?;
                let (stamp, reason) = rest.split_once('-').unwrap_or((rest, ""));
                Some(Backup { path: entry.path(), taken_at: stamp.parse().ok()?, reason: reason.to_owned() })
            })
            .collect();
        backups.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
        backups
    }

    /// Puts a backup back; the file it replaces is itself backed up first.
    pub fn restore(&self, backup: &Backup) -> Result<()> {
        let text = std::fs::read_to_string(&backup.path).context("couldn't read the backup")?;
        let restored = TaiConfig::parse(&text);
        self.edit("restore", |config| {
            *config = restored;
            Ok(())
        })?;
        Ok(())
    }

    /// Makes ur0 the only config: ux0's (the one in effect) is copied over and
    /// renamed out of taiHEN's way.
    pub fn migrate_to_ur0(&self) -> Result<()> {
        let _guard = TAI_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if !self.ux0.exists() {
            return Ok(());
        }
        let text = read_or_empty(&self.ux0)?;
        let old = read_or_empty(&self.ur0)?;
        self.backup(&self.ur0, &old, "before-migrate")?;
        write_atomically(&self.ur0, &text)?;
        let parked = PathBuf::from(format!("{}.vitaforge-migrated", self.ux0.display()));
        let _ = std::fs::remove_file(&parked);
        std::fs::rename(&self.ux0, &parked).context("couldn't move ux0:tai/config.txt aside")?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn temp_store(name: &str) -> Store {
    let root = std::env::temp_dir().join(format!("vitaforge-tai-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("ux0")).unwrap();
    std::fs::create_dir_all(root.join("ur0")).unwrap();
    Store { ux0: root.join("ux0/config.txt"), ur0: root.join("ur0/config.txt"), backups: root.join("ur0/backups") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_the_active_file_and_backs_up_first() {
        let store = temp_store("edit");
        std::fs::write(&store.ur0, "*main\nur0:tai/a.suprx\n").unwrap();
        let ((), changed) = store.edit("toggle", |c| c.set_enabled(1, false).map(|_| ())).unwrap();
        assert!(changed);
        assert_eq!(std::fs::read_to_string(&store.ur0).unwrap(), "*main\n# ur0:tai/a.suprx\n");
        let backups = store.list_backups();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].reason, "toggle");
        assert_eq!(std::fs::read_to_string(&backups[0].path).unwrap(), "*main\nur0:tai/a.suprx\n");

        std::fs::write(&store.ux0, "*KERNEL\n").unwrap();
        assert!(store.both_exist());
        assert_eq!(store.active_path(), store.ux0.as_path());
    }

    #[test]
    fn unchanged_edits_write_nothing() {
        let store = temp_store("noop");
        std::fs::write(&store.ur0, "*main\n").unwrap();
        let ((), changed) = store.edit("noop", |_| Ok(())).unwrap();
        assert!(!changed);
        assert!(store.list_backups().is_empty());
    }

    #[test]
    fn keeps_the_newest_backups_only() {
        let store = temp_store("rotate");
        std::fs::write(&store.ur0, "*main\n").unwrap();
        for i in 0..KEEP_BACKUPS + 4 {
            store.edit("add", |c| Ok(c.add("*main", &format!("ur0:tai/p{i}.suprx"), |_| false))).unwrap();
        }
        let backups = store.list_backups();
        assert_eq!(backups.len(), KEEP_BACKUPS);
        // The oldest backups (fewest lines) are the ones that went.
        let oldest = std::fs::read_to_string(&backups.last().unwrap().path).unwrap();
        assert!(oldest.contains("p3.suprx") && !oldest.contains("p4.suprx"));
    }

    #[test]
    fn restore_and_migrate() {
        let store = temp_store("restore");
        std::fs::write(&store.ur0, "*main\nur0:tai/a.suprx\n").unwrap();
        store.edit("remove", |c| c.remove(1)).unwrap();
        let backup = store.list_backups().remove(0);
        store.restore(&backup).unwrap();
        assert_eq!(std::fs::read_to_string(&store.ur0).unwrap(), "*main\nur0:tai/a.suprx\n");

        std::fs::write(&store.ux0, "*KERNEL\nux0:tai/k.skprx\n").unwrap();
        store.migrate_to_ur0().unwrap();
        assert!(!store.ux0.exists());
        assert_eq!(std::fs::read_to_string(&store.ur0).unwrap(), "*KERNEL\nux0:tai/k.skprx\n");
    }

    #[test]
    fn backup_dates_read_as_calendar_days() {
        let backup = Backup { path: PathBuf::new(), taken_at: 1_793_366_700, reason: String::new() };
        assert_eq!(backup.label(), "2026-10-30 13:25");
    }
}
