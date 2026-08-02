use super::vitadbtoo::RawEntry;
use super::AppEntry;
use std::path::Path;

const DEFAULT_CATALOG_URL: &str = "https://drdecki.github.io/VitaDBtoo-db/apps.json";
const CACHE_PATH: &str = "ux0:data/vitaforge/cache.json";

fn catalog_url() -> &'static str {
    option_env!("SERVER_URL").unwrap_or(DEFAULT_CATALOG_URL)
}

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
            // `name_lower` is `#[serde(skip)]`, so it comes back empty from cache.
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

/// Slow enough to drop frames, so callers hand it to the blocking pool.
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

pub async fn fetch_live() -> anyhow::Result<Vec<AppEntry>> {
    let raw: Vec<RawEntry> =
        crate::net::client().get(catalog_url()).send().await?.json().await?;
    let entries: Vec<AppEntry> = raw.into_iter().filter_map(RawEntry::into_app_entry).collect();
    Ok(entries)
}
