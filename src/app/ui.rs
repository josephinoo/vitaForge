use super::i18n::Language;
use super::icons::IconCache;
use super::{App, AppState};
use crate::data::{Category, Platform, SortDirection, SortOrder};
use crate::input::AppCommand;
use crate::install::installed::{InstallState, InstalledIndex};
const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(0x06, 0x07, 0x0e);
const BG_HEADER: egui::Color32 = egui::Color32::from_rgb(0x0a, 0x0c, 0x18);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0x18, 0x1c, 0x33);
const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(0x23, 0x2a, 0x49);
const ACCENT_STEAM: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x9f, 0xff);
const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(0x38, 0xbd, 0xf8);
const GREEN_PLAY: egui::Color32 = egui::Color32::from_rgb(0x47, 0xa0, 0x1d);
const GREEN_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x5c, 0xba, 0x2a);
const SEPARATOR: egui::Color32 = egui::Color32::from_rgb(0x2c, 0x33, 0x52);
const TEXT_WHITE: egui::Color32 = egui::Color32::from_rgb(0xf8, 0xfa, 0xfc);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x94, 0xa3, 0xb8);
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(0x64, 0x74, 0x8b);
const STAR_GOLD: egui::Color32 = egui::Color32::from_rgb(0xfb, 0xbf, 0x24);
const GLASS_ALPHA: f32 = 0.62;
const GLASS_EDGE: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x16, 0x18, 0x20, 0x16);
const FONT_MICRO: f32 = 9.0;
const FONT_SMALL: f32 = 11.0;
const FONT_BODY: f32 = 13.0;
const FONT_LARGE: f32 = 15.0;
const FONT_TITLE: f32 = 20.0;
const FONT_HEADLINE: f32 = 28.0;
fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}
fn glass(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(GLASS_ALPHA)
}
const CARD_RADIUS: f32 = 10.0;
const HINT_BAR_HEIGHT: f32 = 30.0;
const SEARCH_FIELD_WIDTH: f32 = 190.0;
const SEARCH_FIELD_HEIGHT: f32 = 30.0;
const SEARCH_CLEAR_SIZE: f32 = 18.0;
pub const GRID_COLUMNS: usize = 5;
const GRID_COL_SPACING: f32 = 14.0;
const GRID_ROW_SPACING: f32 = 1.0;
const SCREEN_MARGIN: f32 = 25.0;
const FEATURED_BANNER_CARD_HEIGHT: f32 = 96.0;
const FEATURED_BANNER_HEIGHT: f32 = FEATURED_BANNER_CARD_HEIGHT + 10.0 + 1.0 + 10.0;
const SCROLLBAR_RESERVE: f32 = 30.0;
const PRESS_SHRINK: f32 = 2.5;
static LOGO_TEXTURE: std::sync::OnceLock<egui::TextureHandle> = std::sync::OnceLock::new();
fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    LOGO_TEXTURE
        .get_or_init(|| {
            const LOGO_BYTES: &[u8] = include_bytes!("../../assets/images/icon.png");
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
    ctx.set_style(style);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = BG_DEEP;
    visuals.selection.bg_fill = ACCENT_STEAM.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.5_f32, ACCENT_STEAM);
    visuals.hyperlink_color = ACCENT_CYAN;
    ctx.set_visuals(visuals);
    ctx.options_mut(|opts| {
        opts.input_options.max_click_duration = 5.0;
        opts.input_options.max_click_dist = 32.0;
    });
    ctx.tessellation_options_mut(|opts| {
        opts.feathering = false;
    });
}
fn paint_background(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, BG_DEEP);
}
const HERO_FRACTION: f32 = 0.46;
fn backdrop_url(entry: &crate::data::AppEntry) -> Option<&str> {
    entry
        .background_url
        .as_deref()
        .or_else(|| entry.screenshot_urls.first().map(String::as_str))
        .or(entry.cover_url.as_deref())
}
fn paint_hero(ui: &egui::Ui, icons: &IconCache, entry: &crate::data::AppEntry) -> bool {
    let screen = ui.ctx().screen_rect();
    paint_background(ui.painter(), screen);
    let Some(url) = backdrop_url(entry) else { return false };
    let Some(texture) = icons.get_hero(ui.ctx(), url) else {
        return icons.is_loading(url, super::icons::HERO_SIDE);
    };
    let band = egui::Rect::from_min_max(
        screen.left_top(),
        egui::pos2(screen.right(), screen.top() + screen.height() * HERO_FRACTION),
    );
    let size = texture.size_vec2();
    let scale = (band.width() / size.x).max(band.height() / size.y);
    let uv_size = egui::vec2(band.width() / scale / size.x, band.height() / scale / size.y);
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
pub fn build_ui(ctx: &egui::Context, app: &App) -> Vec<AppCommand> {
    match &app.state {
        AppState::Loading => loading_screen(
            ctx,
            app.lang,
            app.install.as_ref().map(|j| &j.progress),
        ),
        AppState::Catalog(catalog) => catalog_screen(
            ctx,
            &app.icons,
            &app.installed,
            app.lang,
            &catalog.apps,
            &catalog.filtered_indices,
            &catalog.search_query,
            catalog.search_requested,
            catalog.category_filter,
            catalog.source_filter,
            catalog.sort_order,
            catalog.sort_direction,
            catalog.selection_active.then_some(catalog.selected),
            catalog.scroll_to_selected,
            catalog.scroll_reset,
            catalog.is_commercial_view,
            &catalog.source_counts,
            &catalog.category_counts,
            catalog.featured_index,
        ),
        AppState::Detail { app: entry, scroll_delta, comments, comments_loaded, comment_entry_requested, .. } => {
            let progress = app
                .install
                .as_ref()
                .filter(|job| job.app_id == entry.id)
                .map(|job| &job.progress);
            detail_screen(
                ctx,
                &app.icons,
                &app.installed,
                app.lang,
                entry,
                progress,
                app.install_busy(),
                *scroll_delta,
                comments,
                *comments_loaded,
                *comment_entry_requested,
            )
        }
        AppState::Settings { selected, .. } => settings_screen(ctx, app.lang, *selected),
    }
}
fn settings_screen(ctx: &egui::Context, lang: Language, selected: usize) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::TopBottomPanel::top("settings_header")
        .frame(egui::Frame::NONE.fill(glass(BG_HEADER)).inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if back_button(ui, lang.back()) {
                    commands.push(AppCommand::CloseSettings);
                }
            });
        });
    button_hints(ctx, &[(Glyph::Circle, lang.btn_back()), (Glyph::Cross, lang.btn_open())], None);
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            paint_background(ui.painter(), ui.ctx().screen_rect());
            ui.label(egui::RichText::new(lang.settings_title()).size(FONT_TITLE).strong().color(TEXT_WHITE));
            ui.add_space(20.0);
            ui.label(egui::RichText::new(lang.language_label()).size(FONT_BODY).color(TEXT_DIM));
            ui.add_space(8.0);
            if dropdown_row(ui, "English", selected == 0) {
                commands.push(AppCommand::SetLanguage(Language::English));
            }
            if dropdown_row(ui, "Español", selected == 1) {
                commands.push(AppCommand::SetLanguage(Language::Spanish));
            }
        });
    commands
}
fn loading_screen(
    ctx: &egui::Context,
    _lang: Language,
    install_progress: Option<&crate::install::Progress>,
) -> Vec<AppCommand> {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_background(ui.painter(), rect);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
                    |ui| {
                        let total_content_height = 270.0;
                        let pad_y = ((rect.height() - total_content_height) / 2.0).max(0.0);
                        ui.add_space(pad_y);
                        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                        let logo = logo_texture(ui.ctx());
                        ui.painter().image(
                            logo.id(),
                            icon_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new("VitaForge").size(FONT_HEADLINE).strong().color(TEXT_WHITE));
                        ui.label(egui::RichText::new("by josephinoo").size(FONT_SMALL).color(TEXT_FAINT));
                        ui.label(egui::RichText::new("Catalog by DrDecki").size(FONT_MICRO).color(TEXT_FAINT));
                        ui.add_space(8.0);
                        if let Some(progress) = install_progress {
                            ui.label(
                                egui::RichText::new("Installing...")
                                    .color(STAR_GOLD)
                                    .size(FONT_LARGE)
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(progress.label()).color(TEXT_DIM).size(FONT_BODY));
                        } else {
                            ui.label(egui::RichText::new("Bringing catalog information from databases...").color(TEXT_DIM).size(FONT_BODY));
                        }
                        ui.add_space(16.0);
                        let (spinner_rect, _) =
                            ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
                        let time = ui.input(|i| i.time);
                        let angle = time * 4.0;
                        let center = spinner_rect.center();
                        let radius = spinner_rect.width() * 0.4;
                        let n_dots = 8;
                        for i in 0..n_dots {
                            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
                            let pos = center + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32) * radius;
                            let alpha = (i as f32 / n_dots as f32).powf(1.5);
                            ui.painter().circle_filled(pos, 2.2, ACCENT_STEAM.gamma_multiply(0.2 + 0.8 * alpha));
                        }
                    },
                );
            });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
    Vec::new()
}
fn catalog_screen(
    ctx: &egui::Context,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    apps: &[crate::data::AppEntry],
    filtered_indices: &[usize],
    search_query: &str,
    search_active: bool,
    category_filter: Option<Category>,
    source_filter: Option<crate::data::SourceCatalog>,
    sort_order: SortOrder,
    sort_direction: crate::data::SortDirection,
    selected: Option<usize>,
    scroll_to_selected: bool,
    scroll_reset: bool,
    is_commercial_view: bool,
    source_counts: &[(crate::data::SourceCatalog, usize)],
    category_counts: &[(Category, usize)],
    featured_index: Option<usize>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    button_hints(
        ctx,
        &[
            (Glyph::Cross, lang.btn_open()),
            (Glyph::Circle, lang.btn_back()),
            (Glyph::Triangle, lang.btn_search()),
            (Glyph::Shoulders, lang.btn_category()),
        ],
        Some(installed.summary()),
    );
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::vec2(SCREEN_MARGIN, 10.0)))
        .show(ctx, |ui| {
            paint_background(ui.painter(), ui.ctx().screen_rect());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("VitaForge").size(FONT_TITLE).strong().color(TEXT_WHITE));
                ui.label(egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).size(FONT_SMALL).color(TEXT_DIM));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(lang.apps_count(filtered_indices.len())).color(TEXT_FAINT).size(FONT_SMALL));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let field = search_field(ui, search_query, lang.search_placeholder(), search_active);
                    if field.cleared {
                        commands.push(AppCommand::SetSearchQuery(String::new()));
                    }
                    if field.open_requested {
                        commands.push(AppCommand::RequestSearch);
                    }
                });
            });
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                if pill_button(ui, lang.category_label(None), category_filter.is_none()) {
                    commands.push(AppCommand::SetCategoryFilter(None));
                }
                for &(category, _count) in category_counts {
                    if pill_button(ui, lang.category_label(Some(category)), category_filter == Some(category)) {
                        commands.push(AppCommand::SetCategoryFilter(Some(category)));
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.label(
                    egui::RichText::new(lang.sort_by_prefix()).size(FONT_SMALL).color(TEXT_FAINT),
                );
                ui.add_space(6.0);
                for sort in SortOrder::ALL {
                    let active = sort == sort_order;
                    let direction = active.then_some(sort_direction);
                    if sort_pill_button(ui, lang.sort_label(sort), active, direction) {
                        commands.push(AppCommand::SetSortOrder(sort));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(picked) = source_dropdown(ui, apps.len(), source_counts, source_filter) {
                        commands.push(AppCommand::SetSourceFilter(picked));
                    }
                });
            });
            ui.add_space(12.0);
            if filtered_indices.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new(lang.no_results()).size(FONT_LARGE).color(TEXT_DIM));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(lang.no_results_sub()).size(FONT_BODY).color(TEXT_FAINT));
                });
                return;
            }

            let featured_entry = if search_query.trim().is_empty() && !is_commercial_view {
                featured_index.and_then(|idx| apps.get(idx))
            } else {
                None
            };
            let banner_height = if featured_entry.is_some() { FEATURED_BANNER_HEIGHT } else { 0.0 };
            let available = ui.available_width() - SCROLLBAR_RESERVE;
            let aspect_ratio = if is_commercial_view { 1.5 } else { 1.0 };
            let card_width = (available - GRID_COL_SPACING * (GRID_COLUMNS as f32 - 1.0)) / GRID_COLUMNS as f32;
            let card_height = card_width * aspect_ratio;
            let row_height = card_height + GRID_ROW_SPACING;
            let total_rows = filtered_indices.len().div_ceil(GRID_COLUMNS);
            let mut scroll_area = egui::ScrollArea::vertical().id_salt("catalog_grid");
            if scroll_reset {
                scroll_area = scroll_area.vertical_scroll_offset(0.0);
            }
            scroll_area.show_viewport(ui, |ui, viewport| {
                ui.set_height(banner_height + row_height * total_rows as f32);
   
                if scroll_to_selected && !scroll_reset && let Some(cursor) = selected {
                    let row = cursor / GRID_COLUMNS;
                    let row_top = ui.max_rect().top() + banner_height + row as f32 * row_height;

                    let target_top = if row == 0 { ui.max_rect().top() } else { row_top };
                    let row_rect = egui::Rect::from_x_y_ranges(
                        ui.max_rect().x_range(),
                        target_top..=(row_top + row_height),
                    );
                    ui.scroll_to_rect(row_rect, None);
                }
                if let Some(entry) = featured_entry
                    && viewport.min.y < banner_height
                {
                    let banner_rect = egui::Rect::from_x_y_ranges(
                        ui.max_rect().x_range(),
                        ui.max_rect().top()..=(ui.max_rect().top() + banner_height),
                    );
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(banner_rect), |ui| {
                        let banner = featured_banner(ui, icons, lang, entry);
                        if banner.clicked
                            && let Some(pos) = featured_index
                                .and_then(|real_idx| filtered_indices.iter().position(|&i| i == real_idx))
                        {
                            commands.push(AppCommand::SelectApp { index: pos });
                        }
                        ui.add_space(10.0);
                        let line_rect =
                            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover()).0;
                        ui.painter().hline(
                            line_rect.x_range(),
                            line_rect.center().y,
                            egui::Stroke::new(1.0_f32, SEPARATOR),
                        );
                    });
                }
                let grid_min = (viewport.min.y - banner_height).max(0.0);
                let grid_max = (viewport.max.y - banner_height).max(0.0);
                let mut min_row = (grid_min / row_height).floor() as usize;
                let mut max_row = (grid_max / row_height).ceil() as usize + 1;
                if max_row > total_rows {
                    let diff = max_row.saturating_sub(min_row);
                    max_row = total_rows;
                    min_row = total_rows.saturating_sub(diff);
                }
                let y_min = ui.max_rect().top() + banner_height + min_row as f32 * row_height;
                let y_max = ui.max_rect().top() + banner_height + max_row as f32 * row_height;
                let grid_rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(grid_rect), |ui| {
                    ui.skip_ahead_auto_ids(min_row);
                    let row_range = min_row..max_row;
                    let prefetch_start = row_range.end;
                    let prefetch_end = (prefetch_start + 1).min(total_rows);
                    for prefetch_row in prefetch_start..prefetch_end {
                        for column in 0..GRID_COLUMNS {
                            let item_index = prefetch_row * GRID_COLUMNS + column;
                            if let Some(&real_index) = filtered_indices.get(item_index) {
                                if let Some(entry) = apps.get(real_index) {
                                    if let Some(url) = tile_art_url(entry) {
                                        let _ = icons.get(ui.ctx(), url);
                                    }
                                }
                            }
                        }
                    }
                    let mut art_wakeup: Option<std::time::Duration> = None;
                    for grid_row in row_range {
                        ui.horizontal(|ui| {
                            for column in 0..GRID_COLUMNS {
                                let item_index = grid_row * GRID_COLUMNS + column;
                                let Some(&real_index) = filtered_indices.get(item_index) else { break };
                                let Some(entry) = apps.get(real_index) else { continue };
                                if let Some(delay) = tile_art_url(entry)
                                    .and_then(|url| icons.repaint_delay(url, super::icons::MAX_ICON_SIDE))
                                {
                                    art_wakeup =
                                        Some(art_wakeup.map_or(delay, |soonest| soonest.min(delay)));
                                }
                                let card = ui
                                    .push_id((entry.platform.label(), entry.id.as_str()), |ui| {
                                        cover_card(ui, icons, installed, entry, card_width, card_height, selected == Some(item_index))
                                    })
                                    .inner;
                                if card.clicked {
                                    commands.push(AppCommand::SelectApp { index: item_index });
                                }
                                if column + 1 < GRID_COLUMNS {
                                    ui.add_space(GRID_COL_SPACING);
                                }
                            }
                        });
                        ui.add_space(GRID_ROW_SPACING);
                    }
                    if let Some(delay) = art_wakeup {
                        ui.ctx().request_repaint_after(delay);
                    }
                });
            });
        });
    commands
}
pub struct CardResponse {
    pub clicked: bool,
}
fn source_dropdown(
    ui: &mut egui::Ui,
    total_apps: usize,
    source_counts: &[(crate::data::SourceCatalog, usize)],
    source_filter: Option<crate::data::SourceCatalog>,
) -> Option<Option<crate::data::SourceCatalog>> {
    let popup_id = ui.make_persistent_id("source_dropdown");
    let label = source_filter.map_or("All catalogs", crate::data::SourceCatalog::label);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), ACCENT_CYAN));
    let size = egui::vec2((galley.size().x + 34.0).max(96.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));
    let border = if open || source_filter.is_some() { ACCENT_CYAN } else { SEPARATOR };
    ui.painter().rect_filled(rect, rect.height() / 2.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().rect_stroke(rect, rect.height() / 2.0, egui::Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ACCENT_CYAN,
    );
    chevron(ui.painter(), egui::pos2(rect.right() - 12.0, rect.center().y), open);
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let mut picked = None;
    egui::popup_below_widget(ui, popup_id, &response, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(180.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        let all_label = format!("All catalogs ({total_apps})");
        if dropdown_row(ui, &all_label, source_filter.is_none()) {
            picked = Some(None);
        }
        for &(source, count) in source_counts {
            let row_label = format!("{} ({count})", source.label());
            if dropdown_row(ui, &row_label, source_filter == Some(source)) {
                picked = Some(Some(source));
            }
        }
    });
    picked
}
fn dropdown_row(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    if active {
        ui.painter().rect_filled(rect, 5.0, ACCENT_STEAM.gamma_multiply(0.22));
    } else if hover_t > 0.0 {
        ui.painter().rect_filled(rect, 5.0, BG_CARD_HOVER.gamma_multiply(hover_t));
    }
    let text_color = if active { ACCENT_CYAN } else { TEXT_WHITE };
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_BODY),
        text_color,
    );
    response.clicked()
}
fn pill_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let text_color = if active { BG_DEEP } else { TEXT_WHITE };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), text_color));
    let size = egui::vec2(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    if active {
        ui.painter().rect_filled(rect, rect.height() / 2.0, TEXT_WHITE);
    } else {
        ui.painter().rect_filled(rect, rect.height() / 2.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
        ui.painter().rect_stroke(rect, rect.height() / 2.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    response.clicked()
}
fn sort_direction_triangle(painter: &egui::Painter, center: egui::Pos2, direction: SortDirection, color: egui::Color32) {
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
fn sort_pill_button(ui: &mut egui::Ui, label: &str, active: bool, direction: Option<SortDirection>) -> bool {
    let text_color = if active { BG_DEEP } else { TEXT_WHITE };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), text_color));
    let triangle_space = if direction.is_some() { 14.0 } else { 0.0 };
    let size = egui::vec2(galley.size().x + 22.0 + triangle_space, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    if active {
        ui.painter().rect_filled(rect, rect.height() / 2.0, TEXT_WHITE);
    } else {
        ui.painter().rect_filled(rect, rect.height() / 2.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
        ui.painter().rect_stroke(rect, rect.height() / 2.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    }
    let text_pos = rect.center() - egui::vec2(galley.size().x / 2.0 + triangle_space / 2.0, galley.size().y / 2.0);
    ui.painter().galley(text_pos, galley.clone(), text_color);
    if let Some(dir) = direction {
        let tri_center = egui::pos2(rect.right() - 12.0, rect.center().y);
        sort_direction_triangle(ui.painter(), tri_center, dir, text_color);
    }
    response.clicked()
}
fn chevron(painter: &egui::Painter, center: egui::Pos2, open: bool) {
    let stroke = egui::Stroke::new(1.6_f32, TEXT_DIM);
    let (side_y, mid_y) = if open { (2.0, -2.0) } else { (-2.0, 2.0) };
    let left = center + egui::vec2(-4.0, side_y);
    let mid = center + egui::vec2(0.0, mid_y);
    let right = center + egui::vec2(4.0, side_y);
    painter.line_segment([left, mid], stroke);
    painter.line_segment([mid, right], stroke);
}
fn cover_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    entry: &crate::data::AppEntry,
    card_width: f32,
    card_height: f32,
    focused: bool,
) -> CardResponse {
    let (full_rect, response) = ui.allocate_exact_size(egui::vec2(card_width, card_height), egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let press_t = if response.is_pointer_button_down_on() { 1.0_f32 } else { 0.0_f32 };
    let focus_t = if focused { 1.0_f32 } else { 0.0_f32 };
    let zoom = hover_t.max(focus_t);
    let inset = 6.0 - zoom * 4.5 + press_t * PRESS_SHRINK;
    let rect = full_rect.shrink(inset);
    draw_cover(ui, icons, rect, entry);
    tile_label(ui, rect, &entry.name);
    if entry.platform != Platform::Vita {
        tile_platform_badge(ui, rect, entry.platform);
    }
    install_marker(ui.painter(), rect, installed.state(entry));
    if focused {
        ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(2.5_f32, TEXT_WHITE), egui::StrokeKind::Inside);
    } else {
        ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    }
    CardResponse { clicked: response.clicked() }
}
fn featured_banner(ui: &mut egui::Ui, icons: &IconCache, lang: Language, entry: &crate::data::AppEntry) -> CardResponse {
    let width = ui.available_width();
    let height = FEATURED_BANNER_CARD_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    ui.painter().rect_filled(rect, CARD_RADIUS, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().rect_stroke(
        rect,
        CARD_RADIUS,
        egui::Stroke::new(1.0_f32, ACCENT_CYAN.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    let padding = 10.0;
    let cover_height = height - padding * 2.0;
    let cover_width = if entry.platform.is_commercial() { cover_height * (2.0 / 3.0) } else { cover_height };
    let cover_rect =
        egui::Rect::from_min_size(rect.left_top() + egui::vec2(padding, padding), egui::vec2(cover_width, cover_height));
    draw_cover(ui, icons, cover_rect, entry);
    let button_width = 150.0;
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(cover_rect.right() + 18.0, rect.top() + padding),
        egui::pos2(rect.right() - padding - button_width - 14.0, rect.bottom() - padding),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let label = lang.featured_label();
                let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_MICRO), ACCENT_CYAN));
                let size = egui::vec2(galley.size().x + 10.0, 16.0);
                let (badge_rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(badge_rect, 4.0, ACCENT_CYAN.gamma_multiply(0.25));
                ui.painter().galley(badge_rect.center() - galley.size() / 2.0, galley, ACCENT_CYAN);
                ui.add_space(6.0);
                category_badge(ui, entry.category);
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&entry.name).size(FONT_LARGE).strong().color(TEXT_WHITE));
            ui.label(egui::RichText::new(format!("by {}", entry.author)).size(FONT_SMALL).color(TEXT_DIM));
            ui.add_space(6.0);
            rating_stars(ui, entry.rating);
        });
    });
    let button_size = egui::vec2(button_width, 34.0);
    let button_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - padding - button_width, rect.center().y - button_size.y / 2.0),
        button_size,
    );
    ui.painter().rect_filled(button_rect, button_size.y / 2.0, GREEN_PLAY.lerp_to_gamma(GREEN_PLAY_HOVER, hover_t));
    ui.painter().text(
        button_rect.center(),
        egui::Align2::CENTER_CENTER,
        lang.view_details_label(),
        font(FONT_SMALL),
        TEXT_WHITE,
    );
    CardResponse { clicked: response.clicked() }
}
fn tile_art_url(entry: &crate::data::AppEntry) -> Option<&str> {
    if entry.platform.is_commercial() {
        entry.cover_url.as_deref().or(entry.icon_url.as_deref())
    } else {
        entry.icon_url.as_deref().or(entry.cover_url.as_deref())
    }
}
fn cover_fit_uv(texture_size: egui::Vec2, rect: egui::Rect) -> egui::Rect {
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }
    let texture_ratio = texture_size.x / texture_size.y;
    let rect_ratio = rect.width() / rect.height();
    if texture_ratio > rect_ratio {
        let visible_fraction = rect_ratio / texture_ratio;
        let margin = (1.0 - visible_fraction) / 2.0;
        egui::Rect::from_min_max(egui::pos2(margin, 0.0), egui::pos2(1.0 - margin, 1.0))
    } else {
        let visible_fraction = texture_ratio / rect_ratio;
        let margin = (1.0 - visible_fraction) / 2.0;
        egui::Rect::from_min_max(egui::pos2(0.0, margin), egui::pos2(1.0, 1.0 - margin))
    }
}
fn contain_fit_rect(texture_size: egui::Vec2, outer_rect: egui::Rect) -> egui::Rect {
    if texture_size.x <= 0.0 || texture_size.y <= 0.0 {
        return outer_rect;
    }
    let texture_ratio = texture_size.x / texture_size.y;
    let outer_ratio = outer_rect.width() / outer_rect.height();
    let size = if texture_ratio > outer_ratio {
        egui::vec2(outer_rect.width(), outer_rect.width() / texture_ratio)
    } else {
        egui::vec2(outer_rect.height() * texture_ratio, outer_rect.height())
    };
    egui::Rect::from_center_size(outer_rect.center(), size)
}
fn draw_card_plate(ui: &mut egui::Ui, rect: egui::Rect, color: egui::Color32) {
    ui.painter().rect_filled(rect, CARD_RADIUS, BG_DEEP);
    ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(1.0_f32, color.gamma_multiply(0.5)), egui::StrokeKind::Inside);
}
fn draw_cover(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let color = category_color(entry.category);
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            let texture_size = texture.size_vec2();
            if !entry.platform.is_commercial() {
                draw_card_plate(ui, rect, color);
                let inset = rect.width().min(rect.height()) * 0.08;
                let inner = contain_fit_rect(texture_size, rect.shrink(inset));
                ui.painter().add(
                    egui::epaint::RectShape::filled(inner, CARD_RADIUS * 0.6, TEXT_WHITE).with_texture(
                        texture.id(),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    ),
                );
                return;
            }
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, CARD_RADIUS, TEXT_WHITE)
                    .with_texture(texture.id(), cover_fit_uv(texture_size, rect)),
            );
            return;
        }
    }
    draw_card_plate(ui, rect, color);
    if art.is_some_and(|url| icons.is_loading(url, super::icons::MAX_ICON_SIDE)) {
        let time = ui.input(|i| i.time);
        let angle = time * 4.0;
        let center = rect.center();
        let radius = rect.width().min(rect.height()) * 0.18;
        let n_dots = 8;
        for i in 0..n_dots {
            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
            let pos = center + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32) * radius;
            let alpha = (i as f32 / n_dots as f32).powf(1.5);
            ui.painter().circle_filled(pos, 2.0, color.gamma_multiply(0.2 + 0.8 * alpha));
        }
        return;
    }
    let letter = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font((rect.width().min(rect.height()) * 0.42).round()),
        TEXT_WHITE,
    );
}
fn tile_label(ui: &mut egui::Ui, rect: egui::Rect, name: &str) {
    let scrim_height = 42.0;
    let scrim = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - scrim_height),
        rect.right_bottom(),
    );
    let transparent = egui::Color32::from_black_alpha(0);
    let dark = egui::Color32::from_black_alpha(200);
    let gradient_bottom = scrim.bottom() - CARD_RADIUS;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(scrim.left_top(), transparent);
    mesh.colored_vertex(scrim.right_top(), transparent);
    mesh.colored_vertex(egui::pos2(scrim.right(), gradient_bottom), dark);
    mesh.colored_vertex(egui::pos2(scrim.left(), gradient_bottom), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    let cap = egui::Rect::from_min_max(
        egui::pos2(scrim.left(), gradient_bottom),
        scrim.right_bottom(),
    );
    ui.painter().add(egui::epaint::RectShape::filled(
        cap,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: CARD_RADIUS as u8,
            se: CARD_RADIUS as u8,
        },
        dark,
    ));
    let side_margin = 9.0;
    let available_width = (rect.width() - side_margin * 2.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple(
        name.to_owned(),
        font(FONT_SMALL),
        TEXT_WHITE,
        available_width,
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|f| f.layout_job(job));
    let pos = egui::pos2(rect.left() + side_margin, rect.bottom() - 10.0 - galley.size().y);
    ui.painter().galley(pos, galley, TEXT_WHITE);
}
fn tile_platform_badge(ui: &mut egui::Ui, rect: egui::Rect, platform: Platform) {
    let text = platform.label_short();
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font(FONT_MICRO), TEXT_WHITE));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let badge = egui::Rect::from_min_size(rect.left_top() + egui::vec2(7.0, 7.0), size);
    let bg_color = if platform.is_nps() {
        egui::Color32::from_rgb(0xf5, 0x9e, 0x0b).gamma_multiply(0.85) 
    } else {
        egui::Color32::from_black_alpha(170)
    };
    ui.painter().rect_filled(badge, 4.0, bg_color);
    ui.painter().galley(badge.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
fn install_marker(painter: &egui::Painter, rect: egui::Rect, state: InstallState) {
    let color = match state {
        InstallState::Absent => return,
        InstallState::Installed => GREEN_PLAY,
        InstallState::Outdated => STAR_GOLD,
    };
    let radius = 8.0;
    let center = egui::pos2(rect.right() - radius - 4.0, rect.top() + radius + 4.0);
    painter.circle_filled(center, radius, color);
    let stroke = egui::Stroke::new(2.0_f32, TEXT_WHITE);
    match state {
        InstallState::Installed => {
            painter.line_segment(
                [center + egui::vec2(-3.5, 0.0), center + egui::vec2(-1.0, 2.8)],
                stroke,
            );
            painter.line_segment(
                [center + egui::vec2(-1.0, 2.8), center + egui::vec2(3.8, -2.8)],
                stroke,
            );
        }
        InstallState::Outdated => {
            painter.line_segment([center + egui::vec2(0.0, 3.5), center + egui::vec2(0.0, -3.5)], stroke);
            painter.line_segment([center + egui::vec2(-3.0, -0.6), center + egui::vec2(0.0, -3.8)], stroke);
            painter.line_segment([center + egui::vec2(3.0, -0.6), center + egui::vec2(0.0, -3.8)], stroke);
        }
        InstallState::Absent => {}
    }
}
fn category_badge(ui: &mut egui::Ui, category: Category) {
    let color = category_color(category);
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(category.label_upper().to_owned(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, color.gamma_multiply(0.3));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
fn cross_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let arm = radius * 0.62;
    let stroke = egui::Stroke::new(2.0_f32, color);
    painter.line_segment([center + egui::vec2(-arm, -arm), center + egui::vec2(arm, arm)], stroke);
    painter.line_segment([center + egui::vec2(arm, -arm), center + egui::vec2(-arm, arm)], stroke);
}
fn circle_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    painter.circle_stroke(center, radius * 0.66, egui::Stroke::new(2.0_f32, color));
}
fn triangle_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let arm = radius * 0.65;
    let p1 = center + egui::vec2(0.0, -arm);
    let p2 = center + egui::vec2(-arm * 0.9, arm * 0.8);
    let p3 = center + egui::vec2(arm * 0.9, arm * 0.8);
    let stroke = egui::Stroke::new(2.0_f32, color);
    painter.line_segment([p1, p2], stroke);
    painter.line_segment([p2, p3], stroke);
    painter.line_segment([p3, p1], stroke);
}
enum Glyph {
    Cross,
    Circle,
    Triangle,
    Shoulders,
}
fn button_hints(ctx: &egui::Context, hints: &[(Glyph, &str)], note: Option<String>) {
    egui::TopBottomPanel::bottom("hints")
        .exact_height(HINT_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(glass(BG_HEADER)).inner_margin(egui::vec2(SCREEN_MARGIN, 0.0)))
        .show(ctx, |ui| {
            if let Some(note) = note {
                let rect = ui.max_rect();
                ui.painter().text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    note,
                    font(FONT_MICRO),
                    TEXT_DIM,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (glyph, label) in hints {
                    ui.label(egui::RichText::new(*label).color(TEXT_DIM).size(FONT_SMALL));
                    ui.add_space(4.0);
                    match glyph {
                        Glyph::Shoulders => {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 18.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 4.0, BG_CARD);
                            ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "L1 R1",
                                font(FONT_MICRO),
                                TEXT_DIM,
                            );
                        }
                        _ => {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                            let (glyph_color, radius) = match glyph {
                                Glyph::Cross => (egui::Color32::from_rgb(0x38, 0xbd, 0xf8), 8.0),
                                Glyph::Circle => (egui::Color32::from_rgb(0xf8, 0x71, 0x71), 8.0),
                                Glyph::Triangle => (egui::Color32::from_rgb(0x34, 0xd3, 0x99), 8.0),
                                Glyph::Shoulders => unreachable!(),
                            };
                            match glyph {
                                Glyph::Cross => cross_glyph(ui.painter(), rect.center(), radius, glyph_color),
                                Glyph::Circle => circle_glyph(ui.painter(), rect.center(), radius, glyph_color),
                                Glyph::Triangle => triangle_glyph(ui.painter(), rect.center(), radius, glyph_color),
                                Glyph::Shoulders => unreachable!(),
                            }
                        }
                    }
                    ui.add_space(14.0);
                }
            });
        });
}
fn rating_stars(ui: &mut egui::Ui, rating: f32) {
    const FILLED: [&str; 6] = ["", "★", "★★", "★★★", "★★★★", "★★★★★"];
    const EMPTY: [&str; 6] = ["★★★★★", "★★★★", "★★★", "★★", "★", ""];
    let filled = ((rating + 0.5).floor().max(0.0) as usize).min(5);
    let font = font(FONT_SMALL);
    let gold = ui.fonts(|f| f.layout_no_wrap(FILLED[filled].to_owned(), font.clone(), STAR_GOLD));
    let faint = ui.fonts(|f| f.layout_no_wrap(EMPTY[filled].to_owned(), font, TEXT_FAINT));
    let size = egui::vec2(gold.size().x + faint.size().x, gold.size().y.max(faint.size().y));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let gold_width = gold.size().x;
    ui.painter().galley(rect.left_top(), gold, STAR_GOLD);
    ui.painter().galley(rect.left_top() + egui::vec2(gold_width, 0.0), faint, TEXT_FAINT);
}
fn tappable_stars(ui: &mut egui::Ui, current: Option<u8>) -> Option<u8> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for star in 1..=5u8 {
            let filled = current.is_some_and(|c| star <= c);
            let color = if filled { STAR_GOLD } else { TEXT_FAINT };
            let response = ui.add(
                egui::Button::new(egui::RichText::new("★").size(FONT_LARGE).color(color)).frame(false),
            );
            if response.clicked() {
                chosen = Some(star);
            }
        }
    });
    chosen
}
fn like_button(ui: &mut egui::Ui, liked: bool, likes_count: u32) -> bool {
    let color = if liked { egui::Color32::from_rgb(0xf4, 0x5d, 0x5d) } else { TEXT_DIM };
    let glyph = if liked { "♥" } else { "♡" };
    let response = ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{glyph} {likes_count}")).size(FONT_BODY).color(color),
        )
        .frame(false),
    );
    response.clicked()
}
fn platform_badge(ui: &mut egui::Ui, platform: Platform) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(platform.label().to_owned(), font(FONT_MICRO), ACCENT_CYAN)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, ACCENT_CYAN.gamma_multiply(0.22));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ACCENT_CYAN);
}
fn region_badge(ui: &mut egui::Ui, region: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(region.to_owned(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(0xea, 0x58, 0x0c).gamma_multiply(0.8)); 
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
fn source_badge(ui: &mut egui::Ui, label: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(label.to_uppercase(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(0x6d, 0x28, 0xd9).gamma_multiply(0.35)); 
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0xa7, 0x8b, 0xfa).gamma_multiply(0.8)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
fn warning_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4_f32, color);
    let top = center + egui::vec2(0.0, -radius);
    let left = center + egui::vec2(-radius * 0.95, radius * 0.75);
    let right = center + egui::vec2(radius * 0.95, radius * 0.75);
    painter.line_segment([top, left], stroke);
    painter.line_segment([left, right], stroke);
    painter.line_segment([right, top], stroke);
    painter.line_segment(
        [center + egui::vec2(0.0, -radius * 0.28), center + egui::vec2(0.0, radius * 0.28)],
        stroke,
    );
    painter.circle_filled(center + egui::vec2(0.0, radius * 0.55), 0.9, color);
}
fn warning_pill(ui: &mut egui::Ui, label: &str) {
    const ICON_BOX: f32 = 16.0;
    const PADDING: f32 = 7.0;
    let text_width = (ui.available_width() - ICON_BOX - PADDING * 3.0).max(60.0);
    let mut job = egui::text::LayoutJob::simple(
        label.to_owned(),
        font(FONT_MICRO),
        STAR_GOLD,
        text_width,
    );
    job.wrap.max_rows = 4;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|f| f.layout_job(job));
    let size = egui::vec2(
        galley.size().x + ICON_BOX + PADDING * 3.0,
        (galley.size().y + PADDING).max(ICON_BOX),
    );
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, STAR_GOLD.gamma_multiply(0.2));
    warning_glyph(
        ui.painter(),
        egui::pos2(rect.left() + PADDING + ICON_BOX / 2.0, rect.top() + ICON_BOX / 2.0 + 1.0),
        5.0,
        STAR_GOLD,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + PADDING * 2.0 + ICON_BOX, rect.top() + PADDING / 2.0),
        galley,
        STAR_GOLD,
    );
}
fn install_pill(ui: &mut egui::Ui, lang: Language, state: InstallState) {
    let (label, color) = match state {
        InstallState::Absent => return,
        InstallState::Installed => (lang.installed(), GREEN_PLAY),
        InstallState::Outdated => (lang.update_available(), STAR_GOLD),
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_MICRO), color));
    let size = egui::vec2(galley.size().x + 14.0, 16.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, color.gamma_multiply(0.25));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
}
fn detail_screen(
    ctx: &egui::Context,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    entry: &crate::data::AppEntry,
    install: Option<&crate::install::Progress>,
    busy: bool,
    scroll_delta: f32,
    comments: &[crate::data::api::Comment],
    comments_loaded: bool,
    comment_entry_requested: bool,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let state = installed.state(entry);
    egui::TopBottomPanel::top("detail_header")
        .frame(egui::Frame::NONE.fill(glass(BG_HEADER)).inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if busy {
                    ui.label(
                        egui::RichText::new(lang.install_in_progress()).size(FONT_BODY).color(STAR_GOLD),
                    );
                } else if back_button(ui, lang.back()) {
                    commands.push(AppCommand::BackToCatalog);
                }
            });
        });
    if busy {
        button_hints(ctx, &[], None);
    } else {
        button_hints(
            ctx,
            &[(Glyph::Circle, lang.btn_back()), (Glyph::Cross, lang.btn_open())],
            Some(installed.summary()),
        );
    }

    if let Some(job) = install {
        if !job.is_finished() {
            egui::Area::new(egui::Id::new("install_overlay"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::window(&ctx.style())
                        .fill(BG_CARD)
                        .stroke(egui::Stroke::new(1.5_f32, ACCENT_STEAM))
                        .corner_radius(12.0)
                        .inner_margin(egui::vec2(28.0, 20.0))
                        .show(ui, |ui| {
                            ui.set_width(300.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(entry.name.as_str())
                                        .size(16.0)
                                        .strong()
                                        .color(TEXT_WHITE),
                                );
                                ui.add_space(16.0);
                                install_stepper(ui, job);
                                ui.add_space(16.0);
                                ui.add(egui::Spinner::new().size(20.0).color(ACCENT_STEAM));
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(job.label())
                                        .size(13.0)
                                        .color(TEXT_DIM),
                                );
                            });
                        });
                });
        }
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            if paint_hero(ui, icons, entry) {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            egui::ScrollArea::vertical()
                .id_salt(&entry.id)
                .show(ui, |ui| {
                egui::Frame::NONE
                    .fill(glass(BG_CARD))
                    .corner_radius(CARD_RADIUS)
                    .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                    .inner_margin(16.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon_rect = ui.allocate_exact_size(egui::vec2(84.0, 84.0), egui::Sense::hover()).0;
                            draw_icon(ui, icons, icon_rect, entry);
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(&entry.name).size(FONT_TITLE).strong().color(TEXT_WHITE));
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    let author_label = lang.by_author(&entry.author);
                                    let author_btn = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(author_label)
                                                .color(ACCENT_CYAN)
                                                .size(FONT_BODY),
                                        )
                                        .frame(false),
                                    );
                                    if author_btn.clicked() {
                                        commands.push(AppCommand::SetSearchQuery(entry.author.clone()));
                                        commands.push(AppCommand::BackToCatalog);
                                    }
                                    ui.add_space(6.0);
                                    category_badge(ui, entry.category);
                                    if entry.platform != Platform::Vita {
                                        ui.add_space(4.0);
                                        platform_badge(ui, entry.platform);
                                    }
                                    if let Some(region) = &entry.region {
                                        ui.add_space(4.0);
                                        region_badge(ui, region);
                                    }
                                    for label in &entry.source_labels {
                                        ui.add_space(4.0);
                                        source_badge(ui, label);
                                    }
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    rating_stars(ui, entry.rating);
                                    ui.add_space(8.0);
                                    if like_button(ui, entry.user_liked, entry.likes_count) {
                                        commands.push(AppCommand::ToggleLike);
                                    }
                                });
                                if state != InstallState::Absent {
                                    ui.add_space(5.0);
                                    install_pill(ui, lang, state);
                                }
                                if entry.platform.is_nps() {
                                    ui.add_space(5.0);
                                    warning_pill(ui, lang.needs_nonpdrm());
                                }
                                if entry.data_url.is_some() || !entry.requirements.is_empty() {
                                    ui.add_space(5.0);
                                    let text = if entry.requirements.is_empty() {
                                        lang.needs_game_data()
                                    } else {
                                        entry.requirements.as_str()
                                    };
                                    warning_pill(ui, text);
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                match install {
                                    None => {
                                        let label = match (entry.platform, state) {
                                            (Platform::Plugin, _) => lang.download(),
                                            (_, InstallState::Absent) => lang.install(),
                                            (_, InstallState::Installed) => lang.reinstall(),
                                            (_, InstallState::Outdated) => lang.update(),
                                        };
                                        if play_install_button(ui, label, state) {
                                            commands.push(AppCommand::InstallCurrent);
                                        }
                                    }
                                    Some(progress) => {
                                        if install_status(ui, progress) {
                                            commands.push(AppCommand::DismissInstall);
                                        }
                                    }
                                }
                            });
                        });
                    });
                ui.add_space(20.0);
                screenshots_row(ui, icons, entry);
                section_label(ui, lang.description());
                ui.add_space(8.0);
                let body = if entry.long_description.trim().is_empty() {
                    &entry.description
                } else {
                    &entry.long_description
                };
                text_panel(ui, body, TEXT_WHITE);
                ui.add_space(22.0);
                if entry.platform == Platform::Plugin {
                    text_panel(ui, lang.plugin_manual_note(), STAR_GOLD);
                    ui.add_space(22.0);
                }
                if !entry.requirements.trim().is_empty() {
                    section_label(ui, lang.requirements());
                    ui.add_space(8.0);
                    text_panel(ui, entry.requirements.trim(), STAR_GOLD);
                    ui.add_space(22.0);
                }
                if !entry.changelog.trim().is_empty() {
                    section_label(ui, lang.changelog());
                    ui.add_space(8.0);
                    text_panel(ui, entry.changelog.trim(), TEXT_DIM);
                    ui.add_space(22.0);
                }
                if !entry.overview.is_empty() {
                    section_label(ui, lang.overview());
                    ui.add_space(8.0);
                    for (key, value) in &entry.overview {
                        info_row(ui, key, value);
                    }
                    ui.add_space(22.0);
                }
                section_label(ui, lang.technical_info());
                ui.add_space(8.0);
                info_row(
                    ui,
                    lang.installed_version(),
                    match state {
                        InstallState::Absent => "-",
                        InstallState::Installed => lang.installed(),
                        InstallState::Outdated => lang.update_available(),
                    },
                );
                info_row(ui, lang.version(), &entry.version);
                info_row(ui, lang.size(), &entry.size_label());
                if entry.data_size_bytes > 0 {
                    info_row(ui, lang.needs_game_data(), &entry.data_size_label());
                }
                info_row(ui, lang.downloads(), &entry.downloads.to_string());
                info_row(ui, lang.rating(), &format!("{:.1} / 5", entry.rating));
                info_row(ui, lang.updated(), &entry.updated_at);
                if !entry.titleid.is_empty() {
                    info_row(ui, "Title ID", &entry.titleid);
                }
                if let Some(page) = &entry.release_page {
                    info_row(ui, lang.release_page(), page);
                }
                ui.add_space(22.0);
                section_label(ui, lang.community());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(lang.your_rating()).size(FONT_BODY).color(TEXT_DIM));
                    ui.add_space(8.0);
                    if let Some(score) = tappable_stars(ui, entry.user_rating) {
                        commands.push(AppCommand::RateCurrent(score));
                    }
                });
                info_row(ui, lang.ratings_count(), &entry.ratings_count.to_string());
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} ({})", lang.comments(), entry.comments_count))
                            .size(FONT_BODY)
                            .strong()
                            .color(TEXT_WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !comment_entry_requested
                            && ui
                                .add(egui::Button::new(
                                    egui::RichText::new(lang.add_comment()).size(FONT_SMALL).color(ACCENT_CYAN),
                                ).frame(false))
                                .clicked()
                        {
                            commands.push(AppCommand::RequestCommentEntry);
                        }
                    });
                });
                ui.add_space(8.0);
                if !comments_loaded {
                    ui.label(egui::RichText::new(lang.loading_comments()).size(FONT_SMALL).color(TEXT_FAINT));
                } else if comments.is_empty() {
                    ui.label(egui::RichText::new(lang.no_comments_yet()).size(FONT_SMALL).color(TEXT_FAINT));
                } else {
                    for comment in comments {
                        egui::Frame::NONE
                            .fill(glass(BG_HEADER))
                            .corner_radius(8.0)
                            .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(&comment.author_name)
                                            .size(FONT_SMALL)
                                            .strong()
                                            .color(ACCENT_CYAN),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(egui::RichText::new(&comment.content).size(FONT_BODY).color(TEXT_WHITE));
                                });
                            });
                        ui.add_space(6.0);
                    }
                }
                ui.add_space(20.0);
                if scroll_delta != 0.0 {
                    ui.scroll_with_delta(egui::vec2(0.0, -scroll_delta));
                }
            });
        });
    commands
}
fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_DIM).size(FONT_BODY));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(FONT_BODY).strong().color(TEXT_WHITE));
        });
    });
    ui.add_space(6.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().min.y, egui::Stroke::new(1.0_f32, SEPARATOR));
    ui.add_space(6.0);
}
fn text_panel(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(glass(BG_CARD))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(FONT_LARGE).color(color));
        });
}
fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).color(ACCENT_CYAN).size(FONT_SMALL).strong());
    ui.add_space(4.0);
}
pub struct SearchFieldResponse {
    pub open_requested: bool,
    pub cleared: bool,
}
fn search_field(ui: &mut egui::Ui, query: &str, placeholder: &str, active: bool) -> SearchFieldResponse {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(SEARCH_FIELD_WIDTH, SEARCH_FIELD_HEIGHT), egui::Sense::click());
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let border = if active { ACCENT_CYAN } else { SEPARATOR };
    ui.painter().rect_filled(rect, 6.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(if active { 2.0_f32 } else { 1.0_f32 }, border),
        egui::StrokeKind::Inside,
    );
    let has_query = !query.is_empty();
    let clear_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 6.0 - SEARCH_CLEAR_SIZE / 2.0, rect.center().y),
        egui::Vec2::splat(SEARCH_CLEAR_SIZE),
    );
    let text_limit = if has_query { clear_rect.left() - 6.0 } else { rect.right() - 10.0 };
    let (text, color) = if has_query { (query, TEXT_WHITE) } else { (placeholder, TEXT_FAINT) };
    ui.painter()
        .with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(text_limit, rect.max.y)))
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
        ui.painter().line_segment([center - egui::vec2(arm, arm), center + egui::vec2(arm, arm)], stroke);
        ui.painter().line_segment([center + egui::vec2(arm, -arm), center + egui::vec2(-arm, arm)], stroke);
        cleared = clear.clicked();
    }
    SearchFieldResponse { open_requested: response.clicked() && !cleared, cleared }
}
fn back_button(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(100.0, 38.0), egui::Sense::click());
    let press_t = if response.is_pointer_button_down_on() { 1.0_f32 } else { 0.0_f32 };
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));
    ui.painter().rect_filled(rect, 6.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    let chevron_x = rect.left() + 20.0;
    let mid_y = rect.center().y;
    let stroke = egui::Stroke::new(2.0_f32, ACCENT_CYAN);
    ui.painter().line_segment(
        [egui::pos2(chevron_x + 4.0, mid_y - 6.0), egui::pos2(chevron_x - 3.0, mid_y)],
        stroke,
    );
    ui.painter().line_segment(
        [egui::pos2(chevron_x - 3.0, mid_y), egui::pos2(chevron_x + 4.0, mid_y + 6.0)],
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
const INSTALL_STEPS: [&str; 3] = ["Download", "Extract", "Install"];
fn install_stepper(ui: &mut egui::Ui, progress: &crate::install::Progress) {
    let failed = matches!(progress, crate::install::Progress::Failed(_));
    let current = progress.step();
    let circle_r = 14.0;
    let spacing = 90.0;
    let total_width = spacing * (INSTALL_STEPS.len() as f32 - 1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_width, circle_r * 2.0 + 22.0), egui::Sense::hover());
    let center_y = rect.top() + circle_r;
    let start_x = rect.center().x - total_width / 2.0;
    for (index, label) in INSTALL_STEPS.iter().enumerate() {
        let step = index + 1;
        let center = egui::pos2(start_x + index as f32 * spacing, center_y);
        if index > 0 {
            let prev_center = egui::pos2(start_x + (index - 1) as f32 * spacing, center_y);
            let line_color = if step <= current && !(failed && step == current) {
                ACCENT_STEAM
            } else {
                SEPARATOR
            };
            ui.painter().line_segment(
                [egui::pos2(prev_center.x + circle_r, center_y), egui::pos2(center.x - circle_r, center_y)],
                egui::Stroke::new(2.0_f32, line_color),
            );
        }
        let is_current = step == current;
        let is_done = step < current || (step == current && !failed && progress.is_finished());
        let (fill, text_color) = if failed && is_current {
            (egui::Color32::from_rgb(0x3a, 0x1c, 0x1c), egui::Color32::from_rgb(0xff, 0x6b, 0x6b))
        } else if is_done || is_current {
            (ACCENT_STEAM, TEXT_WHITE)
        } else {
            (BG_CARD_HOVER, TEXT_DIM)
        };
        ui.painter().circle_filled(center, circle_r, fill);
        if !is_done && !is_current {
            ui.painter().circle_stroke(center, circle_r, egui::Stroke::new(1.0_f32, SEPARATOR));
        }
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            step.to_string(),
            font(FONT_BODY),
            text_color,
        );
        ui.painter().text(
            egui::pos2(center.x, center.y + circle_r + 14.0),
            egui::Align2::CENTER_CENTER,
            *label,
            font(FONT_SMALL),
            if is_current { TEXT_WHITE } else { TEXT_DIM },
        );
    }
}
fn install_status(ui: &mut egui::Ui, progress: &crate::install::Progress) -> bool {
    use crate::install::Progress;
    let finished = progress.is_finished();
    let text = progress.label();
    let galley = ui.fonts(|f| f.layout_no_wrap(text.clone(), font(FONT_SMALL), TEXT_WHITE));
    let width = (galley.size().x + 28.0).min(240.0);
    let sense = if finished { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 36.0), sense);
    let (fill, text_color) = match progress {
        Progress::Done => (GREEN_PLAY.gamma_multiply(0.25), GREEN_PLAY_HOVER),
        Progress::Queued => (egui::Color32::from_rgb(0x3a, 0x30, 0x14), egui::Color32::from_rgb(0xf5, 0xb8, 0x42)),
        Progress::Failed(_) => (egui::Color32::from_rgb(0x3a, 0x1c, 0x1c), egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
        _ => (BG_CARD_HOVER, TEXT_DIM),
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, ACCENT_STEAM), egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &text,
        font(FONT_BODY),
        text_color,
    );
    if !finished {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    response.clicked()
}
fn play_install_button(ui: &mut egui::Ui, label: &str, state: InstallState) -> bool {
    let desired = egui::vec2(130.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let press_t = if response.is_pointer_button_down_on() { 1.0_f32 } else { 0.0_f32 };
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));
    let (base, hover) = match state {
        InstallState::Outdated => (STAR_GOLD, STAR_GOLD.gamma_multiply(1.15)),
        InstallState::Absent | InstallState::Installed => (GREEN_PLAY, GREEN_PLAY_HOVER),
    };
    let bg = base.lerp_to_gamma(hover, hover_t);
    ui.painter().rect_filled(rect, 6.0, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font(FONT_LARGE),
        TEXT_WHITE,
    );
    response.clicked()
}
const SCREENSHOT_SIZE: egui::Vec2 = egui::vec2(240.0, 135.0);
fn screenshots_row(ui: &mut egui::Ui, icons: &IconCache, entry: &crate::data::AppEntry) {
    if entry.screenshot_urls.is_empty() {
        return;
    }
    egui::ScrollArea::horizontal().id_salt("screenshots").show(ui, |ui| {
        ui.horizontal(|ui| {
            for url in &entry.screenshot_urls {
                let (rect, _response) = ui.allocate_exact_size(SCREENSHOT_SIZE, egui::Sense::hover());
                let nearby = rect.left() < ui.clip_rect().right() + SCREENSHOT_SIZE.x;
                draw_screenshot(ui, icons, rect, url, entry.category, nearby);
                ui.add_space(10.0);
            }
        });
    });
    ui.add_space(20.0);
}
fn draw_screenshot(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    url: &str,
    category: Category,
    fetch: bool,
) {
    ui.painter().rect_filled(rect, 8.0, BG_DEEP);
    ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    if !fetch {
        return;
    }
    if let Some(texture) = icons.get_sized(ui.ctx(), url, super::icons::MAX_SCREENSHOT_SIDE) {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            TEXT_WHITE,
        );
        ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
        return;
    }
    if icons.is_loading(url, super::icons::MAX_SCREENSHOT_SIDE) {
        let time = ui.input(|i| i.time);
        let angle = time * 4.0;
        let center = rect.center();
        let radius = 12.0;
        let color = category_color(category);
        let n_dots = 8;
        for i in 0..n_dots {
            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
            let pos = center + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32) * radius;
            let alpha = (i as f32 / n_dots as f32).powf(1.5);
            ui.painter().circle_filled(pos, 2.5, color.gamma_multiply(0.2 + 0.8 * alpha));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No Image",
            font(FONT_SMALL),
            TEXT_DIM,
        );
    }

}
// TODO  PSP, PS1, Tool, Other - CORE

