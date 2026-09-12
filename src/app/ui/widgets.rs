use crate::app::i18n::Language;
use crate::app::ui::theme::*;
use crate::data::{Category, Platform, SortDirection, SortOrder};
use crate::install::installed::InstallState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glyph {
    Cross,
    Circle,
    Triangle,
    Square,
    Shoulders,
}

static BUTTON_TEXTURES: std::sync::OnceLock<[egui::TextureHandle; 4]> = std::sync::OnceLock::new();

pub(crate) fn button_texture(ctx: &egui::Context, glyph: Glyph) -> Option<egui::TextureHandle> {
    let textures = BUTTON_TEXTURES.get_or_init(|| {
        let decode = |name: &str, bytes: &[u8]| {
            let decoded = image::load_from_memory(bytes)
                .unwrap_or_else(|err| {
                    panic!(
                        "assets/buttons/{name} is bundled at compile time and must decode: {err}"
                    )
                })
                .to_rgba8();
            let size = [decoded.width() as usize, decoded.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };
        [
            decode(
                "ps-button-x",
                include_bytes!("../../../assets/buttons/ps-button-x.png"),
            ),
            decode(
                "ps-button-c",
                include_bytes!("../../../assets/buttons/ps-button-c.png"),
            ),
            decode(
                "ps-button-t",
                include_bytes!("../../../assets/buttons/ps-button-t.png"),
            ),
            decode(
                "ps-button-s",
                include_bytes!("../../../assets/buttons/ps-button-s.png"),
            ),
        ]
    });
    match glyph {
        Glyph::Cross => Some(textures[0].clone()),
        Glyph::Circle => Some(textures[1].clone()),
        Glyph::Triangle => Some(textures[2].clone()),
        Glyph::Square => Some(textures[3].clone()),
        Glyph::Shoulders => None,
    }
}

pub(crate) struct CardResponse {
    pub clicked: bool,
}

pub(crate) struct SearchFieldResponse {
    pub open_requested: bool,
    pub cleared: bool,
}

pub(crate) fn self_update_pill(ui: &mut egui::Ui, lang: Language, tag: &str) -> bool {
    let tag = if tag.starts_with('v') || tag.starts_with('V') {
        tag.to_owned()
    } else {
        format!("v{tag}")
    };
    let label = format!("{} {}", lang.update(), tag);
    let galley = ui.fonts(|f| f.layout_no_wrap(label, font(FONT_SMALL), TEXT_WHITE));
    let pad_x = 14.0;
    let pad_y = 8.0;
    let icon_w = 12.0;
    let size = egui::vec2(
        galley.size().x + pad_x * 2.0 + icon_w + 6.0,
        galley.size().y + pad_y * 2.0,
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover = response.hovered();
    let fill = if hover {
        TEXT_WHITE.gamma_multiply(0.12)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        egui::Stroke::new(1.5_f32, TEXT_WHITE),
        egui::StrokeKind::Inside,
    );
    let icon_c = egui::pos2(rect.left() + pad_x + icon_w * 0.5, rect.center().y);
    let tri = [
        egui::pos2(icon_c.x - 5.0, icon_c.y - 3.0),
        egui::pos2(icon_c.x + 5.0, icon_c.y - 3.0),
        egui::pos2(icon_c.x, icon_c.y + 5.0),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        tri.to_vec(),
        TEXT_WHITE,
        egui::Stroke::NONE,
    ));
    ui.painter().galley(
        egui::pos2(
            rect.left() + pad_x + icon_w + 6.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        TEXT_WHITE,
    );
    response.clicked()
}

pub(crate) fn search_chip(ui: &mut egui::Ui, label: &str) -> bool {
    search_chip_sized(ui, label, None)
}

pub(crate) fn search_chip_sized(ui: &mut egui::Ui, label: &str, fill_width: Option<f32>) -> bool {
    use crate::app::ui::chrome_icons::{self, ChromeIcon};
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), TEXT_DIM));
    let icon = chrome_icons::SEARCH_ICON;
    let natural = galley.size().x + 16.0 + icon + 10.0;
    let width = fill_width.unwrap_or(natural).max(natural);
    let size = egui::vec2(width, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if response.hovered() {
        BG_CARD_HOVER
    } else {
        BG_CARD
    };
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        egui::Stroke::new(1.0_f32, SEPARATOR),
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 10.0 + icon * 0.5, rect.center().y),
        egui::vec2(icon, icon),
    );
    chrome_icons::paint(ui, ChromeIcon::Search, icon_rect, TEXT_DIM);
    ui.painter().galley(
        egui::pos2(
            rect.left() + 10.0 + icon + 6.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        TEXT_DIM,
    );
    response.clicked()
}

