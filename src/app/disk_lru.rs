//! Size-capped, least-recently-used index of the icon files on the memory card.
//! The Vita can't cheaply set file times, so recency lives in `lru.txt`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const MAX_BYTES: u64 = 100 * 1024 * 1024;
/// Evict down to this share of the cap, so a full cache doesn't delete on every insert.
const LOW_WATER_PERCENT: u64 = 90;
const SAVE_EVERY: u32 = 25;
const INDEX_FILE: &str = "lru.txt";

#[derive(Clone, Copy, Default)]
struct Entry {
    size: u64,
    used: u64,
}

pub struct DiskLru {
    dir: PathBuf,
    max: u64,
    entries: HashMap<String, Entry>,
    total: u64,
    clock: u64,
    unsaved: u32,
}

/// Disk work left over after a change; run it once the index lock is released.
#[derive(Default)]
#[must_use]
pub struct Pending {
    delete: Vec<PathBuf>,
    save: Option<(PathBuf, String)>,
}

impl Pending {
    pub fn run(self) {
        for path in self.delete {
            let _ = std::fs::remove_file(path);
        }
        if let Some((path, text)) = self.save {
            let _ = std::fs::write(path, text);
        }
    }
}

/// `<dir>/ab/cdef…` for a 32-char digest.
pub fn file_path(dir: &Path, digest: &str) -> Option<PathBuf> {
    (digest.len() == 32).then(|| dir.join(&digest[..2]).join(&digest[2..]))
}

/// Every cached file with its size, plus the saved recency of each.
pub fn scan(dir: &Path) -> Vec<(String, u64, u64)> {
    let saved: HashMap<String, u64> = std::fs::read_to_string(dir.join(INDEX_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (digest, used) = line.split_once(' ')?;
            Some((digest.to_owned(), used.parse().ok()?))
        })
        .collect();
    let mut found = Vec::new();
    for folder in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let prefix = folder.file_name().to_string_lossy().into_owned();
        if prefix.len() != 2 {
            continue;
        }
        for file in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let Ok(meta) = file.metadata() else { continue };
            if meta.is_file() && name.len() == 30 && name.chars().all(|c| c.is_ascii_hexdigit()) {
                let digest = format!("{prefix}{name}");
                let used = saved.get(&digest).copied().unwrap_or(0);
                found.push((digest, meta.len(), used));
            }
        }
    }
    found
}

impl DiskLru {
    pub fn new(dir: impl Into<PathBuf>, max: u64) -> Self {
        Self { dir: dir.into(), max, entries: HashMap::new(), total: 0, clock: 0, unsaved: 0 }
    }

    /// Adds what `scan` found; entries touched while it ran keep their newer state.
    pub fn load(&mut self, found: Vec<(String, u64, u64)>) -> Pending {
        for (digest, size, used) in found {
            self.clock = self.clock.max(used);
            if let std::collections::hash_map::Entry::Vacant(slot) = self.entries.entry(digest) {
                slot.insert(Entry { size, used });
                self.total += size;
            }
        }
        self.settle(0)
    }

    pub fn contains(&self, digest: &str) -> bool {
        self.entries.contains_key(digest)
    }

    pub fn touch(&mut self, digest: &str) -> Pending {
        let Some(entry) = self.entries.get_mut(digest) else { return Pending::default() };
        self.clock += 1;
        entry.used = self.clock;
        self.settle(1)
    }

    pub fn insert(&mut self, digest: String, size: u64) -> Pending {
        self.clock += 1;
        if let Some(old) = self.entries.insert(digest, Entry { size, used: self.clock }) {
            self.total -= old.size;
        }
        self.total += size;
        self.settle(1)
    }

    pub fn remove(&mut self, digest: &str) {
        if let Some(old) = self.entries.remove(digest) {
            self.total -= old.size;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.total = 0;
        self.unsaved = 0;
    }

    fn settle(&mut self, changes: u32) -> Pending {
        let mut pending = Pending::default();
        if self.total > self.max {
            let target = self.max * LOW_WATER_PERCENT / 100;
            let mut oldest: Vec<(u64, String)> =
                self.entries.iter().map(|(d, e)| (e.used, d.clone())).collect();
            oldest.sort_unstable();
            for (_, digest) in oldest {
                if self.total <= target {
                    break;
                }
                pending.delete.extend(file_path(&self.dir, &digest));
                self.remove(&digest);
            }
        }
        self.unsaved += changes;
        if self.unsaved >= SAVE_EVERY || !pending.delete.is_empty() {
            self.unsaved = 0;
            let text = self.entries.iter().map(|(d, e)| format!("{d} {}\n", e.used)).collect();
            pending.save = Some((self.dir.join(INDEX_FILE), text));
        }
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(c: char) -> String {
        std::iter::repeat_n(c, 32).collect()
    }

    #[test]
    fn evicts_the_least_recently_used_and_remembers_order() {
        let dir = std::env::temp_dir().join(format!("vitaforge-lru-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lru = DiskLru::new(&dir, 300);
        let write = |lru: &mut DiskLru, c| {
            let path = file_path(&dir, &digest(c)).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, [0u8; 100]).unwrap();
            lru.insert(digest(c), 100).run();
        };
        for c in ['a', 'b', 'c'] {
            write(&mut lru, c);
        }
        lru.touch(&digest('a')).run();
        write(&mut lru, 'd');
        // 400 > 300, evict down to 270: b and c are the oldest.
        assert!(lru.contains(&digest('a')) && lru.contains(&digest('d')));
        assert!(!lru.contains(&digest('b')) && !lru.contains(&digest('c')));
        assert!(!file_path(&dir, &digest('b')).unwrap().exists());

        // A restart reads the same files and keeps `a` older than `d`.
        let mut reloaded = DiskLru::new(&dir, 150);
        reloaded.load(scan(&dir)).run();
        assert!(reloaded.contains(&digest('d')) && !reloaded.contains(&digest('a')));
        let _ = std::fs::remove_dir_all(dir);
    }
}
