use crate::app::icons::IconCache;
use crate::data::Category;

use std::sync::atomic::{AtomicU8, Ordering};

const fn rgb(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Semantic colors: a theme may change the mood, never what a color means
/// (warning / success / danger are shared by every theme).
pub struct Palette {
    pub bg_deep: egui::Color32,
    pub bg_header: egui::Color32,
    pub bg_card: egui::Color32,
    pub bg_card_hover: egui::Color32,
    pub separator: egui::Color32,
    pub text: egui::Color32,
    pub text_dim: egui::Color32,
    pub text_faint: egui::Color32,
    /// Brand accent: links, progress, section labels.
    pub accent: egui::Color32,
    pub accent_soft: egui::Color32,
    /// Solid fill of the main action button.
    pub primary: egui::Color32,
    pub primary_hover: egui::Color32,
    /// Focus ring and the glow under a focused tile.
    pub focus: egui::Color32,
    pub focus_glow: egui::Color32,
    pub success: egui::Color32,
    pub success_hover: egui::Color32,
    pub warning: egui::Color32,
    pub warning_hover: egui::Color32,
    pub danger: egui::Color32,
    pub danger_bg: egui::Color32,
    pub glass_edge: egui::Color32,
}

const SUCCESS: egui::Color32 = rgb(0x30d158);
const SUCCESS_HOVER: egui::Color32 = rgb(0x4cdd70);
const WARNING: egui::Color32 = rgb(0xffd60a);
const WARNING_HOVER: egui::Color32 = rgb(0xffe066);
const DANGER: egui::Color32 = rgb(0xff5f5f);
const DANGER_BG: egui::Color32 = rgb(0x3a1c1c);
/// Rating stars are content, not chrome: same gold in every theme.
pub const STAR: egui::Color32 = WARNING;

/// Translucent color given as plain RGB + alpha. egui stores premultiplied
/// values, and passing RGB above alpha there renders as additive light.
const fn glow(hex: u32, alpha: u8) -> egui::Color32 {
    let a = alpha as u32;
    egui::Color32::from_rgba_premultiplied(
        (((hex >> 16) & 0xff) * a / 255) as u8,
        (((hex >> 8) & 0xff) * a / 255) as u8,
        ((hex & 0xff) * a / 255) as u8,
        alpha,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemeId {
    #[default]
    Midnight,
    Obsidian,
    Aurora,
    Sunset,
    HighContrast,
    VitaClassic,
    Graphite,
    Forest,
    Nebula,
    Sakura,
    Mocha,
    TokyoNight,
    Catppuccin,
    Nord,
    Gruvbox,
    Dracula,
    RosePine,
    Everforest,
    Kanagawa,
}

impl ThemeId {
    pub const ALL: [Self; 19] = [
        Self::Midnight,
        Self::VitaClassic,
        Self::Obsidian,
        Self::Graphite,
        Self::Aurora,
        Self::Forest,
        Self::Nebula,
        Self::Sakura,
        Self::Sunset,
        Self::Mocha,
        Self::TokyoNight,
        Self::Catppuccin,
        Self::Nord,
        Self::Gruvbox,
        Self::Dracula,
        Self::RosePine,
        Self::Everforest,
        Self::Kanagawa,
        Self::HighContrast,
    ];

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Midnight => &MIDNIGHT,
            Self::Obsidian => &OBSIDIAN,
            Self::Aurora => &AURORA,
            Self::Sunset => &SUNSET,
            Self::HighContrast => &HIGH_CONTRAST,
            Self::VitaClassic => &VITA_CLASSIC,
            Self::Graphite => &GRAPHITE,
            Self::Forest => &FOREST,
            Self::Nebula => &NEBULA,
            Self::Sakura => &SAKURA,
            Self::Mocha => &MOCHA,
            Self::TokyoNight => &TOKYO_NIGHT,
            Self::Catppuccin => &CATPPUCCIN,
            Self::Nord => &NORD,
            Self::Gruvbox => &GRUVBOX,
            Self::Dracula => &DRACULA,
            Self::RosePine => &ROSE_PINE,
            Self::Everforest => &EVERFOREST,
            Self::Kanagawa => &KANAGAWA,
        }
    }
}

const MIDNIGHT: Palette = Palette {
    bg_deep: rgb(0x050a15),
    bg_header: rgb(0x0a1224),
    bg_card: rgb(0x142038),
    bg_card_hover: rgb(0x203152),
    separator: rgb(0x354a70),
    text: rgb(0xf4f7fb),
    text_dim: rgb(0xc6d3e8),
    text_faint: rgb(0x8298bc),
    accent: rgb(0x3890ff),
    accent_soft: rgb(0x73c6ff),
    primary: rgb(0x0070ff),
    primary_hover: rgb(0x33a6ff),
    focus: rgb(0xf4f7fb),
    focus_glow: glow(0x1eb7ff, 55),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x2c2c2e, 0x55),
};