pub(crate) fn search_field(
    ui: &mut egui::Ui,
    query: &str,
    placeholder: &str,
    active: bool,
) -> SearchFieldResponse {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(SEARCH_FIELD_WIDTH, SEARCH_FIELD_HEIGHT),
        egui::Sense::click(),
    );
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let border = if active { ACCENT_CYAN } else { SEPARATOR };
    ui.painter().rect_filled(
        rect,
        RADIUS_SM,
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
    );
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(if active { 2.0_f32 } else { 1.0_f32 }, border),
        egui::StrokeKind::Inside,
    );
    let has_query = !query.is_empty();
    let clear_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - 6.0 - SEARCH_CLEAR_SIZE / 2.0,
            rect.center().y,
        ),
        egui::Vec2::splat(SEARCH_CLEAR_SIZE),
    );
    let text_limit = if has_query {
        clear_rect.left() - 6.0
    } else {
        rect.right() - 10.0
    };
    let (text, color) = if has_query {
        (query, TEXT_WHITE)
    } else {
        (placeholder, TEXT_FAINT)
    };
    ui.painter()
        .with_clip_rect(egui::Rect::from_min_max(
            rect.min,
            egui::pos2(text_limit, rect.max.y),
        ))
        .text(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            font(FONT_BODY),
            color,
        );
    let mut cleared = false;
    if has_query {
        let clear = ui.interact(clear_rect, response.id.with("clear"), egui::Sense::click());
        let clear_hover = if clear.hovered() { 1.0_f32 } else { 0.0_f32 };
        ui.painter().circle_filled(
            clear_rect.center(),
            SEARCH_CLEAR_SIZE / 2.0,
            SEPARATOR.lerp_to_gamma(BG_CARD_HOVER, clear_hover),
        );
        let arm = SEARCH_CLEAR_SIZE * 0.22;
        let center = clear_rect.center();
        let stroke = egui::Stroke::new(1.5_f32, TEXT_WHITE);
        ui.painter().line_segment(
            [center - egui::vec2(arm, arm), center + egui::vec2(arm, arm)],
            stroke,
        );
        ui.painter().line_segment(
            [
                center + egui::vec2(arm, -arm),
                center + egui::vec2(-arm, arm),
            ],
            stroke,
        );
        cleared = clear.clicked();
    }
    SearchFieldResponse {
        open_requested: response.clicked() && !cleared,
        cleared,
    }
}

pub(crate) fn source_dropdown(
    ui: &mut egui::Ui,
    lang: Language,
    total_apps: usize,
    source_counts: &[(crate::data::SourceCatalog, usize)],
    source_filter: Option<crate::data::SourceCatalog>,
) -> Option<Option<crate::data::SourceCatalog>> {
    let popup_id = ui.make_persistent_id("source_dropdown");
    let label = source_filter.map_or(lang.all_catalogs(), crate::data::SourceCatalog::label);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), ACCENT_CYAN));
    let size = egui::vec2((galley.size().x + 34.0).max(72.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));
    let border = if open || source_filter.is_some() {
        ACCENT_CYAN
    } else {
        SEPARATOR
    };
    ui.painter().rect_filled(
        rect,
        rect.height() / 2.0,
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
    );
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        egui::Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ACCENT_CYAN,
    );
    chevron(
        ui.painter(),
        egui::pos2(rect.right() - 12.0, rect.center().y),
        open,
    );
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let mut picked = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(180.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            let all_label = format!("{} ({total_apps})", lang.all_catalogs());
            if dropdown_row(ui, &all_label, source_filter.is_none()) {
                picked = Some(None);
            }
            for &(source, count) in source_counts {
                let row_label = format!("{} ({count})", source.label());
                if dropdown_row(ui, &row_label, source_filter == Some(source)) {
                    picked = Some(Some(source));
                }
            }
        },
    );
    picked
}

