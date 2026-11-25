//! VitaForge ROMs catalog & downloader for Azahar (`ux0:data/azahar/roms/`).
//! Fetches gzipped snapshots from `https://vitaforge.josephinoo.dev/api/v1/roms/snapshot?type=3ds`
//! with ETag (`If-None-Match`) caching, and reuses `crate::install::download::fetch_file`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use tokio::sync::watch;

pub const SNAPSHOT_API_BASE: &str = "https://vitaforge.josephinoo.dev/api/v1/roms/snapshot";
pub const DEFAULT_ROM_TYPE: &str = "3ds";
pub struct RomConsole {
    pub code: &'static str,
    pub label: &'static str,
    /// Where the console's usual Vita emulator finds games; editable in Settings.
    pub default_dir: &'static str,
}

const fn console(code: &'static str, label: &'static str, default_dir: &'static str) -> RomConsole {
    RomConsole { code, label, default_dir }
}

/// Order of the ROMs sidebar and of the folder rows in Settings (`Row::RomsDir` indexes it).
/// RetroArch has no fixed content folder, so its consoles sit next to its other data.
pub const ROM_CONSOLES: [RomConsole; 8] = [
    console("3ds", "Nintendo 3DS", "ux0:data/azahar/roms"),
    // DSVita reads this folder only.
    console("ds", "Nintendo DS", "ux0:data/dsvita"),
    console("gba", "Game Boy Advance", "ux0:data/retroarch/roms/gba"),
    console("snes", "Super Nintendo", "ux0:data/retroarch/roms/snes"),
    console("n64", "Nintendo 64", "ux0:data/DaedalusX64/Roms"),
    console("gb", "Game Boy", "ux0:data/retroarch/roms/gb"),
    console("gbc", "Game Boy Color", "ux0:data/retroarch/roms/gbc"),
    console("nes", "NES", "ux0:data/retroarch/roms/nes"),
];

pub fn default_dir(rom_type: &str) -> String {
    ROM_CONSOLES
        .iter()
        .find(|c| c.code == rom_type)
        .map_or_else(|| format!("ux0:data/roms/{rom_type}"), |c| c.default_dir.to_owned())
}

pub fn storage_dir_for(rom_type: &str) -> PathBuf {
    PathBuf::from(crate::data::settings::roms_dir(rom_type))
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RomEntry {
    pub id: String,
    pub title: String,
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    pub download_url: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
}

impl RomEntry {
    /// Regions from the No-Intro name, "Game (USA, Europe)"; the API sends no region field.
    pub fn regions(&self) -> impl Iterator<Item = &str> {
        self.title
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(group, _)| group.split(", "))
            .into_iter()
            .flatten()
    }

    /// `needle` is already lowercase.
    pub fn matches(&self, needle: &str, genre: Option<&str>, region: Option<&str>) -> bool {
        (needle.is_empty() || self.title.to_lowercase().contains(needle))
            && genre.is_none_or(|g| self.genres.iter().any(|own| own == g))
            && region.is_none_or(|r| self.regions().any(|own| own == r))
    }

    pub fn to_app_entry(&self, rom_type: &str) -> crate::data::AppEntry {
        self.to_app_entry_in(rom_type, &storage_dir_for(rom_type))
    }

    /// For whole lists: the folder lookup reads the settings file, so do it once.
    pub fn to_app_entry_in(&self, rom_type: &str, dir: &Path) -> crate::data::AppEntry {
        let upper = rom_type.to_uppercase();
        let mut entry = crate::data::AppEntry {
            id: self.id.clone(),
            titleid: self.id.clone(),
            titleid_lower: self.id.to_lowercase(),
            content_id: None,
            name: self.title.clone(),
            original_name: Some(self.filename.clone()),
            name_lower: self.title.to_lowercase(),
            author: format!("{upper} ROM"),
            author_lower: format!("{rom_type} rom"),
            description_lower: String::new(),
            description: format!("{} · {}", self.filename, dir.display()),
            long_description: String::new(),
            requirements: String::new(),
            changelog: String::new(),
            release_page: None,
            category: crate::data::Category::Emulator,
            genres: if self.genres.is_empty() { vec![upper.clone()] } else { self.genres.clone() },
            platform: crate::data::Platform::Vita,
            kind: format!("rom_{rom_type}"),
            icon_url: self.icon_url.clone(),
            cover_url: self.icon_url.clone(),
            background_url: None,
            screenshot_urls: Vec::new(),
            download_url: self.download_url.clone(),
            source: Some("archive.org".to_owned()),
            version: upper,
            region: None,
            zrif: None,
            source_catalog: "roms".to_owned(),
            source_labels: vec!["ROMs".to_owned()],
            hash: String::new(),
            hash2: String::new(),
            data_url: None,
            data_extract_path: None,
            data_size_bytes: 0,
            game_data_languages: Vec::new(),
            plugin_install_path: None,
            plugin_config_section: None,
            plugin_config_line: None,
            plugin_meta: None,
            size_bytes: self.size,
            downloads: 0,
            rating: 0.0,
            updated_at: String::new(),
            synced_at: String::new(),
            ratings_count: 0,
            likes_count: 0,
            comments_count: 0,
            user_liked: false,
            user_rating: None,
            overview: Vec::new(),
        };
        entry.rebuild_derived();
        entry
    }
}