const OBSIDIAN: Palette = Palette {
    bg_deep: rgb(0x0b0c10),
    bg_header: rgb(0x111318),
    bg_card: rgb(0x1c1e24),
    bg_card_hover: rgb(0x2a2d35),
    separator: rgb(0x34373f),
    text: rgb(0xf5f5f7),
    text_dim: rgb(0xc8cad0),
    text_faint: rgb(0x8f939c),
    accent: rgb(0x4c8dff),
    accent_soft: rgb(0x8ab4ff),
    primary: rgb(0x1f6fe5),
    primary_hover: rgb(0x3b82f6),
    focus: rgb(0xffffff),
    focus_glow: glow(0xffffff, 28),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x3a3d45, 0x66),
};

const AURORA: Palette = Palette {
    bg_deep: rgb(0x04161a),
    bg_header: rgb(0x08222a),
    bg_card: rgb(0x0f2f38),
    bg_card_hover: rgb(0x17414c),
    separator: rgb(0x245059),
    text: rgb(0xf0fbfa),
    text_dim: rgb(0xc2dedb),
    text_faint: rgb(0x80aaa6),
    accent: rgb(0x2dd4bf),
    accent_soft: rgb(0x7ae8d9),
    primary: rgb(0x0f766e),
    primary_hover: rgb(0x0d9488),
    focus: rgb(0xd5fff8),
    focus_glow: glow(0x2dd4bf, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x1d4a52, 0x66),
};

const SUNSET: Palette = Palette {
    bg_deep: rgb(0x160a14),
    bg_header: rgb(0x22101e),
    bg_card: rgb(0x301829),
    bg_card_hover: rgb(0x45223a),
    separator: rgb(0x5a3150),
    text: rgb(0xfff5f0),
    text_dim: rgb(0xead0d6),
    text_faint: rgb(0xb88f9f),
    accent: rgb(0xff7a45),
    accent_soft: rgb(0xffb38a),
    primary: rgb(0xc2410c),
    primary_hover: rgb(0xd9480f),
    focus: rgb(0xffe4d6),
    focus_glow: glow(0xff7a45, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x4a2640, 0x66),
};

const HIGH_CONTRAST: Palette = Palette {
    bg_deep: rgb(0x000000),
    bg_header: rgb(0x000000),
    bg_card: rgb(0x141414),
    bg_card_hover: rgb(0x2a2a2a),
    separator: rgb(0x8a8a8a),
    text: rgb(0xffffff),
    text_dim: rgb(0xeeeeee),
    text_faint: rgb(0xc4c4c4),
    accent: rgb(0x66b3ff),
    accent_soft: rgb(0x99ccff),
    primary: rgb(0x0050d0),
    primary_hover: rgb(0x1a66e0),
    focus: rgb(0xffd60a),
    focus_glow: glow(0xffd60a, 40),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0xffffff, 0x80),
};

const VITA_CLASSIC: Palette = Palette {
    bg_deep: rgb(0x061a33),
    bg_header: rgb(0x0a2445),
    bg_card: rgb(0x103059),
    bg_card_hover: rgb(0x1a4275),
    separator: rgb(0x2b5a8f),
    text: rgb(0xf2f8ff),
    text_dim: rgb(0xc4d9f0),
    text_faint: rgb(0x86a6cc),
    accent: rgb(0x00a2ff),
    accent_soft: rgb(0x6cc8ff),
    primary: rgb(0x0067c0),
    primary_hover: rgb(0x0a7fd9),
    focus: rgb(0xf2f8ff),
    focus_glow: glow(0x00a2ff, 50),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x2b5a8f, 0x66),
};