pub(crate) fn genre_dropdown(
    ui: &mut egui::Ui,
    lang: Language,
    genre_counts: &[(String, usize)],
    genre_filter: Option<&str>,
) -> Option<Option<String>> {
    let popup_id = ui.make_persistent_id("genre_dropdown");
    let mut label =
        genre_filter.map_or_else(|| lang.all_genres_label(), |genre| lang.genre_label(genre));
    if label.chars().count() > 16 {
        label = format!("{}…", label.chars().take(15).collect::<String>());
    }
    let galley = ui.fonts(|f| f.layout_no_wrap(label, font(FONT_SMALL), ACCENT_CYAN));
    let size = egui::vec2((galley.size().x + 34.0).clamp(112.0, 156.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));
    ui.painter().rect_filled(
        rect,
        rect.height() / 2.0,
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
    );
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        egui::Stroke::new(
            1.0_f32,
            if open || genre_filter.is_some() {
                ACCENT_CYAN
            } else {
                SEPARATOR
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ACCENT_CYAN,
    );
    chevron(
        ui.painter(),
        egui::pos2(rect.right() - 12.0, rect.center().y),
        open,
    );
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let mut picked = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_width(176.0);
            egui::ScrollArea::vertical()
                .id_salt("genre_dropdown_options")
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if dropdown_row(ui, &lang.all_genres_label(), genre_filter.is_none()) {
                        picked = Some(None);
                    }
                    for (genre, count) in genre_counts {
                        let row_label = format!("{} ({count})", lang.genre_label(genre));
                        if dropdown_row(ui, &row_label, genre_filter == Some(genre.as_str())) {
                            picked = Some(Some(genre.clone()));
                        }
                    }
                });
        },
    );
    picked
}

pub(crate) fn sort_dropdown(
    ui: &mut egui::Ui,
    lang: Language,
    sort_order: SortOrder,
    sort_direction: SortDirection,
) -> Option<SortOrder> {
    let popup_id = ui.make_persistent_id("sort_dropdown");
    let label = lang.sort_label(sort_order);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), TEXT_WHITE));
    let size = egui::vec2((galley.size().x + 40.0).max(110.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));
    ui.painter().rect_filled(
        rect,
        rect.height() / 2.0,
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
    );
    ui.painter().rect_stroke(
        rect,
        rect.height() / 2.0,
        egui::Stroke::new(1.0_f32, if open { ACCENT_CYAN } else { SEPARATOR }),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        TEXT_WHITE,
    );
    sort_direction_triangle(
        ui.painter(),
        egui::pos2(rect.right() - 22.0, rect.center().y),
        sort_direction,
        TEXT_DIM,
    );
    chevron(
        ui.painter(),
        egui::pos2(rect.right() - 10.0, rect.center().y),
        open,
    );
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let mut picked = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(170.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            for sort in SortOrder::ALL {
                if dropdown_row(ui, lang.sort_label(sort), sort == sort_order) {
                    picked = Some(sort);
                }
            }
        },
    );
    picked
}

