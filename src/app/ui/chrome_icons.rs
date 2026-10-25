use crate::data::Category;
use crate::input::ContentTypeGroup;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeIcon {
    Home,
    Games,
    Apps,
    Emulators,
    Plugins,
    All,
    Vita,
    Psp,
    Ps1,
    Ports,
    Homebrew,
    Featured,
    Downloads,
    Recent,
    Favorites,
    Search,
    Library,
    Updates,
    GameData,
    UpdateAvailable,
    Installed,
    Storage,
    Alert,
}

impl ChromeIcon {
    pub fn for_group(group: ContentTypeGroup) -> Self {
        match group {
            ContentTypeGroup::Home => Self::Home,
            ContentTypeGroup::Games => Self::Games,
            ContentTypeGroup::Apps => Self::Apps,
            ContentTypeGroup::Emulators => Self::Emulators,
            ContentTypeGroup::Plugins => Self::Plugins,
        }
    }
    pub fn for_store_tab(tab: crate::input::StoreTab) -> Self {
        match tab {
            crate::input::StoreTab::Categories => Self::All,
            crate::input::StoreTab::Library => Self::Library,
            crate::input::StoreTab::Updates => Self::Updates,
        }
    }
    pub fn for_category(category: Category) -> Self {
        match category {
            Category::PsVitaGame => Self::Vita,
            Category::PspGame => Self::Psp,
            Category::Ps1Game => Self::Ps1,
            Category::Port => Self::Ports,
            Category::Original => Self::Homebrew,
            Category::Utility | Category::Tool => Self::Apps,
            Category::Emulator => Self::Emulators,
            Category::Plugin => Self::Plugins,
            Category::Other => Self::All,
        }
    }
}

pub const ICON_FAMILY: &str = "lucide";

impl ChromeIcon {
    fn glyph(self) -> char {
        match self {
            Self::Home => '\u{e0f8}',
            Self::Games | Self::Vita | Self::Psp => '\u{e0e2}',
            Self::Apps | Self::All => '\u{e105}',
            Self::Emulators => '\u{e0ad}',
            Self::Plugins => '\u{e29c}',
            Self::Ps1 => '\u{e499}',
            Self::Ports => '\u{e12d}',
            Self::Homebrew => '\u{e0da}',
            Self::Featured => '\u{e17a}',
            Self::Downloads => '\u{e0b6}',
            Self::Recent => '\u{e08b}',
            Self::Favorites => '\u{e0f5}',
            Self::Search => '\u{e155}',
            Self::Library => '\u{e106}',
            Self::Updates => '\u{e149}',
            Self::GameData => '\u{e4ea}',
            Self::UpdateAvailable => '\u{e07f}',
            Self::Installed => '\u{e266}',
            Self::Storage => '\u{e0f0}',
            Self::Alert => '\u{e193}',
        }
    }
}

pub fn paint(ui: &egui::Ui, icon: ChromeIcon, rect: egui::Rect, tint: egui::Color32) {
    let side = rect.width().min(rect.height());
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.glyph(),
        egui::FontId::new(side.round(), egui::FontFamily::Name(ICON_FAMILY.into())),
        tint,
    );
}

pub const SIDEBAR_ICON: f32 = 17.0;
pub const NAV_ICON: f32 = 17.0;
pub const SEARCH_ICON: f32 = 16.0;