pub fn start_rom_install(
    entry: crate::data::AppEntry,
) -> (
    watch::Receiver<crate::install::Progress>,
    std::sync::Arc<AtomicBool>,
) {
    let (tx, rx) = watch::channel(crate::install::Progress::Resolving);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let cancel_bg = cancel.clone();
    let rom_type = entry.kind.strip_prefix("rom_").unwrap_or(DEFAULT_ROM_TYPE).to_owned();
    let storage = storage_dir_for(&rom_type);
    let rom = RomEntry {
        id: entry.id,
        title: entry.name,
        filename: entry.original_name.unwrap_or_else(|| "rom.zcci".to_owned()),
        size: entry.size_bytes,
        download_url: entry.download_url,
        icon_url: entry.icon_url,
        genres: entry.genres,
    };
    tokio::spawn(async move {
        if let Err(err) = download_rom(&rom, &storage, &tx, &cancel_bg).await {
            let _ = tx.send(crate::install::Progress::Failed(format!("{err:#}")));
        }
    });
    (rx, cancel)
}

#[derive(Deserialize)]
struct SnapshotResponse {
    #[serde(default)]
    data: Vec<RomEntry>,
}

pub enum SnapshotFetch {
    NotModified,
    Updated {
        entries: Vec<RomEntry>,
        etag: Option<String>,
    },
}

pub fn snapshot_url(rom_type: &str) -> String {
    format!("{SNAPSHOT_API_BASE}?type={rom_type}")
}

/// Catalog size without downloading the snapshot: a one-item page carries `total`.
pub async fn fetch_total(rom_type: &str) -> Result<usize> {
    #[derive(Deserialize)]
    struct Page {
        total: usize,
    }
    let base = SNAPSHOT_API_BASE.trim_end_matches("/snapshot");
    let page: Page = crate::net::client()
        .get(format!("{base}?type={rom_type}&limit=1"))
        .header("X-Client-ID", crate::data::client_id::get())
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(page.total)
}

pub fn parse_snapshot(json: &[u8]) -> Result<Vec<RomEntry>> {
    let parsed: SnapshotResponse =
        serde_json::from_slice(json).context("invalid VitaForge ROMs snapshot JSON")?;
    Ok(parsed.data)
}

pub async fn fetch_snapshot(rom_type: &str, etag: Option<&str>) -> Result<SnapshotFetch> {
    let url = snapshot_url(rom_type);
    let mut req = crate::net::client().get(&url);
    if let Some(tag) = etag {
        req = req.header(reqwest::header::IF_NONE_MATCH, tag);
    }
    let resp = req.send().await.context("failed to reach VitaForge ROMs API")?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(SnapshotFetch::NotModified);
    }
    if !resp.status().is_success() {
        bail!("VitaForge ROMs API returned HTTP {}", resp.status().as_u16());
    }
    let new_etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = resp.bytes().await.context("failed reading ROMs snapshot")?;
    let entries = parse_snapshot(&bytes)?;
    Ok(SnapshotFetch::Updated {
        entries,
        etag: new_etag,
    })
}

