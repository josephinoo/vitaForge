use super::i18n::Language;
use super::icons::IconCache;
use super::{App, AppState};
use crate::data::{Category, Platform, SortDirection, SortOrder};
use crate::input::{AppCommand, DiscoverRail, StoreTab};
use crate::install::installed::{InstallState, InstalledIndex};
const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(0x00, 0x00, 0x00);
const BG_HEADER: egui::Color32 = egui::Color32::from_rgb(0x1c, 0x1c, 0x1e);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0x1c, 0x1c, 0x1e);
const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(0x2c, 0x2c, 0x2e);
const ACCENT_STEAM: egui::Color32 = egui::Color32::from_rgb(0x0a, 0x84, 0xff);
const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(0x6a, 0xc4, 0xdc);
const GREEN_PLAY: egui::Color32 = egui::Color32::from_rgb(0x30, 0xd1, 0x58);
const GREEN_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x4c, 0xdd, 0x70);
const BLUE_PLAY: egui::Color32 = egui::Color32::from_rgb(0x0a, 0x84, 0xff);
const BLUE_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x33, 0x96, 0xff);
const SEPARATOR: egui::Color32 = egui::Color32::from_rgb(0x38, 0x38, 0x3a);
const TEXT_WHITE: egui::Color32 = egui::Color32::from_rgb(0xff, 0xff, 0xff);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x8e, 0x8e, 0x93);
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(0x48, 0x48, 0x4a);
const STAR_GOLD: egui::Color32 = egui::Color32::from_rgb(0xff, 0xd6, 0x0a);
const STAR_GOLD_HOVER: egui::Color32 = egui::Color32::from_rgb(0xff, 0xe0, 0x66);
const GLASS_ALPHA: f32 = 0.78;
const GLASS_EDGE: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x2c, 0x2c, 0x2e, 0x55);
const FONT_MICRO: f32 = 10.0;
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
const RADIUS_XS: f32 = 4.0;
const RADIUS_SM: f32 = 6.0;
const RADIUS_MD: f32 = 8.0;
const RADIUS_LG: f32 = 10.0;
const CARD_RADIUS: f32 = 14.0;
const HINT_BAR_HEIGHT: f32 = 36.0;
const SEARCH_FIELD_WIDTH: f32 = 190.0;
const SEARCH_FIELD_HEIGHT: f32 = 30.0;
const SEARCH_CLEAR_SIZE: f32 = 18.0;
pub const GRID_COLUMNS: usize = 6;
const ART_LOOKAHEAD: usize = 6;
const GRID_COL_SPACING: f32 = 4.0;
const GRID_ROW_SPACING: f32 = 4.0;
const GRID_VISIBLE_ROWS: f32 = 2.0;
const GRID_BOTTOM_PAD: f32 = 8.0;
const SCREEN_MARGIN: f32 = 16.0;
const FEATURED_BANNER_CARD_HEIGHT: f32 = 96.0;
const FEATURED_BANNER_HEIGHT: f32 = FEATURED_BANNER_CARD_HEIGHT + 10.0 + 1.0 + 10.0;
const SCROLLBAR_RESERVE: f32 = 6.0;
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
    style.animation_time = 0.12;
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
    let self_update = app.self_update.as_ref();
    let self_update_progress = app
        .install
        .as_ref()
        .filter(|job| job.app_id == super::SELF_UPDATE_ID)
        .map(|job| &job.progress);
    let commands = match &app.state {
        AppState::Loading => loading_screen(
            ctx,
            app.lang,
            app.install.as_ref().map(|j| &j.progress),
            self_update,
        ),
        AppState::Catalog(catalog) => catalog_view(ctx, app, catalog, self_update, self_update_progress),
        AppState::Detail { app: entry, scroll_delta, comments, comments_loaded, comment_entry_requested, lightbox, data_prompt, .. } => {
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
                *lightbox,
                *data_prompt,
            )
        }
        AppState::Settings { selected, previous } => {
            let mut commands = catalog_view(ctx, app, previous, self_update, self_update_progress);
            commands.extend(settings_modal(
                ctx,
                app.lang,
                *selected,
                previous.apps.len(),
                &app.cache_stats,
                app.cache_notice.as_deref(),
            ));
            commands
        }
    };
    app.icons.maintain(ctx);
    if app.icons.rate_limited_for().is_some() {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
    commands
}
fn catalog_view(
    ctx: &egui::Context,
    app: &App,
    catalog: &super::CatalogState,
    self_update: Option<&super::SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
) -> Vec<AppCommand> {
    catalog_screen(
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
        catalog.total_unique_count,
        &catalog.category_counts,
        catalog.featured_index,
        catalog.tab,
        catalog.discover_home,
        catalog.discover_focus,
        catalog.scroll_category_into_view,
        &catalog.top_rail,
        &catalog.recent_rail,
        self_update,
        self_update_progress,
    )
}
fn settings_modal(
    ctx: &egui::Context,
    lang: Language,
    selected: usize,
    catalog_count: usize,
    stats: &crate::data::cache_manager::CacheStats,
    cache_notice: Option<&str>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::new("settings_modal_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            let panel = egui::Rect::from_center_size(screen.center(), egui::vec2(650.0, 410.0));
            let backdrop = ui.interact(screen, ui.id().with("dismiss"), egui::Sense::click());
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(175));
            if backdrop.clicked() && !backdrop.interact_pointer_pos().is_some_and(|pos| panel.contains(pos)) {
                commands.push(AppCommand::CloseSettings);
            }
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                egui::Frame::new()
                    .fill(BG_CARD)
                    .stroke(egui::Stroke::new(1.0, GLASS_EDGE))
                    .corner_radius(egui::CornerRadius::same(CARD_RADIUS as u8))
                    .inner_margin(egui::Margin::same(20))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(lang.settings_title()).size(FONT_LARGE).strong().color(TEXT_WHITE));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if back_button(ui, lang.back()) {
                                    commands.push(AppCommand::CloseSettings);
                                }
                            });
                        });
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(180.0);
                                ui.label(egui::RichText::new(lang.language_label()).size(FONT_SMALL).color(TEXT_DIM));
                                ui.add_space(8.0);
                                for (index, language) in Language::ALL.into_iter().enumerate() {
                                    if dropdown_row(ui, language.label(), selected == index) {
                                        commands.push(AppCommand::SetLanguage(language));
                                    }
                                }
                            });
                            ui.add_space(22.0);
                            ui.vertical(|ui| {
                                ui.set_width(390.0);
                                ui.label(egui::RichText::new(lang.settings_storage()).size(FONT_SMALL).color(TEXT_DIM));
                                ui.add_space(6.0);
                                let info_rows = [
                                    (lang.settings_version(), format!("VitaForge {}", env!("CARGO_PKG_VERSION"))),
                                    (lang.settings_catalog(), format!("{catalog_count}")),
                                    (lang.settings_icon_cache(), crate::data::cache_manager::format_bytes(stats.icons_bytes)),
                                    (lang.settings_catalog_cache(), crate::data::cache_manager::format_bytes(stats.catalog_bytes)),
                                    (lang.settings_hash_cache(), crate::data::cache_manager::format_bytes(stats.hashes_bytes)),
                                    (lang.settings_total(), crate::data::cache_manager::format_bytes(stats.total_bytes)),
                                ];
                                for (label, value) in info_rows {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(label).size(FONT_MICRO).color(TEXT_DIM));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(value).size(FONT_MICRO).color(TEXT_WHITE));
                                        });
                                    });
                                }
                                ui.add_space(10.0);
                                if dropdown_row(ui, lang.settings_clear_icons(), selected == 5) {
                                    commands.push(AppCommand::ClearIconCache);
                                }
                                if dropdown_row(ui, lang.settings_clear_catalog(), selected == 6) {
                                    commands.push(AppCommand::ClearCatalogCache);
                                }
                                if dropdown_row(ui, lang.settings_purge_all(), selected == 7) {
                                    commands.push(AppCommand::PurgeAllCache);
                                }
                            });
                        });
                        if let Some(notice) = cache_notice {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(notice).size(FONT_SMALL).color(GREEN_PLAY));
                        }
                    });
            });
        });
    commands
}
fn loading_screen(
    ctx: &egui::Context,
    lang: Language,
    install_progress: Option<&crate::install::Progress>,
    self_update: Option<&super::SelfUpdateInfo>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_background(ui.painter(), rect);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
                    |ui| {
                        let total_content_height = 360.0;
                        let pad_y = ((rect.height() - total_content_height) / 2.0).max(0.0);
                        ui.add_space(pad_y);
                        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(168.0, 168.0), egui::Sense::hover());
                        let logo = logo_texture(ui.ctx());
                        ui.painter().image(
                            logo.id(),
                            icon_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new("VitaForge").size(FONT_HEADLINE).strong().color(ACCENT_STEAM));
                        ui.label(
                            egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                .size(FONT_SMALL)
                                .color(TEXT_DIM),
                        );
                        ui.label(egui::RichText::new("by josephinoo").size(FONT_SMALL).color(TEXT_FAINT));
                        ui.add_space(8.0);
                        if let Some(progress) = install_progress {
                            let tag = self_update.map_or("", |u| u.tag.as_str());
                            let title = if tag.is_empty() {
                                "Installing...".to_owned()
                            } else {
                                format!("Updating VitaForge {tag}...")
                            };
                            ui.label(
                                egui::RichText::new(title)
                                    .color(STAR_GOLD)
                                    .size(FONT_LARGE)
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(progress.label()).color(TEXT_DIM).size(FONT_BODY));
                        } else {
                            ui.label(
                                egui::RichText::new("Bringing catalog information from databases...")
                                    .color(TEXT_DIM)
                                    .size(FONT_BODY),
                            );
                            if let Some(info) = self_update {
                                ui.add_space(14.0);
                                if self_update_pill(ui, lang, &info.tag) {
                                    commands.push(AppCommand::SelfUpdate);
                                }
                            }
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
    ctx.request_repaint_after(std::time::Duration::from_millis(200));
    commands
}

fn self_update_pill(ui: &mut egui::Ui, lang: Language, tag: &str) -> bool {
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
    let size = egui::vec2(galley.size().x + pad_x * 2.0 + icon_w + 6.0, galley.size().y + pad_y * 2.0);
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
    ui.painter().add(egui::Shape::convex_polygon(tri.to_vec(), TEXT_WHITE, egui::Stroke::NONE));
    ui.painter().galley(
        egui::pos2(rect.left() + pad_x + icon_w + 6.0, rect.center().y - galley.size().y * 0.5),
        galley,
        TEXT_WHITE,
    );
    response.clicked()
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
    total_unique_count: usize,
    category_counts: &[(Category, usize)],
    featured_index: Option<usize>,
    tab: StoreTab,
    discover_home: bool,
    discover_focus: super::DiscoverFocus,
    scroll_category_into_view: bool,
    top_rail: &[usize],
    recent_rail: &[usize],
    self_update: Option<&super::SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let show_discover_rails = tab == StoreTab::Discover && discover_home && search_query.trim().is_empty();
    let show_category_pills = matches!(tab, StoreTab::Discover | StoreTab::Search) && !show_discover_rails;
    let shoulder_hint = if show_category_pills {
        lang.btn_category()
    } else {
        lang.btn_tabs()
    };
    let hints = vec![
        (Glyph::Cross, lang.btn_open()),
        (Glyph::Circle, lang.btn_back()),
        (Glyph::Triangle, lang.btn_search()),
        (Glyph::Shoulders, shoulder_hint),
    ];
    commands.extend(button_hints(
        ctx,
        &hints,
        Some(status_note(installed, icons)),
        Some(installed),
    ));
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)))
        .show(ctx, |ui| {
            paint_background(ui.painter(), ui.ctx().screen_rect());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("VitaForge").size(FONT_TITLE).strong().color(ACCENT_STEAM));
                ui.label(egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).size(FONT_SMALL).color(TEXT_DIM));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(lang.apps_count(filtered_indices.len())).color(TEXT_FAINT).size(FONT_SMALL));
                if let Some(progress) = self_update_progress {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(progress.label())
                            .size(FONT_SMALL)
                            .strong()
                            .color(STAR_GOLD),
                    );
                } else if let Some(info) = self_update {
                    ui.add_space(10.0);
                    if self_update_pill(ui, lang, &info.tag) {
                        commands.push(AppCommand::SelfUpdate);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tab == StoreTab::Search {
                        let field = search_field(ui, search_query, lang.search_placeholder(), search_active);
                        if field.cleared {
                            commands.push(AppCommand::SetSearchQuery(String::new()));
                        }
                        if field.open_requested {
                            commands.push(AppCommand::RequestSearch);
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                for (store_tab, label) in [
                    (StoreTab::Discover, lang.tab_discover()),
                    (StoreTab::Library, lang.tab_library()),
                    (StoreTab::Updates, lang.tab_updates()),
                    (StoreTab::Search, lang.tab_search()),
                ] {
                    if pill_button(ui, label, tab == store_tab) {
                        commands.push(AppCommand::SetStoreTab(store_tab));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(picked) = source_dropdown(ui, total_unique_count, source_counts, source_filter) {
                        commands.push(AppCommand::SetSourceFilter(picked));
                    }
                });
            });
            ui.add_space(6.0);

            if show_discover_rails {
                discover_home_ui(
                    ui,
                    icons,
                    installed,
                    lang,
                    apps,
                    featured_index,
                    discover_focus,
                    top_rail,
                    recent_rail,
                    is_commercial_view,
                    scroll_to_selected,
                    &mut commands,
                );
                return;
            }

            if tab == StoreTab::Discover || tab == StoreTab::Search {
                if !discover_home && tab == StoreTab::Discover {
                    ui.horizontal(|ui| {
                        let back = ui.add(
                            egui::Button::new(
                                egui::RichText::new(lang.see_all_back())
                                    .size(FONT_SMALL)
                                    .color(ACCENT_CYAN),
                            )
                            .frame(false),
                        );
                        if back.clicked() {
                            commands.push(AppCommand::BackToDiscoverHome);
                        }
                    });
                    ui.add_space(4.0);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    egui::ScrollArea::horizontal()
                        .id_salt("category_pills")
                        .max_height(32.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let all_resp = pill_button(ui, lang.category_label(None), category_filter.is_none());
                                if all_resp {
                                    commands.push(AppCommand::SetCategoryFilter(None));
                                }
                                if scroll_category_into_view && category_filter.is_none() {
                                    ui.scroll_to_cursor(Some(egui::Align::Center));
                                }
                                for &(category, _count) in category_counts {
                                    let active = category_filter == Some(category);
                                    if pill_button(ui, lang.category_label(Some(category)), active) {
                                        commands.push(AppCommand::SetCategoryFilter(Some(category)));
                                    }
                                    if scroll_category_into_view && active {
                                        ui.scroll_to_cursor(Some(egui::Align::Center));
                                    }
                                }
                            });
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(picked) =
                            sort_dropdown(ui, lang, sort_order, sort_direction)
                        {
                            commands.push(AppCommand::SetSortOrder(picked));
                        }
                    });
                });
                ui.add_space(6.0);
            } else {
                ui.add_space(4.0);
            }

            if filtered_indices.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    let empty = match tab {
                        StoreTab::Library => lang.library_empty(),
                        StoreTab::Updates => lang.updates_empty(),
                        _ => lang.no_results(),
                    };
                    ui.label(egui::RichText::new(empty).size(FONT_LARGE).color(TEXT_DIM));
                    ui.add_space(8.0);
                    if !matches!(tab, StoreTab::Library | StoreTab::Updates) {
                        ui.label(egui::RichText::new(lang.no_results_sub()).size(FONT_BODY).color(TEXT_FAINT));
                    }
                });
                return;
            }

            let featured_entry = if tab == StoreTab::Discover
                && discover_home
                && search_query.trim().is_empty()
                && !is_commercial_view
            {
                featured_index.and_then(|idx| apps.get(idx))
            } else {
                None
            };
            let banner_height = if featured_entry.is_some() { FEATURED_BANNER_HEIGHT } else { 0.0 };
            let viewport_h = (ui.available_height() - GRID_BOTTOM_PAD).max(80.0);
            let available = (ui.available_width() - SCROLLBAR_RESERVE).max(0.0);
            let aspect_ratio = 1.0;
            let mut max_card_h = ((viewport_h / GRID_VISIBLE_ROWS) - GRID_ROW_SPACING).max(52.0);
            let max_card_w =
                (available - GRID_COL_SPACING * (GRID_COLUMNS as f32 - 1.0)) / GRID_COLUMNS as f32;
            let mut card_width = max_card_w.min(max_card_h / aspect_ratio);
            let mut card_height = card_width * aspect_ratio;
            let mut row_height = card_height + GRID_ROW_SPACING;
            let rows_that_fit = (viewport_h / row_height).floor().max(1.0);
            let leftover_h = viewport_h - rows_that_fit * row_height;
            if leftover_h > 0.0 && leftover_h < card_height * 0.35 {
                let fit_h = ((viewport_h / rows_that_fit) - GRID_ROW_SPACING).max(52.0);
                max_card_h = fit_h.min(max_card_h);
                card_width = max_card_w.min(max_card_h / aspect_ratio);
                card_height = card_width * aspect_ratio;
                row_height = card_height + GRID_ROW_SPACING;
            }
            let gaps = (GRID_COLUMNS as f32 - 1.0).max(1.0);
            let leftover =
                (available - card_width * GRID_COLUMNS as f32 - GRID_COL_SPACING * gaps).max(0.0);
            let col_spacing = GRID_COL_SPACING;
            let row_inset = leftover * 0.5;
            let total_rows = filtered_indices.len().div_ceil(GRID_COLUMNS);
            let mut scroll_area = egui::ScrollArea::vertical()
                .id_salt("catalog_grid")
                .max_height(viewport_h);
            if scroll_reset {
                scroll_area = scroll_area.vertical_scroll_offset(0.0);
            }
            scroll_area.show_viewport(ui, |ui, viewport| {
                ui.set_height(banner_height + row_height * total_rows as f32 + GRID_BOTTOM_PAD);

                if scroll_to_selected && !scroll_reset && let Some(cursor) = selected {
                    let row = cursor / GRID_COLUMNS;
                    let row_top = ui.max_rect().top() + banner_height + row as f32 * row_height;

                    let target_top = if row == 0 { ui.max_rect().top() } else { row_top };
                    let row_rect = egui::Rect::from_x_y_ranges(
                        ui.max_rect().x_range(),
                        target_top..=(row_top + row_height + GRID_BOTTOM_PAD),
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
                        let banner = featured_banner(ui, icons, lang, entry, false);
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
                    let mut art_wakeup: Option<std::time::Duration> = None;
                    for grid_row in row_range {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if row_inset > 0.0 {
                                ui.add_space(row_inset);
                            }
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
                                    ui.add_space(col_spacing);
                                }
                            }
                            if row_inset > 0.0 {
                                ui.add_space(row_inset);
                            }
                        });
                        ui.add_space(GRID_ROW_SPACING);
                    }
                    let prefetch_start = min_row.saturating_sub(1) * GRID_COLUMNS;
                    let prefetch_end = ((max_row + 1) * GRID_COLUMNS + ART_LOOKAHEAD)
                        .min(filtered_indices.len());
                    let mut warm = Vec::new();
                    for item_index in prefetch_start..prefetch_end {
                        if let Some(&real_index) = filtered_indices.get(item_index)
                            && let Some(entry) = apps.get(real_index)
                            && let Some(url) = tile_art_url(entry)
                        {
                            warm.push(url.to_owned());
                        }
                    }
                    icons.prefetch_urls(ui.ctx(), warm);
                    if let Some(delay) = art_wakeup {
                        ui.ctx().request_repaint_after(delay);
                    }
                });
            });
            ui.add_space(GRID_BOTTOM_PAD);
        });
    commands
}

