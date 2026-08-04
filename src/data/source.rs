use super::AppEntry;
use super::api::{self, CatalogFetch};
use std::path::Path;

const CACHE_PATH: &str = "ux0:data/vitaforge/cache.json";
const ETAG_PATH: &str = "ux0:data/vitaforge/cache.etag";

pub fn load_catalog() -> Vec<AppEntry> {
    Vec::new()
}

pub fn initial_catalog() -> Vec<AppEntry> {
    load_cached().unwrap_or_else(load_catalog)
}

fn load_cached() -> Option<Vec<AppEntry>> {
    let text = std::fs::read_to_string(CACHE_PATH).ok()?;
    match serde_json::from_str::<Vec<AppEntry>>(&text) {
        Ok(mut entries) => {
            for entry in &mut entries {
                entry.rebuild_derived();
            }
            Some(entries)
        }
        Err(err) => {
            eprintln!("cached catalog failed to parse: {err}");
            None
        }
    }
}

fn load_etag() -> Option<String> {
    std::fs::read_to_string(ETAG_PATH).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn save_etag(etag: &str) {
    let _ = std::fs::write(ETAG_PATH, etag);
}

pub fn save_cache(entries: &[AppEntry]) {
    let Some(parent) = Path::new(CACHE_PATH).parent() else { return };
    if let Err(err) = std::fs::create_dir_all(parent) {
        eprintln!("couldn't create catalog cache dir: {err}");
        return;
    }
    match serde_json::to_string(entries) {
        Ok(json) => {
            if let Err(err) = std::fs::write(CACHE_PATH, json) {
                eprintln!("couldn't write catalog cache: {err}");
            }
        }
        Err(err) => eprintln!("couldn't serialize catalog cache: {err}"),
    }
}

/// Fetches the live catalog from the VitaForge API. Returns `None` when the server
/// reports the catalog hasn't changed since the last fetch (ETag match) — callers
/// should keep using the on-disk cache in that case.
pub async fn fetch_live() -> anyhow::Result<Option<Vec<AppEntry>>> {
    let etag = load_etag();
    match api::fetch_catalog(etag.as_deref()).await? {
        CatalogFetch::NotModified => Ok(None),
        CatalogFetch::Fresh { entries, etag } => {
            if let Some(etag) = etag {
                save_etag(&etag);
            }
            Ok(Some(entries))
        }
    }
}