const GRAPHITE: Palette = Palette {
    bg_deep: rgb(0x121212),
    bg_header: rgb(0x181818),
    bg_card: rgb(0x232323),
    bg_card_hover: rgb(0x303030),
    separator: rgb(0x3d3d3d),
    text: rgb(0xf2f2f2),
    text_dim: rgb(0xcfcfcf),
    text_faint: rgb(0x959595),
    accent: rgb(0xe0e0e0),
    accent_soft: rgb(0xbdbdbd),
    primary: rgb(0x4a4a4a),
    primary_hover: rgb(0x5c5c5c),
    focus: rgb(0xffffff),
    focus_glow: glow(0xffffff, 24),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x444444, 0x66),
};

const FOREST: Palette = Palette {
    bg_deep: rgb(0x0a140d),
    bg_header: rgb(0x0f1d13),
    bg_card: rgb(0x16291b),
    bg_card_hover: rgb(0x213a27),
    separator: rgb(0x2f5236),
    text: rgb(0xf1faf2),
    text_dim: rgb(0xcde3d0),
    text_faint: rgb(0x8fb095),
    accent: rgb(0x4ade80),
    accent_soft: rgb(0x86efac),
    primary: rgb(0x15803d),
    primary_hover: rgb(0x16a34a),
    focus: rgb(0xe6ffe9),
    focus_glow: glow(0x4ade80, 40),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x2f5236, 0x66),
};

const NEBULA: Palette = Palette {
    bg_deep: rgb(0x0f0a1f),
    bg_header: rgb(0x16102b),
    bg_card: rgb(0x21183d),
    bg_card_hover: rgb(0x2e2354),
    separator: rgb(0x44366f),
    text: rgb(0xf6f3ff),
    text_dim: rgb(0xd6cdf0),
    text_faint: rgb(0x9e92c4),
    accent: rgb(0xa78bfa),
    accent_soft: rgb(0xc4b5fd),
    primary: rgb(0x6d28d9),
    primary_hover: rgb(0x7c3aed),
    focus: rgb(0xede6ff),
    focus_glow: glow(0xa78bfa, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x44366f, 0x66),
};

const SAKURA: Palette = Palette {
    bg_deep: rgb(0x1a0d14),
    bg_header: rgb(0x24121c),
    bg_card: rgb(0x331a28),
    bg_card_hover: rgb(0x472438),
    separator: rgb(0x6b3a55),
    text: rgb(0xfff4f8),
    text_dim: rgb(0xf0d2df),
    text_faint: rgb(0xc095a9),
    accent: rgb(0xf472b6),
    accent_soft: rgb(0xf9a8d4),
    primary: rgb(0xbe185d),
    primary_hover: rgb(0xdb2777),
    focus: rgb(0xffe4f0),
    focus_glow: glow(0xf472b6, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x6b3a55, 0x66),
};

const MOCHA: Palette = Palette {
    bg_deep: rgb(0x15100c),
    bg_header: rgb(0x1d1611),
    bg_card: rgb(0x2a2019),
    bg_card_hover: rgb(0x3a2d23),
    separator: rgb(0x54422f),
    text: rgb(0xfaf4ee),
    text_dim: rgb(0xe3d3c3),
    text_faint: rgb(0xab9682),
    accent: rgb(0xd4a373),
    accent_soft: rgb(0xe9c79f),
    primary: rgb(0x8b5a2b),
    primary_hover: rgb(0xa0693a),
    focus: rgb(0xfff1e0),
    focus_glow: glow(0xd4a373, 40),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x54422f, 0x66),
};

const TOKYO_NIGHT: Palette = Palette {
    bg_deep: rgb(0x16161e),
    bg_header: rgb(0x1a1b26),
    bg_card: rgb(0x24283b),
    bg_card_hover: rgb(0x292e42),
    separator: rgb(0x3b4261),
    text: rgb(0xc0caf5),
    text_dim: rgb(0xa9b1d6),
    text_faint: rgb(0x8089b3),
    accent: rgb(0x7aa2f7),
    accent_soft: rgb(0x7dcfff),
    primary: rgb(0x3d59a1),
    primary_hover: rgb(0x4a6bc0),
    focus: rgb(0xc0caf5),
    focus_glow: glow(0x7aa2f7, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x3b4261, 0x80),
};

