use super::{AppEntry, Category};
use serde::Deserialize;

const ICON_BASE: &str = "https://drdecki.github.io/VitaDBtoo-db/icons/";
const ASSET_BASE: &str = "https://drdecki.github.io/VitaDBtoo-db/";

#[derive(Debug, Deserialize)]
pub struct RawEntry {
    id: String,
    titleid: String,
    name: String,
    author: String,
    description: String,
    #[serde(rename = "type")]
    kind: String,
    icon: String,
    url: String,
    #[serde(default)]
    source: String,
    version: String,
    #[serde(default)]
    hash: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    downloads: String,
    #[serde(default)]
    score: String,
    date: String,
    #[serde(default)]
    screenshots: String,
}

fn category_from_type(kind: &str) -> Option<Category> {
    match kind {
        "1" => Some(Category::Game),
        "2" => Some(Category::Port),
        "4" => Some(Category::Utility),
        "5" => Some(Category::Emulator),
        _ => None,
    }
}

impl RawEntry {
    pub fn into_app_entry(self) -> Option<AppEntry> {
        if self.url.is_empty() || self.url.contains("get_hb_url.php") {
            return None;
        }
        let category = category_from_type(&self.kind)?;
        let score: f32 = self.score.parse().unwrap_or(0.0);
        let rating = (score / 100.0 * 5.0).clamp(0.0, 5.0);

        Some(AppEntry {
            id: self.id,
            titleid: self.titleid,
            name_lower: self.name.to_lowercase(),
            name: self.name,
            author: if self.author.is_empty() { "unknown".to_owned() } else { self.author },
            description: self.description,
            category,
            icon_url: if self.icon.is_empty() { None } else { Some(format!("{ICON_BASE}{}", self.icon)) },
            screenshot_urls: self
                .screenshots
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("{ASSET_BASE}{s}"))
                .collect(),
            download_url: self.url,
            source: if self.source.contains("github.com") { Some(self.source) } else { None },
            version: self.version,
            hash: self.hash.trim().to_lowercase(),
            size_bytes: self.size.parse().unwrap_or(0),
            downloads: self.downloads.parse().unwrap_or(0),
            rating,
            updated_at: self.date,
        })
    }
}
