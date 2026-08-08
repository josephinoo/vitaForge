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
    let mut entries = Vec::new();
    append_custom_apps(&mut entries);
    entries
}

pub fn initial_catalog() -> Vec<AppEntry> {
    let mut entries = load_cached().unwrap_or_else(load_catalog);
    append_custom_apps(&mut entries);
    entries
}

fn load_cached() -> Option<Vec<AppEntry>> {
    let text = std::fs::read_to_string(CACHE_PATH).ok()?;
    match serde_json::from_str::<Vec<AppEntry>>(&text) {
        Ok(mut entries) => {
            for entry in &mut entries {
                entry.rebuild_derived();
            }
            append_custom_apps(&mut entries);
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
    append_custom_apps(&mut entries);
    Ok(entries)
}

pub fn custom_apps() -> Vec<AppEntry> {
    use super::Category;

    vec![
        AppEntry {
            id: "AUTOPLUG2".to_string(),
            titleid: "AUTOPLUG2".to_string(),
            name: "AutoPlugin II".to_string(),
            name_lower: "autoplugin ii".to_string(),
            author: "ONemenu Team".to_string(),
            description: "Tool to easily install and update PS Vita plugins.".to_string(),
            long_description: "AutoPlugin II is a tool designed for the PS Vita that allows you to install and update plugins for your console with a single click.".to_string(),
            requirements: "".to_string(),
            changelog: "".to_string(),
            release_page: Some("https://github.com/ONemenu/AutoPlugin2".to_string()),
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: Some("https://raw.githubusercontent.com/ONemenu/AutoPlugin2/master/resources/icon0.png".to_string()),
            screenshot_urls: vec![],
            download_url: "https://github.com/ONemenu/AutoPlugin2/releases/download/v2.0.5/AutoPlugin2.vpk".to_string(),
            source: Some("ONemenu/AutoPlugin2".to_string()),
            version: "2.0.5".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 12_500_000,
            downloads: 50000,
            rating: 5.0,
            updated_at: "2024-01-01".to_string(),
        },
        AppEntry {
            id: "ONEMENU01".to_string(),
            titleid: "ONEMENU01".to_string(),
            name: "ONEmenu for PSVita".to_string(),
            name_lower: "onemenu for psvita".to_string(),
            author: "ONemenu Team".to_string(),
            description: "Customizable file manager and homebrew launcher for PS Vita.".to_string(),
            long_description: "ONEmenu for PSVita is a UI alternative for your PSVita with file manager, app launcher, and custom themes.".to_string(),
            requirements: "".to_string(),
            changelog: "".to_string(),
            release_page: Some("https://github.com/ONemenu/ONEmenu-PSVita".to_string()),
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: None,
            screenshot_urls: vec![],
            download_url: "https://github.com/ONemenu/ONEmenu-PSVita/releases/download/v2.0.0/ONEmenu.vpk".to_string(),
            source: Some("ONemenu/ONEmenu-PSVita".to_string()),
            version: "2.0.0".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 8_400_000,
            downloads: 30000,
            rating: 4.8,
            updated_at: "2023-12-01".to_string(),
        },
        AppEntry {
            id: "PSVCLEANR".to_string(),
            titleid: "PSVCLEANR".to_string(),
            name: "PSV Cleaner".to_string(),
            name_lower: "psv cleaner".to_string(),
            author: "MVI-Development".to_string(),
            description: "Utility to clean cache, temporary files, and system logs on PS Vita.".to_string(),
            long_description: "PSV Cleaner frees up space on your PS Vita memory card by deleting temp files, web browser cache, and system logs.".to_string(),
            requirements: "".to_string(),
            changelog: "".to_string(),
            release_page: Some("https://github.com/MVI-Development/PSV-Cleaner".to_string()),
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: None,
            screenshot_urls: vec![],
            download_url: "https://github.com/MVI-Development/PSV-Cleaner/releases/download/v1.0/PSV_Cleaner.vpk".to_string(),
            source: Some("MVI-Development/PSV-Cleaner".to_string()),
            version: "1.0.0".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 2_100_000,
            downloads: 15000,
            rating: 4.7,
            updated_at: "2023-11-01".to_string(),
        },
        AppEntry {
            id: "VITADOC01".to_string(),
            titleid: "VITADOC01".to_string(),
            name: "VitaDoctor".to_string(),
            name_lower: "vitadoctor".to_string(),
            author: "MODEVOLUTION".to_string(),
            description: "Diagnostic and repair tool for PS Vita system database and storage.".to_string(),
            long_description: "VitaDoctor inspects and fixes system database corruptions and tests storage speed and integrity.".to_string(),
            requirements: "".to_string(),
            changelog: "".to_string(),
            release_page: Some("https://github.com/MODEVOLUTION/VitaDoctor".to_string()),
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: None,
            screenshot_urls: vec![],
            download_url: "https://github.com/MODEVOLUTION/VitaDoctor/releases/download/v1.0/VitaDoctor.vpk".to_string(),
            source: Some("MODEVOLUTION/VitaDoctor".to_string()),
            version: "1.0.0".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 3_500_000,
            downloads: 12000,
            rating: 4.6,
            updated_at: "2023-10-01".to_string(),
        },
        AppEntry {
            id: "VPKDIRECT".to_string(),
            titleid: "VPKDIRECT".to_string(),
            name: "VPK Installer Direct".to_string(),
            name_lower: "vpk installer direct".to_string(),
            author: "VitaForge".to_string(),
            description: "Direct VPK installer utility for downloading and installing VPK packages.".to_string(),
            long_description: "VPK Installer Direct allows direct installation of VPK homebrew files from local storage or network URLs.".to_string(),
            requirements: "".to_string(),
            changelog: "".to_string(),
            release_page: None,
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: None,
            screenshot_urls: vec![],
            download_url: "https://github.com/josephinoo/vitaForge/releases/latest".to_string(),
            source: None,
            version: "1.0.0".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 1_800_000,
            downloads: 25000,
            rating: 4.9,
            updated_at: "2024-02-01".to_string(),
        },
        AppEntry {
            id: "PBP661INS".to_string(),
            titleid: "PBP661INS".to_string(),
            name: "6.61 PBP Installer".to_string(),
            name_lower: "6.61 pbp installer".to_string(),
            author: "TheOfficialFloW".to_string(),
            description: "Utility to download and install official PSP 6.61 firmware PBP for Adrenaline.".to_string(),
            long_description: "Downloads and places the official PSP 6.61 firmware update PBP file into ux0:app/PSPEMUCFW/661.PBP required by Adrenaline eCFW.".to_string(),
            requirements: "Adrenaline".to_string(),
            changelog: "".to_string(),
            release_page: None,
            category: Category::Utility,
            platform: Platform::Vita,
            icon_url: None,
            screenshot_urls: vec![],
            download_url: "http://du01.psp.update.playstation.org/update/psp/image/us/2014_1212_6bbe8b0e92ab13d3d573db97d3c0e1a0/EBOOT.PBP".to_string(),
            source: None,
            version: "6.61".to_string(),
            hash: "".to_string(),
            hash2: "".to_string(),
            data_url: None,
            data_size_bytes: 0,
            size_bytes: 31_000_000,
            downloads: 45000,
            rating: 5.0,
            updated_at: "2024-01-01".to_string(),
        },
    ]
}

pub fn append_custom_apps(entries: &mut Vec<AppEntry>) {
    let existing_ids: std::collections::HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();
    for mut custom in custom_apps() {
        if !existing_ids.contains(&custom.id) {
            custom.rebuild_derived();
            entries.push(custom);
        }
    }
}

async fn fetch_section(url: &str, platform: Platform) -> anyhow::Result<Vec<AppEntry>> {
    let raw: Vec<RawEntry> = crate::net::client().get(url).send().await?.json().await?;
    Ok(raw.into_iter().filter_map(|entry| entry.into_app_entry(platform)).collect())
}