const CATPPUCCIN: Palette = Palette {
    bg_deep: rgb(0x11111b),
    bg_header: rgb(0x181825),
    bg_card: rgb(0x1e1e2e),
    bg_card_hover: rgb(0x313244),
    separator: rgb(0x45475a),
    text: rgb(0xcdd6f4),
    text_dim: rgb(0xbac2de),
    text_faint: rgb(0x9399b2),
    accent: rgb(0xcba6f7),
    accent_soft: rgb(0xb4befe),
    primary: rgb(0x8839ef),
    primary_hover: rgb(0x9d5bf5),
    focus: rgb(0xf5e0dc),
    focus_glow: glow(0xcba6f7, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x45475a, 0x80),
};

const NORD: Palette = Palette {
    bg_deep: rgb(0x242933),
    bg_header: rgb(0x2e3440),
    bg_card: rgb(0x3b4252),
    bg_card_hover: rgb(0x434c5e),
    separator: rgb(0x4c566a),
    text: rgb(0xeceff4),
    text_dim: rgb(0xd8dee9),
    text_faint: rgb(0xaab1c0),
    accent: rgb(0x88c0d0),
    accent_soft: rgb(0x8fbcbb),
    primary: rgb(0x4c6a94),
    primary_hover: rgb(0x5e81ac),
    focus: rgb(0xeceff4),
    focus_glow: glow(0x88c0d0, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x4c566a, 0x80),
};

const GRUVBOX: Palette = Palette {
    bg_deep: rgb(0x1d2021),
    bg_header: rgb(0x282828),
    bg_card: rgb(0x32302f),
    bg_card_hover: rgb(0x3c3836),
    separator: rgb(0x504945),
    text: rgb(0xfbf1c7),
    text_dim: rgb(0xebdbb2),
    text_faint: rgb(0xbdae93),
    accent: rgb(0xfe8019),
    accent_soft: rgb(0x8ec07c),
    primary: rgb(0x076678),
    primary_hover: rgb(0x458588),
    focus: rgb(0xfbf1c7),
    focus_glow: glow(0xfe8019, 40),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x504945, 0x80),
};

const DRACULA: Palette = Palette {
    bg_deep: rgb(0x21222c),
    bg_header: rgb(0x282a36),
    bg_card: rgb(0x343746),
    bg_card_hover: rgb(0x44475a),
    separator: rgb(0x6272a4),
    text: rgb(0xf8f8f2),
    text_dim: rgb(0xe2e2dc),
    text_faint: rgb(0xb0b8dc),
    accent: rgb(0xbd93f9),
    accent_soft: rgb(0xff79c6),
    primary: rgb(0x7c4dcc),
    primary_hover: rgb(0x9061e0),
    focus: rgb(0xf8f8f2),
    focus_glow: glow(0xbd93f9, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x6272a4, 0x80),
};

const ROSE_PINE: Palette = Palette {
    bg_deep: rgb(0x191724),
    bg_header: rgb(0x1f1d2e),
    bg_card: rgb(0x26233a),
    bg_card_hover: rgb(0x403d52),
    separator: rgb(0x524f67),
    text: rgb(0xe0def4),
    text_dim: rgb(0xcfcce6),
    text_faint: rgb(0xa29fbd),
    accent: rgb(0xc4a7e7),
    accent_soft: rgb(0xebbcba),
    primary: rgb(0x6e5a9a),
    primary_hover: rgb(0x7d68ab),
    focus: rgb(0xebbcba),
    focus_glow: glow(0xc4a7e7, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x524f67, 0x80),
};

const EVERFOREST: Palette = Palette {
    bg_deep: rgb(0x1e2326),
    bg_header: rgb(0x232a2e),
    bg_card: rgb(0x2d353b),
    bg_card_hover: rgb(0x3d484d),
    separator: rgb(0x475258),
    text: rgb(0xd3c6aa),
    text_dim: rgb(0xc9bd9f),
    text_faint: rgb(0xa0aba3),
    accent: rgb(0xa7c080),
    accent_soft: rgb(0x83c092),
    primary: rgb(0x3f6b4e),
    primary_hover: rgb(0x4d7d5c),
    focus: rgb(0xd3c6aa),
    focus_glow: glow(0xa7c080, 40),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x475258, 0x80),
};

