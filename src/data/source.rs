use super::vitadbtoo::RawEntry;
use super::{AppEntry, Platform};
use std::path::Path;

const DEFAULT_CATALOG_URL: &str = "https://drdecki.github.io/VitaDBtoo-db/apps.json";
const CACHE_PATH: &str = "ux0:data/vitaforge/cache.json";

pub fn catalog_url() -> &'static str {
    option_env!("SERVER_URL").unwrap_or(DEFAULT_CATALOG_URL)
}

pub fn base_url() -> &'static str {
    let url = catalog_url();
    url.rfind('/').map_or(url, |cut| &url[..=cut])
}

fn minimal_url() -> String {
    format!("{}minimal.json", base_url())
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

#[derive(serde::Deserialize)]
pub struct MinimalEntry {
    pub id: String,
    pub hash: String,
}

pub async fn fetch_minimal() -> anyhow::Result<Vec<MinimalEntry>> {
    Ok(crate::net::client().get(minimal_url()).send().await?.json().await?)
}

pub fn apply_hashes(entries: &mut [AppEntry], minimal: &[MinimalEntry]) {
    let fresh: std::collections::HashMap<&str, &str> =
        minimal.iter().map(|m| (m.id.as_str(), m.hash.trim())).collect();
    for entry in entries {
        if let Some(hash) = fresh.get(entry.id.as_str()) {
            entry.hash = hash.to_lowercase();
        }
    }
}

pub async fn fetch_live() -> anyhow::Result<Vec<AppEntry>> {
    let mut entries = fetch_section(catalog_url(), Platform::Vita).await?;

    let base = base_url();
    let sections = [
        (format!("{base}psp_apps.json"), Platform::Psp),
        (format!("{base}preserved/plugins.json"), Platform::Plugin),
    ];
    for (url, platform) in sections {
        match fetch_section(&url, platform).await {
            Ok(extra) => entries.extend(extra),
            Err(err) => eprintln!("couldn't load the {} catalog: {err:#}", platform.label()),
        }
    }
    Ok(entries)
}

async fn fetch_section(url: &str, platform: Platform) -> anyhow::Result<Vec<AppEntry>> {
    let raw: Vec<RawEntry> = crate::net::client().get(url).send().await?.json().await?;
    Ok(raw.into_iter().filter_map(|entry| entry.into_app_entry(platform)).collect())
}
