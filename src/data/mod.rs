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

    /// Literals, to avoid a `to_uppercase()` per card per frame.
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
    /// Precomputed: lowercasing 1000+ names per keystroke was the main typing lag.
    #[serde(skip)]
    pub name_lower: String,
    pub author: String,
    pub description: String,
    pub category: Category,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
    pub download_url: String,
    #[serde(default)]
    pub source: Option<String>,
    pub version: String,
    /// MD5 the catalog expects `eboot.bin` to have; empty if it ships none.
    #[serde(default)]
    pub hash: String,
    pub size_bytes: u64,
    pub downloads: u64,
    pub rating: f32,
    pub updated_at: String,
}

impl AppEntry {
    /// Refills fields skipped by serde after loading from the on-disk cache.
    pub fn rebuild_derived(&mut self) {
        self.name_lower = self.name.to_lowercase();
    }

    pub fn size_label(&self) -> String {
        let mb = self.size_bytes as f64 / (1024.0 * 1024.0);
        if mb < 1.0 {
            let kb = self.size_bytes as f64 / 1024.0;
            format!("{kb:.0} KB")
        } else {
            format!("{mb:.1} MB")
        }
    }
}