fn discover_home_ui(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    apps: &[crate::data::AppEntry],
    featured_index: Option<usize>,
    discover_focus: super::DiscoverFocus,
    top_rail: &[usize],
    recent_rail: &[usize],
    is_commercial_view: bool,
    scroll_focus_into_view: bool,
    commands: &mut Vec<AppCommand>,
) {
    let mut warm = Vec::new();
    if let Some(entry) = featured_index.and_then(|idx| apps.get(idx))
        && let Some(url) = tile_art_url(entry)
    {
        warm.push(url.to_owned());
    }
    for indices in [top_rail, recent_rail] {
        for &real_index in indices.iter().take(16) {
            if let Some(entry) = apps.get(real_index)
                && let Some(url) = tile_art_url(entry)
            {
                warm.push(url.to_owned());
            }
        }
    }
    icons.prefetch_urls(ui.ctx(), warm);
    let viewport_height = ui.available_height();
    egui::ScrollArea::vertical()
        .id_salt("discover_home")
        .max_height(viewport_height)
        .auto_shrink([false, false])
        .drag_to_scroll(true)
        .show(ui, |ui| {
            let is_featured = matches!(discover_focus, super::DiscoverFocus::Featured);
            if !is_commercial_view
                && let Some(entry) = featured_index.and_then(|idx| apps.get(idx))
            {
                let banner_resp = ui.scope(|ui| {
                    let banner = featured_banner(ui, icons, lang, entry, is_featured);
                    if banner.clicked {
                        commands.push(AppCommand::SelectAppById(entry.id.clone()));
                    }
                }).response;
                if scroll_focus_into_view && is_featured {
                    banner_resp.scroll_to_me(Some(egui::Align::TOP));
                }
                ui.add_space(12.0);
            }
            let top_selected = match discover_focus {
                super::DiscoverFocus::Top(i) => Some(i),
                _ => None,
            };
            let top_resp = ui.scope(|ui| {
                rail_section(
                    ui,
                    icons,
                    installed,
                    lang,
                    lang.rail_top(),
                    DiscoverRail::Top,
                    apps,
                    top_rail,
                    top_selected,
                    is_commercial_view,
                    commands,
                );
            }).response;
            if scroll_focus_into_view && top_selected.is_some() && !is_featured {
                top_resp.scroll_to_me(None);
            }
            ui.add_space(14.0);
            let new_selected = match discover_focus {
                super::DiscoverFocus::New(i) => Some(i),
                _ => None,
            };
            let new_resp = ui.scope(|ui| {
                rail_section(
                    ui,
                    icons,
                    installed,
                    lang,
                    lang.rail_new(),
                    DiscoverRail::New,
                    apps,
                    recent_rail,
                    new_selected,
                    is_commercial_view,
                    commands,
                );
            }).response;
            if scroll_focus_into_view && new_selected.is_some() {
                new_resp.scroll_to_me(None);
            }
            ui.add_space(16.0);
            let browse_focused = matches!(discover_focus, super::DiscoverFocus::BrowseAll);
            let browse_resp = ui.scope(|ui| {
                if pill_button(ui, lang.see_all_catalog(), browse_focused) {
                    commands.push(AppCommand::SeeAllRail(DiscoverRail::Top));
                }
            }).response;
            ui.add_space(GRID_BOTTOM_PAD + 16.0);
            if scroll_focus_into_view && browse_focused {
                browse_resp.scroll_to_me(Some(egui::Align::BOTTOM));
            }
        });
}