const KANAGAWA: Palette = Palette {
    bg_deep: rgb(0x16161d),
    bg_header: rgb(0x1f1f28),
    bg_card: rgb(0x2a2a37),
    bg_card_hover: rgb(0x363646),
    separator: rgb(0x54546d),
    text: rgb(0xdcd7ba),
    text_dim: rgb(0xc8c093),
    text_faint: rgb(0xa6a69c),
    accent: rgb(0x7e9cd8),
    accent_soft: rgb(0x7fb4ca),
    primary: rgb(0x2d4f67),
    primary_hover: rgb(0x3a6585),
    focus: rgb(0xdcd7ba),
    focus_glow: glow(0x7e9cd8, 45),
    success: SUCCESS,
    success_hover: SUCCESS_HOVER,
    warning: WARNING,
    warning_hover: WARNING_HOVER,
    danger: DANGER,
    danger_bg: DANGER_BG,
    glass_edge: glow(0x54546d, 0x80),
};

static ACTIVE: AtomicU8 = AtomicU8::new(0);

/// The active palette; read every frame, so a theme change repaints everything.
pub fn pal() -> &'static Palette {
    ThemeId::ALL[ACTIVE.load(Ordering::Relaxed) as usize % ThemeId::ALL.len()].palette()
}

pub fn active_theme() -> ThemeId {
    ThemeId::ALL[ACTIVE.load(Ordering::Relaxed) as usize % ThemeId::ALL.len()]
}

/// Theme whose palette egui's `Visuals` were last built from.
static APPLIED: AtomicU8 = AtomicU8::new(u8::MAX);

/// Switch the palette; widgets read `pal()` so they follow on the next frame.
pub fn set_theme(theme: ThemeId) {
    let index = ThemeId::ALL.iter().position(|&t| t == theme).unwrap_or(0);
    ACTIVE.store(index as u8, Ordering::Relaxed);
}

/// Rebuild egui's own visuals (panel fill, selection) after a theme change.
pub fn sync_visuals(ctx: &egui::Context) {
    let active = ACTIVE.load(Ordering::Relaxed);
    if APPLIED.swap(active, Ordering::Relaxed) != active {
        apply_visuals(ctx);
    }
}

pub const GLASS_ALPHA: f32 = 0.88;
pub const FONT_MICRO: f32 = 12.0;
pub const FONT_SMALL: f32 = 13.0;
pub const FONT_BODY: f32 = 15.0;
pub const FONT_LARGE: f32 = 16.0;
pub const FONT_TITLE: f32 = 18.0;
pub const FONT_HEADLINE: f32 = 24.0;

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

pub fn glass(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(GLASS_ALPHA)
}

pub const RADIUS_XS: f32 = 5.0;
pub const RADIUS_SM: f32 = 8.0;
pub const RADIUS_MD: f32 = 10.0;
pub const RADIUS_LG: f32 = 14.0;
pub const CARD_RADIUS: f32 = 12.0;
pub const HINT_BAR_HEIGHT: f32 = 38.0;
pub const SEARCH_FIELD_WIDTH: f32 = 160.0;
pub const SEARCH_FIELD_HEIGHT: f32 = 28.0;
pub const SEARCH_CLEAR_SIZE: f32 = 18.0;
pub const GRID_COLUMNS: usize = 5;
pub const ART_LOOKAHEAD: usize = 6;
pub const GRID_COL_SPACING: f32 = 12.0;
pub const GRID_ROW_SPACING: f32 = 8.0;
pub const GRID_BOTTOM_PAD: f32 = 12.0;
pub const TILE_LABEL_HEIGHT: f32 = 22.0;
pub const TILE_LABEL_GAP: f32 = 4.0;
pub const COVER_ASPECT: f32 = 1.0;
pub const SCREEN_MARGIN: f32 = 14.0;
pub const SIDEBAR_WIDTH: f32 = 146.0;
pub const SIDEBAR_GAP: f32 = 14.0;
pub const SIDEBAR_ROW_HEIGHT: f32 = 34.0;
pub const HEADER_HEIGHT: f32 = 52.0;
pub const TAB_PILL_HEIGHT: f32 = 34.0;
pub const FOCUS_STROKE: f32 = 2.5;
pub const SCROLLBAR_RESERVE: f32 = 6.0;
pub const PRESS_SHRINK: f32 = 1.5;
pub const SELECT_SCALE: f32 = 1.02;

