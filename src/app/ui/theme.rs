use crate::app::icons::IconCache;
use crate::data::Category;

pub const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(0x07, 0x11, 0x1c);
pub const BG_HEADER: egui::Color32 = egui::Color32::from_rgb(0x08, 0x14, 0x21);
pub const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0x10, 0x2a, 0x48);
pub const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(0x18, 0x3b, 0x62);
pub const ACCENT_STEAM: egui::Color32 = egui::Color32::from_rgb(0x15, 0x97, 0xff);
pub const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(0x1e, 0xb7, 0xff);
pub const GREEN_PLAY: egui::Color32 = egui::Color32::from_rgb(0x30, 0xd1, 0x58);
pub const GREEN_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x4c, 0xdd, 0x70);
pub const BLUE_PLAY: egui::Color32 = egui::Color32::from_rgb(0x00, 0x70, 0xff);
pub const BLUE_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x33, 0xa6, 0xff);
pub const SEPARATOR: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x4d, 0x70);
pub const TEXT_WHITE: egui::Color32 = egui::Color32::from_rgb(0xf4, 0xf7, 0xfb);
pub const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0xbb, 0xc7, 0xe2);
pub const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(0x6b, 0x85, 0xa6);
pub const STAR_GOLD: egui::Color32 = egui::Color32::from_rgb(0xff, 0xd6, 0x0a);
pub const STAR_GOLD_HOVER: egui::Color32 = egui::Color32::from_rgb(0xff, 0xe0, 0x66);
pub const SELECT_GLOW: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x1e, 0xb7, 0xff, 55);
pub const GLASS_ALPHA: f32 = 0.78;
pub const GLASS_EDGE: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x2c, 0x2c, 0x2e, 0x55);
pub const FONT_MICRO: f32 = 11.0;
pub const FONT_SMALL: f32 = 12.0;
pub const FONT_BODY: f32 = 14.0;
pub const FONT_LARGE: f32 = 15.0;
pub const FONT_TITLE: f32 = 18.0;
pub const FONT_HEADLINE: f32 = 24.0;

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

pub fn glass(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(GLASS_ALPHA)
}

pub const RADIUS_XS: f32 = 4.0;
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 10.0;
pub const CARD_RADIUS: f32 = 8.0;
pub const HINT_BAR_HEIGHT: f32 = 34.0;
pub const SEARCH_FIELD_WIDTH: f32 = 160.0;
pub const SEARCH_FIELD_HEIGHT: f32 = 28.0;
pub const SEARCH_CLEAR_SIZE: f32 = 18.0;
pub const GRID_COLUMNS: usize = 5;
pub const ART_LOOKAHEAD: usize = 6;
pub const GRID_COL_SPACING: f32 = 8.0;
pub const GRID_ROW_SPACING: f32 = 8.0;
pub const GRID_BOTTOM_PAD: f32 = 6.0;
pub const TILE_LABEL_HEIGHT: f32 = 22.0;
pub const TILE_LABEL_GAP: f32 = 3.0;
pub const COVER_ASPECT: f32 = 0.92;
pub const SCREEN_MARGIN: f32 = 8.0;
pub const SIDEBAR_WIDTH: f32 = 128.0;
pub const SIDEBAR_GAP: f32 = 6.0;
pub const SIDEBAR_ROW_HEIGHT: f32 = 28.0;
pub const HEADER_HEIGHT: f32 = 46.0;
pub const TAB_PILL_HEIGHT: f32 = 25.0;
pub const FOCUS_STROKE: f32 = 2.3;
pub const FEATURED_BANNER_CARD_HEIGHT: f32 = 154.0;
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
    let mut style = (*ctx.style()).clone();
    style.spacing.scroll.bar_width = 4.0;
    style.spacing.scroll.bar_inner_margin = 0.0;
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.interaction.interact_radius = 8.0;
    style.animation_time = 0.12;
    style.scroll_animation = egui::style::ScrollAnimation::duration(0.08);
    ctx.set_style(style);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = BG_DEEP;
    visuals.selection.bg_fill = ACCENT_STEAM.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.5_f32, ACCENT_STEAM);
    visuals.hyperlink_color = ACCENT_CYAN;
    let control_radius = egui::CornerRadius::same(RADIUS_MD as u8);
    visuals.widgets.noninteractive.corner_radius = control_radius;
    visuals.widgets.inactive.corner_radius = control_radius;
    visuals.widgets.hovered.corner_radius = control_radius;
    visuals.widgets.active.corner_radius = control_radius;
    visuals.widgets.open.corner_radius = control_radius;
    visuals.window_corner_radius = egui::CornerRadius::same(CARD_RADIUS as u8);
    visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS_LG as u8);
    ctx.set_visuals(visuals);
    ctx.options_mut(|opts| {
        opts.input_options.max_click_duration = 5.0;
        opts.input_options.max_click_dist = 32.0;
    });
    ctx.tessellation_options_mut(|opts| {
        opts.feathering = false;
    });
}

pub fn paint_background(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, BG_DEEP);
    let band = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.right(), rect.top() + rect.height() * 0.46),
    );
    let mut wash = egui::Mesh::default();
    wash.colored_vertex(band.left_top(), egui::Color32::from_rgb(0x0d, 0x20, 0x30));
    wash.colored_vertex(band.right_top(), egui::Color32::from_rgb(0x08, 0x16, 0x24));
    wash.colored_vertex(band.right_bottom(), BG_DEEP);
    wash.colored_vertex(band.left_bottom(), BG_DEEP);
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
    let screen = ui.ctx().screen_rect();
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
    scrim.colored_vertex(fade.right_bottom(), BG_DEEP);
    scrim.colored_vertex(fade.left_bottom(), BG_DEEP);
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