/// Downloads a ROM using `crate::install::download::fetch_file`
/// (parallel range resume + Archive.org fastest node + MD5 check) and extracts `.zip` into `storage_dir`.
pub async fn download_rom(
    entry: &RomEntry,
    storage_dir: &Path,
    tx: &watch::Sender<crate::install::Progress>,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    std::fs::create_dir_all(storage_dir)
        .with_context(|| format!("failed to create {}", storage_dir.display()))?;

    let file_name = Path::new(&entry.filename)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("rom.zcci");
    let dest = storage_dir.join(file_name);

    let _ = tx.send(crate::install::Progress::Resolving);
    crate::install::download::fetch_file(
        &entry.download_url,
        &dest,
        tx,
        |received, total, elapsed_secs| crate::install::Progress::Downloading {
            received,
            total,
            elapsed_secs,
        },
        cancel,
    )
    .await?;

    if file_name.to_ascii_lowercase().ends_with(".zip") {
        let zip_path = dest.clone();
        let out_dir = storage_dir.to_path_buf();
        let tx_ext = tx.clone();
        let cancel_ext = cancel.load(std::sync::atomic::Ordering::Relaxed);
        if cancel_ext {
            bail!(crate::install::CANCELLED_MESSAGE);
        }
        let extracted = tokio::task::spawn_blocking(move || extract_rom_zip(&zip_path, &out_dir, &tx_ext))
            .await
            .context("zip extraction worker crashed")??;
        let _ = std::fs::remove_file(&dest);
        let _ = tx.send(crate::install::Progress::Done);
        Ok(extracted)
    } else {
        let _ = tx.send(crate::install::Progress::Done);
        Ok(dest)
    }
}

pub fn extract_rom_zip(
    zip_path: &Path,
    out_dir: &Path,
    tx: &watch::Sender<crate::install::Progress>,
) -> Result<PathBuf> {
    use std::io::{Read, Write};
    const MIB: u64 = 1024 * 1024;
    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("failed to open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
    let total_files = archive.len() as u32;
    let total_bytes: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().filter(|e| !e.is_dir()).map(|e| e.size()))
        .sum();
    let total_mib = (total_bytes / MIB).max(1) as u32;
    let mut done_bytes = 0u64;
    let mut last_reported_mib = u32::MAX;
    let mut buf = vec![0u8; 256 * 1024];
    let mut first_rom: Option<PathBuf> = None;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = Path::new(entry.name()).file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let target = out_dir.join(name);
        let out_file = std::fs::File::create(&target)
            .with_context(|| format!("failed to create {}", target.display()))?;
        let mut out = std::io::BufWriter::with_capacity(512 * 1024, out_file);
        loop {
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done_bytes += n as u64;
            let done_mib = (done_bytes / MIB) as u32;
            if done_mib != last_reported_mib {
                last_reported_mib = done_mib;
                let _ = tx.send(crate::install::Progress::Extracting {
                    done: done_mib,
                    total: total_mib,
                    file: (i + 1) as u32,
                    files: total_files,
                });
            }
        }
        out.flush()?;
        if first_rom.is_none() {
            first_rom = Some(target);
        }
    }

    first_rom.ok_or_else(|| anyhow::anyhow!("zip archive contained no files"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_console_has_its_own_folder() {
        let dirs: std::collections::HashSet<_> = ROM_CONSOLES.iter().map(|c| c.default_dir).collect();
        assert_eq!(dirs.len(), ROM_CONSOLES.len());
        assert_eq!(default_dir("ds"), "ux0:data/dsvita");
    }

    #[test]
    fn region_comes_from_the_title() {
        let mut rom = parse_snapshot(br#"{"data":[{"id":"a","title":"Game (USA, Europe) (Rev 1)","filename":"a.zip","download_url":"u"}]}"#).unwrap().remove(0);
        assert_eq!(rom.regions().collect::<Vec<_>>(), ["USA", "Europe"]);
        assert!(rom.matches("game", None, Some("Europe")));
        assert!(!rom.matches("", None, Some("Japan")));
        rom.title = "No region".to_owned();
        assert_eq!(rom.regions().count(), 0);
    }

    #[test]
    fn parses_vitaforge_snapshot_json() {
        let json = br#"{
            "type": "3ds",
            "count": 2,
            "data": [
                {
                    "id": "mario-kart-7",
                    "title": "Mario Kart 7 (USA)",
                    "filename": "Mario Kart 7 (USA).zip",
                    "size": 671088640,
                    "download_url": "https://archive.org/download/example/Mario%20Kart%207.zip",
                    "icon_url": "https://thumbnails.libretro.com/Nintendo%20-%20Nintendo%203DS/Named_Boxarts/Mario%20Kart%207.png"
                },
                {
                    "id": "no-art-game",
                    "title": "No Art Game",
                    "filename": "no-art.zip",
                    "size": 1024,
                    "download_url": "https://archive.org/download/example/no-art.zip",
                    "icon_url": null
                }
            ]
        }"#;
        let entries = parse_snapshot(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Mario Kart 7 (USA)");
        assert_eq!(entries[0].size, 671088640);
        assert!(entries[0].icon_url.is_some());
        assert_eq!(entries[1].icon_url, None);
        assert_eq!(
            snapshot_url(DEFAULT_ROM_TYPE),
            "https://vitaforge.josephinoo.dev/api/v1/roms/snapshot?type=3ds"
        );
    }
}
