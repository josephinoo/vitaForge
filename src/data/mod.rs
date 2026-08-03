pub mod source;
mod vitadbtoo;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    #[default]
    Vita,

    Psp,

    Plugin,
}

impl Platform {
    pub fn label(self) -> &'static str {
        match self {
            Platform::Vita => "VITA",
            Platform::Psp => "PSP",
            Platform::Plugin => "PLUGIN",
        }
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
    pub name: String,

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
    pub screenshot_urls: Vec<String>,
    pub download_url: String,
    #[serde(default)]
    pub source: Option<String>,
    pub version: String,

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
        self.name_lower = self.name.to_lowercase();
    }

    pub fn size_label(&self) -> String {
        format_size(self.size_bytes)
    }

    pub fn data_size_label(&self) -> String {
        format_size(self.data_size_bytes)
    }
}