pub(crate) fn dropdown_row(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    if active {
        ui.painter()
            .rect_filled(rect, RADIUS_XS, ACCENT_STEAM.gamma_multiply(0.22));
    } else if hover_t > 0.0 {
        ui.painter()
            .rect_filled(rect, RADIUS_XS, BG_CARD_HOVER.gamma_multiply(hover_t));
    }
    let text_color = if active { ACCENT_CYAN } else { TEXT_WHITE };
    let mut job = egui::text::LayoutJob::simple(
        label.to_owned(),
        font(FONT_BODY),
        text_color,
        (rect.width() - 20.0).max(0.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );
    response.clicked()
}

pub(crate) fn pill_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let text_color = if active { BG_DEEP } else { TEXT_WHITE };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), text_color));
    let size = egui::vec2(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    if active {
        ui.painter()
            .rect_filled(rect, rect.height() / 2.0, TEXT_WHITE);
    } else {
        ui.painter().rect_filled(
            rect,
            rect.height() / 2.0,
            BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
        );
        ui.painter().rect_stroke(
            rect,
            rect.height() / 2.0,
            egui::Stroke::new(1.0_f32, SEPARATOR),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, text_color);
    response.clicked()
}

pub(crate) fn top_nav_button(
    ui: &mut egui::Ui,
    icon: crate::app::ui::chrome_icons::ChromeIcon,
    label: &str,
    active: bool,
) -> bool {
    use crate::app::ui::chrome_icons;
    let color = if active { TEXT_WHITE } else { TEXT_DIM };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_MICRO), color));
    let height = TAB_PILL_HEIGHT;
    let icon_pad = chrome_icons::NAV_ICON + 3.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2((galley.size().x + 10.0 + icon_pad).max(36.0), height),
        egui::Sense::click(),
    );
    let radius = height / 2.0;
    let active_t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(("nav", label)), active, 0.14);
    if active_t > 0.01 {
        let fill = BLUE_PLAY.gamma_multiply(0.55 + 0.45 * active_t);
        ui.painter().rect_filled(rect, radius, fill);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, BG_CARD_HOVER);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.left() + 5.0 + chrome_icons::NAV_ICON * 0.5,
            rect.center().y,
        ),
        egui::vec2(chrome_icons::NAV_ICON, chrome_icons::NAV_ICON),
    );
    chrome_icons::paint(ui, icon, icon_rect, color);
    ui.painter().galley(
        egui::pos2(
            rect.left() + 5.0 + icon_pad,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    response.clicked()
}

pub(crate) fn back_button(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(100.0, 38.0), egui::Sense::click());
    let press_t = if response.is_pointer_button_down_on() {
        1.0_f32
    } else {
        0.0_f32
    };
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));
    ui.painter().rect_filled(
        rect,
        RADIUS_SM,
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t),
    );
    ui.painter().rect_stroke(
        rect,
        RADIUS_SM,
        egui::Stroke::new(1.0_f32, SEPARATOR),
        egui::StrokeKind::Inside,
    );
    let chevron_x = rect.left() + 20.0;
    let mid_y = rect.center().y;
    let stroke = egui::Stroke::new(2.0_f32, ACCENT_CYAN);
    ui.painter().line_segment(
        [
            egui::pos2(chevron_x + 4.0, mid_y - 6.0),
            egui::pos2(chevron_x - 3.0, mid_y),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(chevron_x - 3.0, mid_y),
            egui::pos2(chevron_x + 4.0, mid_y + 6.0),
        ],
        stroke,
    );
    ui.painter().text(
        egui::pos2(chevron_x + 12.0, mid_y),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_BODY),
        TEXT_WHITE,
    );
    response.clicked()
}

