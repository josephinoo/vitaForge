use std::fs;
use std::path::Path;

#[cfg(target_os = "vita")]
const ICON_CACHE_DIR: &str = "ux0:data/vitaforge/cache";
#[cfg(not(target_os = "vita"))]
const ICON_CACHE_DIR: &str = "data/vitaforge/cache";

#[cfg(target_os = "vita")]
const HASH_CACHE_DIR: &str = "ux0:data/vitaforge/hashes";
#[cfg(not(target_os = "vita"))]
const HASH_CACHE_DIR: &str = "data/vitaforge/hashes";

#[cfg(target_os = "vita")]
const CATALOG_CACHE_PATH: &str = "ux0:data/vitaforge/catalog_cache.json";
#[cfg(not(target_os = "vita"))]
const CATALOG_CACHE_PATH: &str = "data/vitaforge/catalog_cache.json";

#[cfg(target_os = "vita")]
const CATALOG_VERSION_PATH: &str = "ux0:data/vitaforge/catalog_version.json";
#[cfg(not(target_os = "vita"))]
const CATALOG_VERSION_PATH: &str = "data/vitaforge/catalog_version.json";

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub icons_bytes: u64,
    pub catalog_bytes: u64,
    pub hashes_bytes: u64,
    pub total_bytes: u64,
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn file_size(path: &str) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_size(path: &str) -> u64 {
    fn walk(path: &Path) -> u64 {
        let Ok(entries) = fs::read_dir(path) else {
            return 0;
        };
        let mut total = 0;
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                total += walk(&child);
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        total
    }
    walk(Path::new(path))
}

fn remove_dir_contents(path: &str) -> u64 {
    let bytes = dir_size(path);
    let _ = fs::remove_dir_all(path);
    let _ = fs::create_dir_all(path);
    bytes
}

pub fn compute_cache_stats() -> CacheStats {
    let icons_bytes = dir_size(ICON_CACHE_DIR);
    let catalog_bytes = file_size(CATALOG_CACHE_PATH) + file_size(CATALOG_VERSION_PATH);
    let hashes_bytes = dir_size(HASH_CACHE_DIR);
    CacheStats {
        icons_bytes,
        catalog_bytes,
        hashes_bytes,
        total_bytes: icons_bytes + catalog_bytes + hashes_bytes,
    }
}

pub fn clear_icon_cache() -> u64 {
    remove_dir_contents(ICON_CACHE_DIR)
}

pub fn clear_catalog_cache() -> u64 {
    let bytes = file_size(CATALOG_CACHE_PATH) + file_size(CATALOG_VERSION_PATH);
    let _ = fs::remove_file(CATALOG_CACHE_PATH);
    let _ = fs::remove_file(CATALOG_VERSION_PATH);
    bytes
}

pub fn clear_hash_cache() -> u64 {
    remove_dir_contents(HASH_CACHE_DIR)
}

pub fn purge_all_cache() -> u64 {
    clear_icon_cache() + clear_catalog_cache() + clear_hash_cache()
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_byte_sizes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2 KB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MB");
    }
}