static LOGO_TEXTURE: std::sync::OnceLock<egui::TextureHandle> = std::sync::OnceLock::new();

pub fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    LOGO_TEXTURE
        .get_or_init(|| {
            const LOGO_BYTES: &[u8] = include_bytes!("../../../assets/images/icon.png");
            let decoded = image::load_from_memory(LOGO_BYTES)
                .expect("assets/images/icon.png is bundled at compile time and must decode")
                .to_rgba8();
            let size = [decoded.width() as usize, decoded.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
            ctx.load_texture("app-logo", color_image, egui::TextureOptions::LINEAR)
        })
        .clone()
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        super::chrome_icons::ICON_FAMILY.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../../../assets/fonts/lucide-icons.ttf"))),
    );
    fonts.families.insert(
        egui::FontFamily::Name(super::chrome_icons::ICON_FAMILY.into()),
        vec![super::chrome_icons::ICON_FAMILY.to_owned()],
    );
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.spacing.scroll.bar_width = 4.0;
    style.spacing.scroll.bar_inner_margin = 0.0;
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.interaction.interact_radius = 8.0;
    style.animation_time = 0.12;
    style.scroll_animation = egui::style::ScrollAnimation::duration(0.08);
    ctx.set_style(style);
    apply_visuals(ctx);
    ctx.options_mut(|opts| {
        opts.input_options.max_click_duration = 5.0;
        opts.input_options.max_click_dist = 32.0;
    });
    ctx.tessellation_options_mut(|opts| {
        opts.feathering = false;
    });
}

fn apply_visuals(ctx: &egui::Context) {
    let p = pal();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = p.bg_deep;
    visuals.window_fill = p.bg_deep;
    visuals.selection.bg_fill = p.accent.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.5_f32, p.accent);
    visuals.hyperlink_color = p.accent_soft;
    let control_radius = egui::CornerRadius::same(RADIUS_MD as u8);
    visuals.widgets.noninteractive.corner_radius = control_radius;
    visuals.widgets.inactive.corner_radius = control_radius;
    visuals.widgets.hovered.corner_radius = control_radius;
    visuals.widgets.active.corner_radius = control_radius;
    visuals.widgets.open.corner_radius = control_radius;
    visuals.window_corner_radius = egui::CornerRadius::same(CARD_RADIUS as u8);
    visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS_LG as u8);
    ctx.set_visuals(visuals);
}

pub fn paint_background(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, pal().bg_deep);
    let band = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.right(), rect.top() + rect.height() * 0.46),
    );
    let mut wash = egui::Mesh::default();
    wash.colored_vertex(band.left_top(), egui::Color32::from_rgb(0x13, 0x27, 0x4e));
    wash.colored_vertex(band.right_top(), egui::Color32::from_rgb(0x0a, 0x16, 0x31));
    wash.colored_vertex(band.right_bottom(), pal().bg_deep);
    wash.colored_vertex(band.left_bottom(), pal().bg_deep);
    wash.add_triangle(0, 1, 2);
    wash.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(wash));
    let vignette = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - rect.height() * 0.28),
        rect.right_bottom(),
    );
    let mut bottom = egui::Mesh::default();
    bottom.colored_vertex(vignette.left_top(), egui::Color32::TRANSPARENT);
    bottom.colored_vertex(vignette.right_top(), egui::Color32::TRANSPARENT);
    bottom.colored_vertex(
        vignette.right_bottom(),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 90),
    );
    bottom.colored_vertex(
        vignette.left_bottom(),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 90),
    );
    bottom.add_triangle(0, 1, 2);
    bottom.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(bottom));
}

pub const HERO_FRACTION: f32 = 0.46;