fn category_color(category: Category) -> egui::Color32 {
    match category {
        Category::Emulator => egui::Color32::from_rgb(0xf5, 0x9e, 0x0b),
        Category::Original => egui::Color32::from_rgb(0x3b, 0x82, 0xf6),
        Category::PsVitaGame => egui::Color32::from_rgb(0xec, 0x48, 0x99),
        Category::Ps1Game => egui::Color32::from_rgb(0xea, 0xb3, 0x08),
        Category::PspGame => egui::Color32::from_rgb(0x8b, 0x5c, 0xf6),
        Category::Utility => egui::Color32::from_rgb(0x10, 0xb9, 0x81),
        Category::Port => egui::Color32::from_rgb(0xa8, 0x55, 0xf7),
        Category::Tool => egui::Color32::from_rgb(0x22, 0xd3, 0xee),
        Category::Plugin => egui::Color32::from_rgb(0x06, 0xb6, 0xd4),
        Category::Other => egui::Color32::from_rgb(0x64, 0x74, 0x8b),
    }
}
fn draw_icon(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let color = category_color(entry.category);
    let corner_r = rect.width() * 0.22;
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, corner_r, TEXT_WHITE).with_texture(
                    texture.id(),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                ),
            );
            return;
        }
    }
    ui.painter().rect_filled(rect, corner_r, BG_DEEP);
    ui.painter().rect_stroke(rect, corner_r, egui::Stroke::new(1.5_f32, color), egui::StrokeKind::Inside);
    if art.is_some_and(|url| icons.is_loading(url, super::icons::MAX_ICON_SIDE)) {
        let time = ui.input(|i| i.time);
        let angle = time * 4.0;
        let center = rect.center();
        let radius = rect.width() * 0.18;
        let n_dots = 8;
        for i in 0..n_dots {
            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
            let pos = center + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32) * radius;
            let alpha = (i as f32 / n_dots as f32).powf(1.5);
            ui.painter().circle_filled(pos, 2.0, color.gamma_multiply(0.2 + 0.8 * alpha));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        return;
    }
    let letter = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font((rect.width() * 0.42).round()),
        TEXT_WHITE,
    );
}

