pub mod api;
pub mod cache_manager;
pub mod client_id;
pub mod settings;
pub mod source;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Emulator,
    Original,
    PsVitaGame,
    Ps1Game,
    PspGame,
    Plugin,
    Port,
    Tool,
    Utility,
    Other,
}
impl Category {
    pub const ALL: [Category; 6] = [
        Category::Emulator,
        Category::Original,
        Category::PsVitaGame,
        Category::Plugin,
        Category::Port,
        Category::Utility,
    ];
    pub fn label_upper(self) -> &'static str {
        match self {
            Category::Emulator => "EMULATOR",
            Category::Original => "ORIGINAL",
            Category::PsVitaGame => "PS VITA GAME",
            Category::Ps1Game => "PS1 GAME",
            Category::PspGame => "PSP GAME",
            Category::Plugin => "PLUGIN",
            Category::Port => "PORT",
            Category::Tool => "TOOL",
            Category::Utility => "UTILITY",
            Category::Other => "OTHER",
        }
    }

    pub fn from_api(category: &str, kind: &str) -> Category {
        if kind.eq_ignore_ascii_case("plugin") {
            return Category::Plugin;
        }
        match category.trim().to_lowercase().as_str() {
            "emulator" | "emulators" => Category::Emulator,
            "original" => Category::Original,
            // PSN/NPS store "Games" / "PS Vita Game"; homebrew uses Ports/etc.
            "game" | "games" | "ps vita game" => Category::PsVitaGame,
            "ps1 game" => Category::Ps1Game,
            "psp game" => Category::PspGame,
            "plugin" | "plugins" => Category::Plugin,
            "port" | "ports" => Category::Port,
            "tool" | "tools" => Category::Tool,
            "utility" | "utilities" => Category::Utility,
            // type=psv_game is the backup when category is blank/odd.
            _ => match kind.trim().to_lowercase().as_str() {
                "psv_game" => Category::PsVitaGame,
                "psp_game" => Category::PspGame,
                "psx_game" => Category::Ps1Game,
                _ => Category::Other,
            },
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    #[default]
    Vita,
    Psp,
    Plugin,
    NpsVita,
    NpsPsp,
    NpsPsx,
}
impl Platform {
    pub fn label(self) -> &'static str {
        match self {
            Platform::Vita => "VITA",
            Platform::Psp => "PSP",
            Platform::Plugin => "PLUGIN",
            Platform::NpsVita => "PS VITA (NPS)",
            Platform::NpsPsp => "PSP (NPS)",
            Platform::NpsPsx => "PS1 (NPS)",
        }
    }
    pub fn label_short(self) -> &'static str {
        match self {
            Platform::Vita => "VITA",
            Platform::Psp => "PSP",
            Platform::Plugin => "PLUG",
            Platform::NpsVita => "PSV",
            Platform::NpsPsp => "PSP",
            Platform::NpsPsx => "PS1",
        }
    }
    pub fn from_api_type(kind: &str) -> Platform {
        match kind {
            "psp_app" => Platform::Psp,
            "plugin" => Platform::Plugin,
            "psv_game" => Platform::NpsVita,
            "psp_game" => Platform::NpsPsp,
            "psx_game" => Platform::NpsPsx,
            _ => Platform::Vita,
        }
    }
    pub fn is_nps(self) -> bool {
        matches!(self, Platform::NpsVita | Platform::NpsPsp | Platform::NpsPsx)
    }
    pub fn is_commercial(self) -> bool {
        self.is_nps()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Downloads,
    Rating,
    Recent,
    Size,
    Name,
}
impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Downloads
    }
}
impl SortOrder {
    pub const ALL: [SortOrder; 5] =
        [SortOrder::Downloads, SortOrder::Rating, SortOrder::Recent, SortOrder::Size, SortOrder::Name];
    pub fn default_direction(self) -> SortDirection {
        match self {
            SortOrder::Name => SortDirection::Asc,
            _ => SortDirection::Desc,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}
impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Desc
    }
}
impl SortDirection {
    pub fn flipped(self) -> Self {
        match self {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        }
    }
}
/// Game data this big takes ~15+ min over the Vita's Wi-Fi, so we suggest a PC download.
pub const HEAVY_DATA_BYTES: u64 = 1024 * 1024 * 1024;
pub fn is_heavy_data(entry: &AppEntry) -> bool {
    entry.data_url.is_some() && entry.data_size_bytes >= HEAVY_DATA_BYTES
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub titleid: String,
    #[serde(skip)]
    pub titleid_lower: String,
    #[serde(default)]
    pub content_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(skip)]
    pub name_lower: String,
    pub author: String,
    #[serde(skip)]
    pub author_lower: String,
    pub description: String,
    #[serde(default)]
    pub long_description: String,
    #[serde(default)]
    pub requirements: String,
    #[serde(default)]
    pub changelog: String,
    #[serde(default)]
    pub release_page: Option<String>,
    pub category: Category,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub kind: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub background_url: Option<String>,
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    pub download_url: String,
    #[serde(default)]
    pub source: Option<String>,
    pub version: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub zrif: Option<String>,
    #[serde(default)]
    pub source_catalog: String,
    #[serde(default)]
    pub source_labels: Vec<String>,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub hash2: String,
    #[serde(default)]
    pub data_url: Option<String>,
    #[serde(default)]
    pub data_extract_path: Option<String>,
    #[serde(default)]
    pub data_size_bytes: u64,
    #[serde(default)]
    pub plugin_install_path: Option<String>,
    #[serde(default)]
    pub plugin_config_section: Option<String>,
    #[serde(default)]
    pub plugin_config_line: Option<String>,
    pub size_bytes: u64,
    pub downloads: u64,
    pub rating: f32,
    pub updated_at: String,
    #[serde(default)]
    pub ratings_count: u32,
    #[serde(default)]
    pub likes_count: u32,
    #[serde(default)]
    pub comments_count: u32,
    #[serde(default)]
    pub user_liked: bool,
    #[serde(default)]
    pub user_rating: Option<u8>,
    #[serde(default)]
    pub overview: Vec<(String, String)>,
}
fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb < 1.0 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}
impl AppEntry {
    pub fn rebuild_derived(&mut self) {
        use crate::app::text;
        text::sanitize(&mut self.name);
        if let Some(original) = self.original_name.as_mut() {
            text::sanitize(original);
        }
        if self.name.trim().is_empty() {
            self.name =
                text::display_name("", self.original_name.as_deref(), &self.titleid).to_owned();
        }
        text::sanitize(&mut self.author);
        text::sanitize(&mut self.description);
        text::sanitize(&mut self.long_description);
        text::sanitize(&mut self.requirements);
        text::sanitize(&mut self.changelog);
        text::sanitize(&mut self.version);
        self.genres.retain_mut(|genre| {
            text::sanitize(genre);
            !genre.trim().is_empty()
        });
        self.genres.sort_unstable();
        self.genres.dedup();
        for (key, value) in &mut self.overview {
            text::sanitize(key);
            text::sanitize(value);
        }
        self.name_lower = self.name.to_lowercase();
        self.author_lower = self.author.to_lowercase();
        self.titleid_lower = self.titleid.to_lowercase();
    }
    pub fn size_label(&self) -> String {
        format_size(self.size_bytes)
    }
    pub fn sort_epoch(&self) -> u32 {
        let bytes = self.updated_at.as_bytes();
        if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return 0;
        }
        let digits = |s: &str| s.parse::<u32>().ok();
        let year = digits(&self.updated_at[0..4]);
        let month = digits(&self.updated_at[5..7]);
        let day = digits(&self.updated_at[8..10]);
        match (year, month, day) {
            (Some(y), Some(m), Some(d)) => y * 10000 + m * 100 + d,
            _ => 0,
        }
    }
    pub fn data_size_label(&self) -> String {
        format_size(self.data_size_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psn_type_maps_to_game_category_without_source_tag() {
        assert_eq!(Category::from_api("", "psv_game"), Category::PsVitaGame);
        assert_eq!(Category::from_api("Games", ""), Category::PsVitaGame);
        assert_eq!(Category::from_api("PS Vita Game", "psv_game"), Category::PsVitaGame);
        assert_eq!(Category::from_api("Ports", ""), Category::Port);
        assert_eq!(Category::from_api("junk", "psv_game"), Category::PsVitaGame);
        assert_eq!(Platform::from_api_type("psv_game").is_commercial(), true);
    }
}