fn rail_section(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    title: &str,
    rail: DiscoverRail,
    apps: &[crate::data::AppEntry],
    indices: &[usize],
    selected_index: Option<usize>,
    _is_commercial_view: bool,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).size(FONT_LARGE).strong().color(TEXT_WHITE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill_button(ui, lang.see_all(), false) {
                commands.push(AppCommand::SeeAllRail(rail));
            }
        });
    });
    ui.add_space(8.0);
    if indices.is_empty() {
        ui.label(egui::RichText::new("—").size(FONT_BODY).color(TEXT_FAINT));
        return;
    }
    let tile = egui::vec2(96.0, 96.0);
    egui::ScrollArea::horizontal()
        .id_salt(title)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                for (rail_i, &real_index) in indices.iter().take(16).enumerate() {
                    let Some(entry) = apps.get(real_index) else { continue };
                    let focused = selected_index == Some(rail_i);
                    let card = ui
                        .push_id(("rail", title, entry.id.as_str()), |ui| {
                            cover_card(ui, icons, installed, entry, tile.x, tile.y, focused)
                        })
                        .inner;
                    if focused {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                    if card.clicked {
                        commands.push(AppCommand::SelectAppById(entry.id.clone()));
                    }
                }
            });
        });
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
fn sort_dropdown(
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
    ui.painter().rect_filled(rect, rect.height() / 2.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
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
    chevron(ui.painter(), egui::pos2(rect.right() - 10.0, rect.center().y), open);
    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }
    let mut picked = None;
    egui::popup_below_widget(ui, popup_id, &response, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(170.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        for sort in SortOrder::ALL {
            if dropdown_row(ui, lang.sort_label(sort), sort == sort_order) {
                picked = Some(sort);
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
        ui.painter().rect_filled(rect, RADIUS_XS, ACCENT_STEAM.gamma_multiply(0.22));
    } else if hover_t > 0.0 {
        ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD_HOVER.gamma_multiply(hover_t));
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
    let ctx = ui.ctx().clone();
    let anim_id = egui::Id::new(("cover_card", entry.id.as_str()));
    let hover_t = ctx.animate_bool(anim_id.with("hover"), response.hovered());
    let press_t = ctx.animate_bool(anim_id.with("press"), response.is_pointer_button_down_on());
    let zoom = if focused { 1.0 } else { hover_t };
    let inset = 1.5 - zoom * 1.0 + press_t * PRESS_SHRINK;
    let rect = full_rect.shrink(inset.max(0.0));
    draw_cover(ui, icons, rect, entry);
    tile_label(ui, rect, &entry.name);
    tile_source_badge(ui, rect, entry);
    if entry.platform != Platform::Vita {
        tile_platform_badge(ui, rect, entry.platform);
    }
    install_marker(ui.painter(), rect, installed.state(entry));
    if focused {
        ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(2.5_f32, ACCENT_CYAN), egui::StrokeKind::Inside);
    } else {
        ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    }
    CardResponse { clicked: response.clicked() }
}
fn featured_banner(
    ui: &mut egui::Ui,
    icons: &IconCache,
    lang: Language,
    entry: &crate::data::AppEntry,
    focused: bool,
) -> CardResponse {
    let width = ui.available_width();
    let height = FEATURED_BANNER_CARD_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let anim_id = egui::Id::new(("featured_banner", entry.id.as_str())).with("hover");
    let hover_t = ui.ctx().animate_bool(anim_id, response.hovered() || focused);
    let active_t = if focused { 1.0 } else { hover_t };
    ui.painter().rect_filled(rect, CARD_RADIUS, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, active_t));
    let stroke = if focused {
        egui::Stroke::new(2.5_f32, ACCENT_CYAN)
    } else {
        egui::Stroke::new(1.0_f32, ACCENT_CYAN.gamma_multiply(0.45))
    };
    ui.painter().rect_stroke(rect, CARD_RADIUS, stroke, egui::StrokeKind::Inside);
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
                ui.painter().rect_filled(badge_rect, RADIUS_XS, ACCENT_CYAN.gamma_multiply(0.25));
                ui.painter().galley(badge_rect.center() - galley.size() / 2.0, galley, ACCENT_CYAN);
                ui.add_space(6.0);
                category_badge(ui, entry.category);
                if let Some(source) = crate::data::SourceCatalog::from_api(&entry.source_catalog) {
                    ui.add_space(4.0);
                    source_chip(ui, source);
                }
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
    ui.painter().rect_filled(button_rect, button_size.y / 2.0, GREEN_PLAY.lerp_to_gamma(GREEN_PLAY_HOVER, active_t));
    ui.painter().text(
        button_rect.center(),
        egui::Align2::CENTER_CENTER,
        lang.view_details_label(),
        font(FONT_SMALL),
        TEXT_WHITE,
    );
    CardResponse { clicked: response.clicked() }
}
use super::tile_art_url;
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
fn draw_cover(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    ui.painter().rect_filled(rect, CARD_RADIUS, BG_CARD);

    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            let texture_size = texture.size_vec2();
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, CARD_RADIUS, TEXT_WHITE)
                    .with_texture(texture.id(), cover_fit_uv(texture_size, rect)),
            );
            return;
        }
    }

    let color = category_color(entry.category);
    let letter = entry
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let loading = art.is_some_and(|url| icons.is_loading(url, super::icons::MAX_ICON_SIDE));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font((rect.width().min(rect.height()) * 0.38).round()),
        TEXT_WHITE.gamma_multiply(if loading { 0.7 } else { 1.0 }),
    );
    if loading {
        let bar_w = rect.width() * 0.42;
        let bar_h = 3.0;
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.bottom() - 14.0),
            egui::vec2(bar_w, bar_h),
        );
        ui.painter().rect_filled(bar, 2.0, color.gamma_multiply(0.25));
        let fill = egui::Rect::from_min_size(bar.left_top(), egui::vec2(bar_w * 0.6, bar_h));
        ui.painter().rect_filled(fill, 2.0, color.gamma_multiply(0.9));
    }
}
fn tile_label(ui: &mut egui::Ui, rect: egui::Rect, name: &str) {
    let scrim_height = (rect.height() * 0.34).clamp(36.0, 52.0);
    let scrim = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - scrim_height),
        rect.right_bottom(),
    );
    let mut mesh = egui::Mesh::default();
    let top = egui::Color32::from_black_alpha(0);
    let bot = egui::Color32::from_black_alpha(210);
    let tl = mesh.vertices.len() as u32;
    mesh.colored_vertex(scrim.left_top(), top);
    mesh.colored_vertex(scrim.right_top(), top);
    mesh.colored_vertex(scrim.right_bottom(), bot);
    mesh.colored_vertex(scrim.left_bottom(), bot);
    mesh.add_triangle(tl, tl + 1, tl + 2);
    mesh.add_triangle(tl, tl + 2, tl + 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    let side_margin = 8.0;
    let available_width = (rect.width() - side_margin * 2.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple(
        name.to_owned(),
        font(FONT_SMALL),
        TEXT_WHITE,
        available_width,
    );
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|f| f.layout_job(job));
    let pos = egui::pos2(
        rect.left() + side_margin,
        rect.bottom() - 8.0 - galley.size().y,
    );
    ui.painter().galley(pos, galley, TEXT_WHITE);
}
fn tile_source_badge(ui: &mut egui::Ui, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let Some(source) = crate::data::SourceCatalog::from_api(&entry.source_catalog) else {
        return;
    };
    let text = source.short_label();
    let (fg, bg) = source_chip_colors(source);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font(FONT_MICRO), fg));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let y = if entry.platform != Platform::Vita { 24.0 } else { 7.0 };
    let badge = egui::Rect::from_min_size(rect.left_top() + egui::vec2(7.0, y), size);
    ui.painter().rect_filled(badge, RADIUS_XS, bg);
    ui.painter().galley(badge.center() - galley.size() / 2.0, galley, fg);
}
fn source_chip_colors(source: crate::data::SourceCatalog) -> (egui::Color32, egui::Color32) {
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
fn source_chip(ui: &mut egui::Ui, source: crate::data::SourceCatalog) {
    let (fg, bg) = source_chip_colors(source);
    let galley = ui.fonts(|f| f.layout_no_wrap(source.short_label().to_owned(), font(FONT_MICRO), fg));
    let size = egui::vec2(galley.size().x + 10.0, 16.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_XS, bg);
    ui.painter().rect_stroke(rect, RADIUS_XS, egui::Stroke::new(1.0_f32, fg.gamma_multiply(0.55)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
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
    ui.painter().rect_filled(badge, RADIUS_XS, bg_color);
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
    ui.painter().rect_filled(rect, RADIUS_XS, color.gamma_multiply(0.3));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
#[derive(Clone, Copy)]
enum Glyph {
    Cross,
    Circle,
    Triangle,
    Square,
    Shoulders,
}
static BUTTON_TEXTURES: std::sync::OnceLock<[egui::TextureHandle; 4]> = std::sync::OnceLock::new();
fn button_texture(ctx: &egui::Context, glyph: Glyph) -> Option<egui::TextureHandle> {
    let textures = BUTTON_TEXTURES.get_or_init(|| {
        let decode = |name: &str, bytes: &[u8]| {
            let decoded = image::load_from_memory(bytes)
                .unwrap_or_else(|err| panic!("assets/buttons/{name} is bundled at compile time and must decode: {err}"))
                .to_rgba8();
            let size = [decoded.width() as usize, decoded.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };
        [
            decode("ps-button-x", include_bytes!("../../assets/buttons/ps-button-x.png")),
            decode("ps-button-c", include_bytes!("../../assets/buttons/ps-button-c.png")),
            decode("ps-button-t", include_bytes!("../../assets/buttons/ps-button-t.png")),
            decode("ps-button-s", include_bytes!("../../assets/buttons/ps-button-s.png")),
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
enum StatusNote {
    RateLimited(String),
    Overview {
        installed: usize,
        updates: usize,
        storage: Option<(f64, f64)>,
    },
}

fn status_note(installed: &InstalledIndex, icons: &IconCache) -> StatusNote {
    if let Some(left) = icons.rate_limited_for() {
        let secs = left.as_secs();
        return StatusNote::RateLimited(if secs >= 60 {
            format!("Artwork resumes in {}m {:02}s", secs / 60, secs % 60)
        } else {
            format!("Artwork resumes in {}s", secs.max(1))
        });
    }
    let (installed_count, outdated_count) = installed.counts();
    let storage = super::sysinfo::storage("ux0:").map(|(used, total)| {
        let gb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        (gb(used), gb(total))
    });
    StatusNote::Overview {
        installed: installed_count,
        updates: outdated_count,
        storage,
    }
}

fn status_icon(painter: &egui::Painter, center: egui::Pos2, kind: StatusIcon, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.35, color);
    match kind {
        StatusIcon::Library => {
            let rect = egui::Rect::from_center_size(center, egui::vec2(9.0, 7.0));
            painter.rect_stroke(rect, 1.5, stroke, egui::StrokeKind::Inside);
            painter.line_segment([egui::pos2(rect.left(), center.y - 1.0), egui::pos2(rect.right(), center.y - 1.0)], stroke);
            painter.line_segment([egui::pos2(center.x, center.y - 1.0), egui::pos2(center.x, rect.bottom())], stroke);
        }
        StatusIcon::Update => {
            painter.line_segment([egui::pos2(center.x, center.y + 4.0), egui::pos2(center.x, center.y - 4.0)], stroke);
            painter.line_segment([egui::pos2(center.x - 3.0, center.y - 1.0), egui::pos2(center.x, center.y - 4.0)], stroke);
            painter.line_segment([egui::pos2(center.x + 3.0, center.y - 1.0), egui::pos2(center.x, center.y - 4.0)], stroke);
            painter.line_segment([egui::pos2(center.x - 4.0, center.y + 4.0), egui::pos2(center.x + 4.0, center.y + 4.0)], stroke);
        }
        StatusIcon::Storage => {
            painter.circle_stroke(center, 4.5, stroke);
            painter.line_segment([center, egui::pos2(center.x, center.y - 4.5)], stroke);
            painter.line_segment([center, egui::pos2(center.x + 3.5, center.y + 2.5)], stroke);
        }
        StatusIcon::Alert => {
            let points = [
                egui::pos2(center.x, center.y - 5.0),
                egui::pos2(center.x + 5.0, center.y + 4.0),
                egui::pos2(center.x - 5.0, center.y + 4.0),
            ];
            painter.add(egui::Shape::convex_polygon(points.to_vec(), color.gamma_multiply(0.2), stroke));
            painter.line_segment([egui::pos2(center.x, center.y - 2.0), egui::pos2(center.x, center.y + 1.0)], stroke);
            painter.circle_filled(egui::pos2(center.x, center.y + 2.8), 0.9, color);
        }
    }
}

#[derive(Clone, Copy)]
enum StatusIcon {
    Library,
    Update,
    Storage,
    Alert,
}

fn status_chip(
    ui: &mut egui::Ui,
    pos: egui::Pos2,
    text: &str,
    icon: StatusIcon,
    color: egui::Color32,
) -> egui::Rect {
    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text.to_owned(), font(FONT_MICRO), color));
    let size = egui::vec2(galley.size().x + 28.0, 22.0);
    let rect = egui::Rect::from_min_size(pos, size);
    ui.painter().rect_filled(rect, 11.0, color.gamma_multiply(0.13));
    ui.painter().rect_stroke(rect, 11.0, egui::Stroke::new(1.0, color.gamma_multiply(0.34)), egui::StrokeKind::Inside);
    status_icon(ui.painter(), egui::pos2(rect.left() + 11.0, rect.center().y), icon, color);
    ui.painter().galley(egui::pos2(rect.left() + 20.0, rect.center().y - galley.size().y * 0.5), galley, color);
    rect
}

fn status_note_widget(ui: &mut egui::Ui, rect: egui::Rect, note: StatusNote) -> egui::Response {
    let response = ui.interact(rect, ui.id().with("status_note"), egui::Sense::click());
    let mut cursor = egui::pos2(rect.left(), rect.center().y - 11.0);
    let mut paint_chip = |text: &str, icon, color| {
        let chip = status_chip(ui, cursor, text, icon, color);
        cursor.x = chip.right() + 5.0;
    };
    match note {
        StatusNote::RateLimited(message) => paint_chip(&message, StatusIcon::Alert, STAR_GOLD),
        StatusNote::Overview { installed, updates, storage } => {
            paint_chip(&format!("{installed} installed"), StatusIcon::Library, GREEN_PLAY);
            if updates > 0 {
                paint_chip(&format!("{updates} updates"), StatusIcon::Update, STAR_GOLD);
            }
            if let Some((used, total)) = storage {
                paint_chip(&format!("{used:.1}/{total:.1} GB"), StatusIcon::Storage, TEXT_DIM);
            }
        }
    }
    response
}
fn button_hints(
    ctx: &egui::Context,
    hints: &[(Glyph, &str)],
    note: Option<StatusNote>,
    installed: Option<&InstalledIndex>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::TopBottomPanel::bottom("hints")
        .exact_height(HINT_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(glass(BG_HEADER)).inner_margin(egui::vec2(SCREEN_MARGIN, 6.0)))
        .show(ctx, |ui| {
            if let Some(note) = note {
                let rect = ui.max_rect();
                let note_rect = egui::Rect::from_min_size(
                    rect.left_top(),
                    egui::vec2(rect.width() * 0.54, rect.height()),
                );
                let response = status_note_widget(ui, note_rect, note);
                if response.clicked() {
                    let outdated = installed.map(|i| i.counts().1).unwrap_or(0);
                    if outdated > 0 {
                        commands.push(AppCommand::SetStoreTab(StoreTab::Updates));
                    } else {
                        commands.push(AppCommand::SetStoreTab(StoreTab::Library));
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (glyph, label) in hints {
                    ui.label(egui::RichText::new(*label).color(TEXT_DIM).size(FONT_SMALL));
                    ui.add_space(4.0);
                    match glyph {
                        Glyph::Shoulders => {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD);
                            ui.painter().rect_stroke(rect, RADIUS_XS, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
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
                            if let Some(texture) = button_texture(ui.ctx(), *glyph) {
                                ui.painter().image(
                                    texture.id(),
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                        }
                    }
                    ui.add_space(14.0);
                }
            });
        });
    commands
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
    ui.painter().rect_filled(rect, RADIUS_XS, ACCENT_CYAN.gamma_multiply(0.22));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, ACCENT_CYAN);
}
fn region_badge(ui: &mut egui::Ui, region: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(region.to_owned(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, RADIUS_XS, egui::Color32::from_rgb(0xea, 0x58, 0x0c).gamma_multiply(0.8));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}
fn source_badge(ui: &mut egui::Ui, label: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(label.to_uppercase(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD_HOVER);
    ui.painter().rect_stroke(rect, RADIUS_XS, egui::Stroke::new(1.0_f32, ACCENT_CYAN), egui::StrokeKind::Inside);
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
    ui.painter().rect_filled(rect, RADIUS_XS, STAR_GOLD.gamma_multiply(0.2));
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
    ui.painter().rect_filled(rect, RADIUS_XS, color.gamma_multiply(0.25));
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
    lightbox: Option<usize>,
    data_prompt: bool,
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
        commands.extend(button_hints(ctx, &[], None, None));
    } else {
        commands.extend(button_hints(
            ctx,
            &[(Glyph::Circle, lang.btn_back()), (Glyph::Cross, lang.btn_open())],
            Some(status_note(installed, icons)),
            Some(installed),
        ));
    }

    if data_prompt {
        egui::Area::new(egui::Id::new("data_prompt"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let screen = ui.ctx().screen_rect();
                let (bg, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                ui.painter().rect_filled(bg, 0.0, egui::Color32::from_black_alpha(200));
                let panel =
                    egui::Rect::from_center_size(screen.center(), egui::vec2(360.0, 210.0));
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                    egui::Frame::window(&ctx.style())
                        .fill(BG_CARD)
                        .stroke(egui::Stroke::new(1.5_f32, STAR_GOLD))
                        .corner_radius(RADIUS_LG)
                        .inner_margin(egui::vec2(24.0, 18.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(lang.data_prompt_title())
                                        .size(FONT_LARGE)
                                        .strong()
                                        .color(TEXT_WHITE),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new(lang.data_prompt_body())
                                        .size(FONT_BODY)
                                        .color(TEXT_DIM),
                                );
                                if entry.data_size_bytes > 0 {
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new(entry.data_size_label())
                                            .size(FONT_BODY)
                                            .strong()
                                            .color(STAR_GOLD),
                                    );
                                }
                                ui.add_space(18.0);
                                ui.horizontal(|ui| {
                                    if pill_button(ui, lang.data_prompt_accept(), true) {
                                        commands.push(AppCommand::InstallCurrent);
                                    }
                                    ui.add_space(10.0);
                                    if pill_button(ui, lang.cancel_btn(), false) {
                                        commands.push(AppCommand::CancelDataPrompt);
                                    }
                                });
                            });
                        });
                });
            });
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
                        .corner_radius(RADIUS_LG)
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
                                if job.is_cancellable() {
                                    ui.add_space(14.0);
                                    if cancel_button(ui, lang.cancel_btn()) {
                                        commands.push(AppCommand::CancelInstall);
                                    }
                                }
                            });
                        });
                });
        }
    }

    if let Some(shot_idx) = lightbox {
        if let Some(url) = entry.screenshot_urls.get(shot_idx) {
            egui::Area::new(egui::Id::new("screenshot_lightbox"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let screen = ui.ctx().screen_rect();
                    let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                    ui.painter().rect_filled(bg, 0.0, egui::Color32::from_black_alpha(200));
                    if bg_resp.clicked() {
                        commands.push(AppCommand::CloseScreenshot);
                    }
                    let frame = egui::Rect::from_center_size(screen.center(), egui::vec2(720.0, 405.0));
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(frame), |ui| {
                        let (rect, _) = ui.allocate_exact_size(frame.size(), egui::Sense::hover());
                        draw_screenshot(ui, icons, rect, url, entry.category, true);
                    });
                    ui.allocate_new_ui(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::pos2(screen.right() - 120.0, screen.top() + 16.0),
                            egui::vec2(100.0, 36.0),
                        )),
                        |ui| {
                            if back_button(ui, lang.back()) {
                                commands.push(AppCommand::CloseScreenshot);
                            }
                        },
                    );
                });
        }
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            if paint_hero(ui, icons, entry) {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
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
                                        commands.push(AppCommand::MoreByAuthor(entry.author.clone()));
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
                                    if entry.source_labels.is_empty() {
                                        if let Some(source) =
                                            crate::data::SourceCatalog::from_api(&entry.source_catalog)
                                        {
                                            ui.add_space(4.0);
                                            source_chip(ui, source);
                                        }
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
                            ui.add_space(8.0);
                            version_info_block(ui, lang, installed, entry, state);
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
                screenshots_row(ui, icons, entry, &mut commands);
                if !entry.changelog.trim().is_empty() {
                    ui.add_space(16.0);
                    egui::Frame::NONE
                        .fill(glass(BG_CARD))
                        .corner_radius(CARD_RADIUS)
                        .stroke(egui::Stroke::new(1.0_f32, ACCENT_CYAN.gamma_multiply(0.45)))
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            section_label(ui, lang.changelog());
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(entry.changelog.trim())
                                    .size(FONT_BODY)
                                    .color(TEXT_WHITE),
                            );
                        });
                    ui.add_space(16.0);
                }
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
                            .corner_radius(RADIUS_MD)
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
        .corner_radius(RADIUS_MD)
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
    ui.painter().rect_filled(rect, RADIUS_SM, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
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
    ui.painter().rect_filled(rect, RADIUS_SM, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
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
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 38.0), sense);
    let (fill, text_color) = match progress {
        Progress::Done => (GREEN_PLAY.gamma_multiply(0.25), GREEN_PLAY_HOVER),
        Progress::Queued => (egui::Color32::from_rgb(0x3a, 0x30, 0x14), egui::Color32::from_rgb(0xf5, 0xb8, 0x42)),
        Progress::Failed(_) => (egui::Color32::from_rgb(0x3a, 0x1c, 0x1c), egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
        _ => (BG_CARD_HOVER, TEXT_DIM),
    };
    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter().rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, ACCENT_STEAM), egui::StrokeKind::Inside);
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
fn version_info_block(
    ui: &mut egui::Ui,
    lang: Language,
    installed: &InstalledIndex,
    entry: &crate::data::AppEntry,
    state: InstallState,
) {
    let catalog_date = entry.updated_at.get(..10).unwrap_or(entry.updated_at.as_str());
    ui.vertical(|ui| {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(format!("{}: {} · {}", lang.version(), entry.version, catalog_date))
                .size(FONT_SMALL)
                .color(TEXT_DIM),
        );
        if state != InstallState::Absent {
            let info = installed.installed_info(ui.ctx(), entry).unwrap_or_default();
            let installed_line = match (&info.app_ver, &info.installed_at) {
                (Some(ver), _) => format!("{}: {ver}", lang.installed_version_value()),
                (None, Some(date)) => format!("{}: {date}", lang.installed_version_value()),
                (None, None) => format!("{}: —", lang.installed_version_value()),
            };
            ui.label(egui::RichText::new(installed_line).size(FONT_SMALL).color(TEXT_FAINT));
        }
    });
}
fn cancel_button(ui: &mut egui::Ui, label: &str) -> bool {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), TEXT_WHITE));
    let desired = egui::vec2(galley.size().x + 24.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let (fill, stroke) = if response.hovered() {
        (egui::Color32::from_rgb(0x3a, 0x1c, 0x1c), egui::Color32::from_rgb(0xff, 0x6b, 0x6b))
    } else {
        (BG_CARD_HOVER, SEPARATOR)
    };
    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter().rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font(FONT_SMALL), TEXT_WHITE);
    response.clicked()
}
fn play_install_button(ui: &mut egui::Ui, label: &str, state: InstallState) -> bool {
    let desired = egui::vec2(130.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let press_t = if response.is_pointer_button_down_on() { 1.0_f32 } else { 0.0_f32 };
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));
    let (base, hover) = match state {
        InstallState::Outdated => (STAR_GOLD, STAR_GOLD_HOVER),
        InstallState::Absent | InstallState::Installed => (BLUE_PLAY, BLUE_PLAY_HOVER),
    };
    let bg = base.lerp_to_gamma(hover, hover_t);
    ui.painter().rect_filled(rect, RADIUS_SM, bg);
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
fn screenshots_row(
    ui: &mut egui::Ui,
    icons: &IconCache,
    entry: &crate::data::AppEntry,
    commands: &mut Vec<AppCommand>,
) {
    if entry.screenshot_urls.is_empty() {
        return;
    }
    egui::ScrollArea::horizontal().id_salt("screenshots").show(ui, |ui| {
        ui.horizontal(|ui| {
            let mut load_budget = 2usize;
            for (index, url) in entry.screenshot_urls.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(SCREENSHOT_SIZE, egui::Sense::click());
                let nearby = rect.right() > ui.clip_rect().left() && rect.left() < ui.clip_rect().right();
                let already = icons.peek(url).is_some();
                let fetch = nearby && (already || load_budget > 0);
                if fetch && !already {
                    load_budget = load_budget.saturating_sub(1);
                }
                draw_screenshot(ui, icons, rect, url, entry.category, fetch);
                if response.clicked() {
                    commands.push(AppCommand::OpenScreenshot(index));
                }
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
    ui.painter().rect_filled(rect, RADIUS_MD, BG_DEEP);
    ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
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
        ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
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
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
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

fn category_color(category: Category) -> egui::Color32 {
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
fn draw_icon(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let color = category_color(entry.category);
    let corner_r = rect.width() * 0.22;
    let plate = color.lerp_to_gamma(BG_DEEP, 0.62);
    ui.painter().rect_filled(rect, corner_r, plate);
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, corner_r, TEXT_WHITE).with_texture(
                    texture.id(),
                    cover_fit_uv(texture.size_vec2(), rect),
                ),
            );
            return;
        }
    }
    let letter = entry
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let loading = art.is_some_and(|url| icons.is_loading(url, super::icons::MAX_ICON_SIDE));
    let pulse = if loading {
        let t = ui.input(|i| i.time);
        0.55 + 0.35 * ((t * 3.0).sin() as f32 * 0.5 + 0.5)
    } else {
        1.0
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font((rect.width() * 0.42).round()),
        TEXT_WHITE.gamma_multiply(pulse),
    );
    if loading {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
    }
}