pub(crate) fn sort_direction_triangle(
    painter: &egui::Painter,
    center: egui::Pos2,
    direction: SortDirection,
    color: egui::Color32,
) {
    let (top, bottom) = match direction {
        SortDirection::Asc => (-3.0, 3.0),
        SortDirection::Desc => (3.0, -3.0),
    };
    painter.add(egui::Shape::convex_polygon(
        vec![
            center + egui::vec2(-3.5, top),
            center + egui::vec2(3.5, top),
            center + egui::vec2(0.0, bottom),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

pub(crate) fn chevron(painter: &egui::Painter, center: egui::Pos2, open: bool) {
    let stroke = egui::Stroke::new(1.6_f32, TEXT_DIM);
    let (side_y, mid_y) = if open { (2.0, -2.0) } else { (-2.0, 2.0) };
    let left = center + egui::vec2(-4.0, side_y);
    let mid = center + egui::vec2(0.0, mid_y);
    let right = center + egui::vec2(4.0, side_y);
    painter.line_segment([left, mid], stroke);
    painter.line_segment([mid, right], stroke);
}

pub(crate) fn source_chip_colors(source: crate::data::SourceCatalog) -> (egui::Color32, egui::Color32) {
    match source {
        crate::data::SourceCatalog::VitaDb => (
            egui::Color32::from_rgb(0x7d, 0xd3, 0xfc),
            egui::Color32::from_rgba_unmultiplied(0x0e, 0xa5, 0xe9, 55),
        ),
        crate::data::SourceCatalog::VitaDbToo => (
            egui::Color32::from_rgb(0xd8, 0xb4, 0xfe),
            egui::Color32::from_rgba_unmultiplied(0xa8, 0x55, 0xf7, 55),
        ),
        crate::data::SourceCatalog::Nps => (
            egui::Color32::from_rgb(0x6e, 0xe7, 0xb7),
            egui::Color32::from_rgba_unmultiplied(0x10, 0xb9, 0x81, 55),
        ),
    }
}

pub(crate) fn source_chip(ui: &mut egui::Ui, source: crate::data::SourceCatalog) {
    let (fg, bg) = source_chip_colors(source);
    let galley =
        ui.fonts(|f| f.layout_no_wrap(source.short_label().to_owned(), font(FONT_MICRO), fg));
    let size = egui::vec2(galley.size().x + 10.0, 16.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_XS, bg);
    ui.painter().rect_stroke(
        rect,
        RADIUS_XS,
        egui::Stroke::new(1.0_f32, fg.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
}

pub(crate) fn install_marker(painter: &egui::Painter, rect: egui::Rect, state: InstallState) {
    let color = match state {
        InstallState::Absent => return,
        InstallState::Installed => GREEN_PLAY,
        InstallState::Outdated => STAR_GOLD,
    };
    let size = 14.0;
    let marker = egui::Rect::from_min_size(
        egui::pos2(rect.right() - size - 4.0, rect.top() + 4.0),
        egui::vec2(size, size),
    );
    let center = marker.center();
    painter.rect_filled(marker, RADIUS_XS, color);
    let stroke = egui::Stroke::new(2.0_f32, TEXT_WHITE);
    match state {
        InstallState::Installed => {
            painter.line_segment(
                [
                    center + egui::vec2(-3.5, 0.0),
                    center + egui::vec2(-1.0, 2.8),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-1.0, 2.8),
                    center + egui::vec2(3.8, -2.8),
                ],
                stroke,
            );
        }
        InstallState::Outdated => {
            painter.line_segment(
                [
                    center + egui::vec2(0.0, 3.5),
                    center + egui::vec2(0.0, -3.5),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-3.0, -0.6),
                    center + egui::vec2(0.0, -3.8),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(3.0, -0.6),
                    center + egui::vec2(0.0, -3.8),
                ],
                stroke,
            );
        }
        InstallState::Absent => {}
    }
}

pub(crate) fn category_badge(ui: &mut egui::Ui, category: Category) {
    let color = category_color(category);
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            category.label_upper().to_owned(),
            font(FONT_MICRO),
            TEXT_WHITE,
        )
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter()
        .rect_filled(rect, RADIUS_XS, color.gamma_multiply(0.3));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

pub(crate) fn genre_badge(ui: &mut egui::Ui, lang: Language, genre: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            lang.genre_label(genre).to_uppercase(),
            font(FONT_MICRO),
            TEXT_WHITE,
        )
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter()
        .rect_filled(rect, RADIUS_XS, ACCENT_CYAN.gamma_multiply(0.22));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

pub(crate) fn rating_stars(ui: &mut egui::Ui, rating: f32) {
    const FILLED: [&str; 6] = ["", "★", "★★", "★★★", "★★★★", "★★★★★"];
    const EMPTY: [&str; 6] = ["★★★★★", "★★★★", "★★★", "★★", "★", ""];
    let filled = ((rating + 0.5).floor().max(0.0) as usize).min(5);
    let font = font(FONT_SMALL);
    let gold = ui.fonts(|f| f.layout_no_wrap(FILLED[filled].to_owned(), font.clone(), STAR_GOLD));
    let faint = ui.fonts(|f| f.layout_no_wrap(EMPTY[filled].to_owned(), font, TEXT_FAINT));
    let size = egui::vec2(
        gold.size().x + faint.size().x,
        gold.size().y.max(faint.size().y),
    );
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let gold_width = gold.size().x;
    ui.painter().galley(rect.left_top(), gold, STAR_GOLD);
    ui.painter().galley(
        rect.left_top() + egui::vec2(gold_width, 0.0),
        faint,
        TEXT_FAINT,
    );
}

pub(crate) fn tappable_stars(ui: &mut egui::Ui, current: Option<u8>) -> Option<u8> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for star in 1..=5u8 {
            let filled = current.is_some_and(|c| star <= c);
            let color = if filled { STAR_GOLD } else { TEXT_FAINT };
            let response = ui.add(
                egui::Button::new(egui::RichText::new("★").size(FONT_LARGE).color(color))
                    .frame(false),
            );
            if response.clicked() {
                chosen = Some(star);
            }
        }
    });
    chosen
}

pub(crate) fn like_button(ui: &mut egui::Ui, liked: bool, likes_count: u32) -> bool {
    let color = if liked {
        egui::Color32::from_rgb(0xf4, 0x5d, 0x5d)
    } else {
        TEXT_DIM
    };
    let glyph = if liked { "♥" } else { "♡" };
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{glyph} {likes_count}"))
                .size(FONT_BODY)
                .color(color),
        )
        .frame(false),
    );
    response.clicked()
}

