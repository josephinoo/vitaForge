pub mod api;
pub mod client_id;
pub mod source;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Game,
    Emulator,
    Utility,
    Port,
    Plugin,
    Media,
    Theme,
    Other,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Game,
        Category::Emulator,
        Category::Utility,
        Category::Port,
        Category::Plugin,
        Category::Media,
        Category::Theme,
        Category::Other,
    ];

    pub fn label_upper(self) -> &'static str {
        match self {
            Category::Game => "GAMES",
            Category::Emulator => "EMULATORS",
            Category::Utility => "UTILITIES",
            Category::Port => "PORTS",
            Category::Plugin => "PLUGINS",
            Category::Media => "MEDIA",
            Category::Theme => "THEMES",
            Category::Other => "OTHER",
        }
    }

    /// Maps the API's free-form `category`/`type` strings onto our fixed filter set.
    pub fn from_api(category: &str, kind: &str) -> Category {
        if kind.eq_ignore_ascii_case("plugin") {
            return Category::Plugin;
        }
        match category.trim().to_lowercase().as_str() {
            "game" | "games" => Category::Game,
            "emulator" | "emulators" => Category::Emulator,
            "utility" | "utilities" | "tool" | "tools" => Category::Utility,
            "port" | "ports" => Category::Port,
            "plugin" | "plugins" => Category::Plugin,
            "media" => Category::Media,
            "theme" | "themes" => Category::Theme,
            _ => Category::Other,
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
    pub const ALL: [Platform; 5] = [
        Platform::Vita,
        Platform::Psp,
        Platform::NpsPsx,
        Platform::NpsVita, // we will display this or map to PSV/PS1/PSP console menu
        Platform::Plugin,
    ];

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Downloads,
    Rating,
    Recent,
    Size,
    NameAsc,
}

impl SortOrder {
    pub const ALL: [SortOrder; 5] =
        [SortOrder::Downloads, SortOrder::Rating, SortOrder::Recent, SortOrder::Size, SortOrder::NameAsc];

    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Downloads => "Most downloaded",
            SortOrder::Rating => "Top rated",
            SortOrder::Recent => "Recently updated",
            SortOrder::Size => "Size",
            SortOrder::NameAsc => "A-Z",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub titleid: String,

    /// PSN content ID (e.g. `UP0001-NPXS10001_00-...`), used to build the fake
    /// NoNpDrm/NoPspEmuDrm license. Falls back to a value derived from
    /// `titleid` + `region` when the catalog doesn't provide one.
    #[serde(default)]
    pub content_id: Option<String>,
    pub name: String,

    /// Untranslated title, when the catalog carries one. Only used as a fallback
    /// for names that sanitize down to nothing.
    #[serde(default)]
    pub original_name: Option<String>,

    #[serde(skip)]
    pub name_lower: String,
    pub author: String,
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
    pub platform: Platform,
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
    pub data_size_bytes: u64,
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

    /// Publisher / developer / system, as scraped for NPS titles.
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

    /// Normalises every string the UI will draw and recomputes the search key.
    ///
    /// Runs on both catalog paths — freshly fetched entries and ones restored
    /// from `cache.json`, which was written before sanitizing existed. The
    /// sanitizer is idempotent, so applying it twice costs nothing.
    pub fn rebuild_derived(&mut self) {
        use crate::app::text;

        text::sanitize(&mut self.name);
        if let Some(original) = self.original_name.as_mut() {
            text::sanitize(original);
        }
        // A Japanese or Korean title sanitizes to nothing; resolve the fallback
        // once here so no draw site has to think about empty names.
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
        for (key, value) in &mut self.overview {
            text::sanitize(key);
            text::sanitize(value);
        }

        self.name_lower = self.name.to_lowercase();
    }

    pub fn size_label(&self) -> String {
        format_size(self.size_bytes)
    }

    pub fn data_size_label(&self) -> String {
        format_size(self.data_size_bytes)
    }
}
