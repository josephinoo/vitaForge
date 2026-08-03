use super::{AppEntry, Category, Platform};
use serde::Deserialize;

use super::source::base_url;

#[derive(Debug, Deserialize)]
pub struct RawEntry {
    id: String,
    #[serde(default)]
    titleid: String,
    name: String,
    author: String,
    description: String,
    #[serde(default)]
    long_description: String,
    #[serde(default)]
    requirements: String,
    #[serde(default)]
    changelog: String,
    #[serde(default)]
    release_page: String,
    #[serde(default)]
    data: String,
    #[serde(default)]
    data_size: String,

    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    icon: String,
    url: String,
    #[serde(default)]
    source: String,
    version: String,
    #[serde(default)]
    hash: String,
    #[serde(default)]
    hash2: String,
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

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn offset_type(kind: &str, by: i32) -> String {
    kind.trim().parse::<i32>().map(|n| (n - by).to_string()).unwrap_or_default()
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
    pub fn into_app_entry(self, platform: Platform) -> Option<AppEntry> {

        if self.url.is_empty() || self.url.contains("get_hb_url.php") {
            return None;
        }
        let category = match platform {
            Platform::Plugin => Category::Plugin,

            Platform::Psp => category_from_type(&offset_type(&self.kind, 10))?,
            Platform::Vita => category_from_type(&self.kind)?,
        };
        let score: f32 = self.score.parse().unwrap_or(0.0);
        let rating = (score / 100.0 * 5.0).clamp(0.0, 5.0);

        Some(AppEntry {
            id: self.id,
            titleid: self.titleid,
            name_lower: self.name.to_lowercase(),
            name: self.name,
            author: if self.author.is_empty() { "unknown".to_owned() } else { self.author },
            description: self.description,
            long_description: self.long_description,
            requirements: self.requirements,
            changelog: self.changelog,
            release_page: non_empty(self.release_page),
            category,
            platform,
            icon_url: (!self.icon.is_empty())
                .then(|| format!("{}icons/{}", base_url(), self.icon)),
            screenshot_urls: self
                .screenshots
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("{}{s}", base_url()))
                .collect(),
            download_url: self.url,
            source: if self.source.contains("github.com") { Some(self.source) } else { None },
            version: self.version,
            hash: self.hash.trim().to_lowercase(),
            hash2: self.hash2.trim().to_lowercase(),
            data_url: non_empty(self.data),
            data_size_bytes: self.data_size.parse().unwrap_or(0),
            size_bytes: self.size.parse().unwrap_or(0),
            downloads: self.downloads.parse().unwrap_or(0),
            rating,
            updated_at: self.date,
        })
    }
}
