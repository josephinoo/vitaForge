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
}

impl ChromeIcon {
    pub const ALL: [ChromeIcon; 18] = [
        Self::Home,
        Self::Games,
        Self::Apps,
        Self::Emulators,
        Self::Plugins,
        Self::All,
        Self::Vita,
        Self::Psp,
        Self::Ps1,
        Self::Ports,
        Self::Homebrew,
        Self::Featured,
        Self::Downloads,
        Self::Recent,
        Self::Favorites,
        Self::Search,
        Self::Library,
        Self::Updates,
    ];

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

    fn asset_name(self) -> &'static str {
        match self {
            Self::Home => "house",
            Self::Games => "gamepad-2",
            Self::Apps => "app-window",
            Self::Emulators => "cpu",
            Self::Plugins => "plug-2",
            Self::All => "layout-grid",
            Self::Vita => "gamepad-2",
            Self::Psp => "gamepad-2",
            Self::Ps1 => "disc-3",
            Self::Ports => "package",
            Self::Homebrew => "flask-conical",
            Self::Featured => "star",
            Self::Downloads => "download",
            Self::Recent => "clock",
            Self::Favorites => "heart",
            Self::Search => "search",
            Self::Library => "package",
            Self::Updates => "download",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Home => include_bytes!("../../../assets/icons/lucide/house.png"),
            Self::Games => include_bytes!("../../../assets/icons/lucide/gamepad-2.png"),
            Self::Apps => include_bytes!("../../../assets/icons/lucide/app-window.png"),
            Self::Emulators => include_bytes!("../../../assets/icons/lucide/cpu.png"),
            Self::Plugins => include_bytes!("../../../assets/icons/lucide/plug-2.png"),
            Self::All => include_bytes!("../../../assets/icons/lucide/layout-grid.png"),
            Self::Vita => include_bytes!("../../../assets/icons/lucide/gamepad-2.png"),
            Self::Psp => include_bytes!("../../../assets/icons/lucide/gamepad-2.png"),
            Self::Ps1 => include_bytes!("../../../assets/icons/lucide/disc-3.png"),
            Self::Ports => include_bytes!("../../../assets/icons/lucide/package.png"),
            Self::Homebrew => include_bytes!("../../../assets/icons/lucide/flask-conical.png"),
            Self::Featured => include_bytes!("../../../assets/icons/lucide/star.png"),
            Self::Downloads => include_bytes!("../../../assets/icons/lucide/download.png"),
            Self::Recent => include_bytes!("../../../assets/icons/lucide/clock.png"),
            Self::Favorites => include_bytes!("../../../assets/icons/lucide/heart.png"),
            Self::Search => include_bytes!("../../../assets/icons/lucide/search.png"),
            Self::Library => include_bytes!("../../../assets/icons/lucide/package.png"),
            Self::Updates => include_bytes!("../../../assets/icons/lucide/download.png"),
        }
    }

    fn atlas_index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&icon| icon == self)
            .expect("ChromeIcon::ALL must list every variant")
    }
}

const CELL: usize = 48;
const ATLAS_COLS: usize = 8;

struct ChromeAtlas {
    texture: egui::TextureHandle,
    uvs: Vec<egui::Rect>,
}

static ATLAS: std::sync::OnceLock<ChromeAtlas> = std::sync::OnceLock::new();

fn atlas(ctx: &egui::Context) -> &'static ChromeAtlas {
    ATLAS.get_or_init(|| {
        let rows = ChromeIcon::ALL.len().div_ceil(ATLAS_COLS);
        let width = ATLAS_COLS * CELL;
        let height = rows * CELL;
        let mut pixels = vec![0u8; width * height * 4];
        let mut uvs = Vec::with_capacity(ChromeIcon::ALL.len());
        for (index, icon) in ChromeIcon::ALL.iter().enumerate() {
            let col = index % ATLAS_COLS;
            let row = index / ATLAS_COLS;
            let decoded = image::load_from_memory(icon.bytes())
                .unwrap_or_else(|err| {
                    panic!(
                        "lucide icon {} must decode at compile time: {err}",
                        icon.asset_name()
                    )
                })
                .to_rgba8();
            let src_w = decoded.width() as usize;
            let src_h = decoded.height() as usize;
            let copy_w = src_w.min(CELL);
            let copy_h = src_h.min(CELL);
            for y in 0..copy_h {
                for x in 0..copy_w {
                    let src = ((y * src_w + x) * 4)..((y * src_w + x) * 4 + 4);
                    let dst_x = col * CELL + x;
                    let dst_y = row * CELL + y;
                    let dst = ((dst_y * width + dst_x) * 4)..((dst_y * width + dst_x) * 4 + 4);
                    pixels[dst].copy_from_slice(&decoded.as_raw()[src]);
                }
            }
            let u0 = (col * CELL) as f32 / width as f32;
            let v0 = (row * CELL) as f32 / height as f32;
            let u1 = ((col + 1) * CELL) as f32 / width as f32;
            let v1 = ((row + 1) * CELL) as f32 / height as f32;
            uvs.push(egui::Rect::from_min_max(
                egui::pos2(u0, v0),
                egui::pos2(u1, v1),
            ));
        }
        let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
        let texture = ctx.load_texture(
            "lucide-chrome-atlas",
            color_image,
            egui::TextureOptions::LINEAR,
        );
        ChromeAtlas { texture, uvs }
    })
}

pub fn paint(ui: &egui::Ui, icon: ChromeIcon, rect: egui::Rect, tint: egui::Color32) {
    let atlas = atlas(ui.ctx());
    let uv = atlas.uvs[icon.atlas_index()];
    ui.painter().image(atlas.texture.id(), rect, uv, tint);
}

pub const SIDEBAR_ICON: f32 = 15.0;
pub const NAV_ICON: f32 = 14.0;
pub const SEARCH_ICON: f32 = 14.0;