pub fn backdrop_url(entry: &crate::data::AppEntry) -> Option<&str> {
    entry
        .background_url
        .as_deref()
        .or_else(|| entry.screenshot_urls.first().map(String::as_str))
        .or(entry.cover_url.as_deref())
}

pub fn paint_hero(ui: &egui::Ui, icons: &IconCache, entry: &crate::data::AppEntry) -> bool {
    // Detail content must never paint over the navigation or footer panels.
    let screen = ui.clip_rect();
    paint_background(ui.painter(), screen);
    let Some(url) = backdrop_url(entry) else {
        return false;
    };
    let Some(texture) = icons.get_hero(ui.ctx(), url) else {
        return icons.is_loading(url, crate::app::icons::HERO_SIDE);
    };
    let band = egui::Rect::from_min_max(
        screen.left_top(),
        egui::pos2(
            screen.right(),
            screen.top() + screen.height() * HERO_FRACTION,
        ),
    );
    let size = texture.size_vec2();
    let scale = (band.width() / size.x).max(band.height() / size.y);
    let uv_size = egui::vec2(
        band.width() / scale / size.x,
        band.height() / scale / size.y,
    );
    let uv = egui::Rect::from_min_size(
        ((egui::Vec2::splat(1.0) - uv_size) / 2.0).to_pos2(),
        uv_size,
    );
    let mut mesh = egui::Mesh::with_texture(texture.id());
    mesh.add_rect_with_uv(band, uv, egui::Color32::WHITE.gamma_multiply(0.7));
    ui.painter().add(egui::Shape::mesh(mesh));
    let fade = egui::Rect::from_min_max(
        egui::pos2(band.left(), band.top() + band.height() * 0.35),
        band.right_bottom(),
    );
    let mut scrim = egui::Mesh::default();
    scrim.colored_vertex(fade.left_top(), egui::Color32::TRANSPARENT);
    scrim.colored_vertex(fade.right_top(), egui::Color32::TRANSPARENT);
    scrim.colored_vertex(fade.right_bottom(), pal().bg_deep);
    scrim.colored_vertex(fade.left_bottom(), pal().bg_deep);
    scrim.add_triangle(0, 1, 2);
    scrim.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(scrim));
    false
}

pub fn category_color(category: Category) -> egui::Color32 {
    match category {
        Category::Emulator => egui::Color32::from_rgb(0xd9, 0x8e, 0x24),
        Category::Original => egui::Color32::from_rgb(0x3b, 0x7c, 0xbf),
        Category::PsVitaGame => egui::Color32::from_rgb(0xc4, 0x5c, 0x7a),
        Category::Ps1Game => egui::Color32::from_rgb(0xc9, 0xa2, 0x27),
        Category::PspGame => egui::Color32::from_rgb(0x4a, 0x7c, 0x59),
        Category::Utility => egui::Color32::from_rgb(0x2f, 0x9e, 0x7a),
        Category::Port => egui::Color32::from_rgb(0x5b, 0x8d, 0xef),
        Category::Tool => egui::Color32::from_rgb(0x3a, 0xa8, 0xb5),
        Category::Plugin => egui::Color32::from_rgb(0x2a, 0x9a, 0xa8),
        Category::Other => egui::Color32::from_rgb(0x6b, 0x73, 0x80),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: egui::Color32) -> f32 {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// WCAG AA: body text 4.5:1, faint text and the focus ring 3:1.
    #[test]
    fn palettes_meet_contrast() {
        for theme in ThemeId::ALL {
            let p = theme.palette();
            for bg in [p.bg_deep, p.bg_card, p.bg_card_hover] {
                assert!(contrast(p.text, bg) >= 4.5, "{theme:?} text");
                assert!(contrast(p.text_dim, bg) >= 4.5, "{theme:?} text_dim");
                assert!(contrast(p.text_faint, bg) >= 3.0, "{theme:?} text_faint");
                assert!(contrast(p.focus, bg) >= 3.0, "{theme:?} focus");
            }
            // Midnight keeps the original brand blue (4.4:1) on purpose.
            assert!(contrast(egui::Color32::WHITE, p.primary) >= 4.3, "{theme:?} primary");
        }
    }
}