pub(crate) fn platform_badge(ui: &mut egui::Ui, platform: Platform) {
    let galley =
        ui.fonts(|f| f.layout_no_wrap(platform.label().to_owned(), font(FONT_MICRO), ACCENT_CYAN));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter()
        .rect_filled(rect, RADIUS_XS, ACCENT_CYAN.gamma_multiply(0.22));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ACCENT_CYAN);
}

pub(crate) fn region_badge(ui: &mut egui::Ui, region: &str) {
    let galley = ui.fonts(|f| f.layout_no_wrap(region.to_owned(), font(FONT_MICRO), TEXT_WHITE));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(
        rect,
        RADIUS_XS,
        egui::Color32::from_rgb(0xea, 0x58, 0x0c).gamma_multiply(0.8),
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

pub(crate) fn source_badge(ui: &mut egui::Ui, label: &str) {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_uppercase(), font(FONT_MICRO), TEXT_WHITE));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD_HOVER);
    ui.painter().rect_stroke(
        rect,
        RADIUS_XS,
        egui::Stroke::new(1.0_f32, ACCENT_CYAN),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

pub(crate) fn warning_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4_f32, color);
    let top = center + egui::vec2(0.0, -radius);
    let left = center + egui::vec2(-radius * 0.95, radius * 0.75);
    let right = center + egui::vec2(radius * 0.95, radius * 0.75);
    painter.line_segment([top, left], stroke);
    painter.line_segment([left, right], stroke);
    painter.line_segment([right, top], stroke);
    painter.line_segment(
        [
            center + egui::vec2(0.0, -radius * 0.28),
            center + egui::vec2(0.0, radius * 0.28),
        ],
        stroke,
    );
    painter.circle_filled(center + egui::vec2(0.0, radius * 0.55), 0.9, color);
}

pub(crate) fn warning_pill(ui: &mut egui::Ui, label: &str) {
    const ICON_BOX: f32 = 16.0;
    const PADDING: f32 = 7.0;
    let text_width = (ui.available_width() - ICON_BOX - PADDING * 3.0).max(60.0);
    let mut job =
        egui::text::LayoutJob::simple(label.to_owned(), font(FONT_MICRO), STAR_GOLD, text_width);
    job.wrap.max_rows = 4;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|f| f.layout_job(job));
    let size = egui::vec2(
        galley.size().x + ICON_BOX + PADDING * 3.0,
        (galley.size().y + PADDING).max(ICON_BOX),
    );
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter()
        .rect_filled(rect, RADIUS_XS, STAR_GOLD.gamma_multiply(0.2));
    warning_glyph(
        ui.painter(),
        egui::pos2(
            rect.left() + PADDING + ICON_BOX / 2.0,
            rect.top() + ICON_BOX / 2.0 + 1.0,
        ),
        5.0,
        STAR_GOLD,
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + PADDING * 2.0 + ICON_BOX,
            rect.top() + PADDING / 2.0,
        ),
        galley,
        STAR_GOLD,
    );
}

pub(crate) fn install_pill(ui: &mut egui::Ui, lang: Language, state: InstallState) {
    let (label, color) = match state {
        InstallState::Absent => return,
        InstallState::Installed => (lang.installed(), GREEN_PLAY),
        InstallState::Outdated => (lang.update_available(), STAR_GOLD),
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_MICRO), color));
    let size = egui::vec2(galley.size().x + 14.0, 16.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter()
        .rect_filled(rect, RADIUS_XS, color.gamma_multiply(0.25));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
}

pub(crate) fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_DIM).size(FONT_BODY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value)
                    .size(FONT_BODY)
                    .strong()
                    .color(TEXT_WHITE),
            );
        });
    });
    ui.add_space(6.0);
    ui.painter().hline(
        ui.max_rect().x_range(),
        ui.cursor().min.y,
        egui::Stroke::new(1.0_f32, SEPARATOR),
    );
    ui.add_space(6.0);
}
