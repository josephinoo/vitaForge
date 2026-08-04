use super::i18n::Language;
use super::icons::IconCache;
use super::{App, AppState};
use crate::data::{Category, Platform, SortOrder};
use crate::input::AppCommand;
use crate::install::installed::{InstallState, InstalledIndex};

const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(0x06, 0x07, 0x0e);
const BG_GRAD_TOP: egui::Color32 = egui::Color32::from_rgb(0x12, 0x14, 0x2e);
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

/// Panels are translucent so the backdrop reads through them. This is plain
/// alpha blending over what is already drawn — no blur pass, no extra
/// geometry, so it costs nothing beyond the fill that was happening anyway.
const GLASS_ALPHA: f32 = 0.62;
/// The thin top-edge highlight that makes a translucent panel read as a pane
/// of glass rather than a flat wash.
const GLASS_EDGE: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x16, 0x18, 0x20, 0x16);

/// Every distinct font size claims its own row in the glyph atlas, and the
/// painter re-uploads the whole atlas when it grows. Seven steps instead of the
/// sixteen ad-hoc sizes this file used to carry keeps that atlas small.
const FONT_MICRO: f32 = 9.0;
const FONT_SMALL: f32 = 11.0;
const FONT_BODY: f32 = 13.0;
const FONT_LARGE: f32 = 15.0;
const FONT_TITLE: f32 = 22.0;
const FONT_HEADLINE: f32 = 28.0;
const FONT_DISPLAY: f32 = 44.0;

fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

/// Backdrop tint for a panel sitting on top of the hero image.
fn glass(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply(GLASS_ALPHA)
}

const CARD_RADIUS: f32 = 12.0;
const HINT_BAR_HEIGHT: f32 = 36.0;
const SEARCH_FIELD_WIDTH: f32 = 190.0;
const SEARCH_FIELD_HEIGHT: f32 = 30.0;
const SEARCH_CLEAR_SIZE: f32 = 18.0;

pub const GRID_COLUMNS: usize = 5;
const GRID_SPACING: f32 = 14.0;
const SCREEN_MARGIN: f32 = 18.0;

const SCROLLBAR_RESERVE: f32 = 30.0;

const PRESS_ANIM_SECS: f32 = 0.08;
const HOVER_ANIM_SECS: f32 = 0.12;
const PRESS_SHRINK: f32 = 2.5;

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
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), BG_GRAD_TOP);
    mesh.colored_vertex(rect.right_top(), BG_GRAD_TOP);
    mesh.colored_vertex(rect.right_bottom(), BG_DEEP);
    mesh.colored_vertex(rect.left_bottom(), BG_DEEP);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));

    radial_glow(
        painter,
        egui::pos2(rect.left() + rect.width() * 0.18, rect.top() + rect.height() * 0.22),
        rect.width() * 0.33,
        ACCENT_STEAM.linear_multiply(0.10),
    );
    radial_glow(
        painter,
        egui::pos2(rect.left() + rect.width() * 0.88, rect.top() + rect.height() * 0.55),
        rect.width() * 0.38,
        egui::Color32::from_rgb(0x8b, 0x5c, 0xf6).linear_multiply(0.08),
    );
}

/// Height of the hero image on the detail screen, as a fraction of the screen.
const HERO_FRACTION: f32 = 0.46;

/// Picks what to blur behind the detail screen. `background_url` is the
/// intended source, but the catalog leaves it null on effectively every entry,
/// so the first screenshot (then the cover) stands in — that is what makes the
/// banner actually appear instead of the flat gradient.
fn backdrop_url(entry: &crate::data::AppEntry) -> Option<&str> {
    entry
        .background_url
        .as_deref()
        .or_else(|| entry.screenshot_urls.first().map(String::as_str))
        .or(entry.cover_url.as_deref())
}

/// Paints the detail backdrop: gradient, then the app's own art blurred across
/// the top band, then a scrim that melts it into the page.
///
/// The blur is free. The image is decoded once at 64px (see
/// [`icons::HERO_SIDE`]) and stretched over the band with linear filtering, so
/// the whole effect is a single textured quad per frame.
///
/// Returns whether the backdrop is still loading and the frame should be redrawn.
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

    // Crop through the UVs so the art fills the band without stretching.
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

    // Bottom-up scrim, same trick as the tile labels: keeps the art bright at
    // the top and lets it dissolve into the page colour behind the content.
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

fn radial_glow(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    const SEGMENTS: u32 = 28;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    for i in 0..=SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(
            center + egui::vec2(angle.cos() * radius, angle.sin() * radius),
            egui::Color32::TRANSPARENT,
        );
    }
    for i in 0..SEGMENTS {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn build_ui(ctx: &egui::Context, app: &App) -> Vec<AppCommand> {
    match &app.state {
        AppState::Loading => loading_screen(
            ctx,
            app.lang,
            app.install.as_ref().map(|j| &j.progress),
            app.self_update.as_ref(),
            app.loading_start_time,
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
            catalog.platform_filter,
            catalog.sort_order,
            catalog.selection_active.then_some(catalog.selected),
            catalog.scroll_to_selected,
            app.self_update.as_ref(),
        ),
        AppState::Detail { app: entry, scroll_offset, comments, comments_loaded, comment_entry_requested, .. } => {
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
                *scroll_offset,
                comments,
                *comments_loaded,
                *comment_entry_requested,
            )
        }
    }
}

fn loading_screen(
    ctx: &egui::Context,
    lang: Language,
    install_progress: Option<&crate::install::Progress>,
    self_update: Option<&super::SelfUpdateInfo>,
    loading_start_time: std::time::Instant,
) -> Vec<AppCommand> {
    let elapsed = loading_start_time.elapsed().as_secs_f32();
    let progress_val = (elapsed / 4.0).min(0.98);
    let remaining_val = (4.0 - elapsed).max(0.1);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP))
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_background(ui.painter(), rect);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
                    |ui| {
                        let total_content_height = 290.0;
                        let pad_y = ((rect.height() - total_content_height) / 2.0).max(0.0);
                        ui.add_space(pad_y);

                        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                        ui.painter().rect_filled(icon_rect, 24.0, BG_CARD);
                        ui.painter().rect_stroke(
                            icon_rect,
                            24.0,
                            egui::Stroke::new(2.0_f32, ACCENT_STEAM),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            icon_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "V",
                            font(FONT_DISPLAY),
                            TEXT_WHITE,
                        );

                        ui.add_space(18.0);
                        ui.label(egui::RichText::new("VitaForge").size(FONT_HEADLINE).strong().color(TEXT_WHITE));
                        ui.add_space(8.0);

                        if let Some(progress) = install_progress {
                            let tag = self_update.map_or("", |u| u.tag.as_str());
                            ui.label(
                                egui::RichText::new(format!("Updating VitaForge {}...", tag))
                                    .color(STAR_GOLD)
                                    .size(FONT_LARGE)
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(progress.label()).color(TEXT_DIM).size(FONT_BODY));
                        } else {
                            ui.label(egui::RichText::new("Bringing catalog information from databases...").color(TEXT_DIM).size(FONT_BODY));
                            
                            ui.add_space(12.0);
                            let progress_bar = egui::ProgressBar::new(progress_val)
                                .text(format!("{:.0}%", progress_val * 100.0))
                                .animate(true);
                            ui.add_sized([220.0, 16.0], progress_bar);

                            ui.add_space(8.0);
                            let est_text = if elapsed < 4.0 {
                                format!("Estimated: {:.1}s remaining", remaining_val)
                            } else {
                                "Downloading last items...".to_owned()
                            };
                            ui.label(egui::RichText::new(est_text).color(TEXT_DIM).size(FONT_SMALL));
                        }

                        ui.add_space(16.0);
                        ui.add(egui::Spinner::new().size(24.0).color(ACCENT_STEAM));
                    },
                );
            });
        });
    ctx.request_repaint();
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
    platform_filter: Option<Platform>,
    sort_order: SortOrder,
    selected: Option<usize>,
    scroll_to_selected: bool,
    self_update: Option<&super::SelfUpdateInfo>,
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
        .frame(egui::Frame::NONE.fill(BG_DEEP).inner_margin(egui::vec2(SCREEN_MARGIN, 10.0)))
        .show(ctx, |ui| {

            paint_background(ui.painter(), ui.ctx().screen_rect());

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("VitaForge").size(FONT_TITLE).strong().color(TEXT_WHITE));
                ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(FONT_SMALL).color(TEXT_DIM));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(lang.apps_count(filtered_indices.len())).color(TEXT_FAINT).size(FONT_SMALL));

                if let Some(info) = self_update {
                    ui.add_space(12.0);
                    let btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!("UPDATE {}", info.tag))
                                .size(FONT_SMALL)
                                .strong()
                                .color(STAR_GOLD),
                        )
                        .fill(STAR_GOLD.gamma_multiply(0.2))
                        .stroke(egui::Stroke::new(1.0_f32, STAR_GOLD))
                        .corner_radius(6.0),
                    );
                    if btn.clicked() {
                        commands.push(AppCommand::SelfUpdate);
                    }
                }

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
            ui.horizontal(|ui| {
                if let Some(picked) = sort_dropdown(ui, lang, sort_order) {
                    commands.push(AppCommand::SetSortOrder(picked));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(picked) = category_dropdown(ui, lang, apps, category_filter) {
                        commands.push(AppCommand::SetCategoryFilter(picked));
                    }
                    ui.add_space(8.0);
                    if let Some(picked) = platform_dropdown(ui, lang, apps, platform_filter) {
                        commands.push(AppCommand::SetPlatformFilter(picked));
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

            let available = ui.available_width() - SCROLLBAR_RESERVE;
            let card_size = (available - GRID_SPACING * (GRID_COLUMNS as f32 - 1.0)) / GRID_COLUMNS as f32;
            let row_height = card_size + GRID_SPACING;
            let total_rows = filtered_indices.len().div_ceil(GRID_COLUMNS);
            let viewport_height = ui.available_height();

            let mut scroll_area = egui::ScrollArea::vertical().id_salt("catalog_grid");
            if scroll_to_selected && let Some(cursor) = selected {
                let item_center = (cursor / GRID_COLUMNS) as f32 * row_height + card_size / 2.0;
                scroll_area = scroll_area.vertical_scroll_offset((item_center - viewport_height / 2.0).max(0.0));
            }

            scroll_area.show_rows(ui, row_height, total_rows, |ui, row_range| {
                // Warm the next row's art before it scrolls into view. One row,
                // not two: the server rate-limits image requests, and a burst
                // large enough to trip it used to blank the grid out entirely.
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

                let mut art_pending = false;
                for grid_row in row_range {
                    ui.horizontal(|ui| {
                        for column in 0..GRID_COLUMNS {
                            let item_index = grid_row * GRID_COLUMNS + column;
                            let Some(&real_index) = filtered_indices.get(item_index) else { break };
                            let Some(entry) = apps.get(real_index) else { continue };

                            art_pending |= tile_art_url(entry)
                                .is_some_and(|url| icons.is_loading(url, super::icons::MAX_ICON_SIDE));

                            let card = ui
                                .push_id((entry.platform.label(), entry.id.as_str()), |ui| {
                                    cover_card(ui, icons, installed, entry, card_size, selected == Some(item_index))
                                })
                                .inner;

                            if card.clicked {
                                commands.push(AppCommand::SelectApp {
                                    index: item_index,
                                    origin: Some(card.rect),
                                });
                            }
                            if column + 1 < GRID_COLUMNS {
                                ui.add_space(GRID_SPACING);
                            }
                        }
                    });
                    ui.add_space(GRID_SPACING);
                }
                // One repaint request for the whole grid rather than one per
                // tile per frame, which is what this used to do.
                if art_pending {
                    ui.ctx().request_repaint();
                }
            });
        });

    commands
}

pub struct CardResponse {
    pub clicked: bool,
    pub rect: egui::Rect,
}

fn sort_icon(ui: &mut egui::Ui, rect: egui::Rect, hovered: bool) {
    let hover_t = ui.ctx().animate_bool_with_time(ui.id().with("sort_icon_hover"), hovered, HOVER_ANIM_SECS);

    ui.painter().circle_filled(rect.center(), 15.0, BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t));
    ui.painter().circle_stroke(rect.center(), 15.0, egui::Stroke::new(1.0_f32, SEPARATOR));

    let stroke = egui::Stroke::new(1.8_f32, TEXT_WHITE);
    let top = rect.center().y - 5.0;
    for (i, half) in [5.5_f32, 3.5, 1.5].iter().enumerate() {
        let y = top + i as f32 * 5.0;
        let left = rect.center().x - 6.0;
        ui.painter().line_segment([egui::pos2(left, y), egui::pos2(left + half * 2.0, y)], stroke);
    }
    ui.painter().line_segment(
        [egui::pos2(rect.right() - 8.0, rect.center().y + 1.0), egui::pos2(rect.right() - 5.0, rect.center().y + 5.0)],
        stroke,
    );
    ui.painter().line_segment(
        [egui::pos2(rect.right() - 5.0, rect.center().y + 5.0), egui::pos2(rect.right() - 2.0, rect.center().y + 1.0)],
        stroke,
    );
}

fn sort_dropdown(ui: &mut egui::Ui, lang: Language, sort_order: SortOrder) -> Option<SortOrder> {
    let popup_id = ui.make_persistent_id("sort_dropdown");
    let label = format!("{} {}", lang.sort_by_prefix(), lang.sort_label(sort_order));
    let galley = ui.fonts(|f| f.layout_no_wrap(label, font(FONT_BODY), TEXT_DIM));

    let icon_size = 30.0;
    let gap = 8.0;
    let width = icon_size + gap + galley.size().x;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, icon_size), egui::Sense::click());
    let icon_rect = egui::Rect::from_min_size(rect.left_top(), egui::vec2(icon_size, icon_size));
    sort_icon(ui, icon_rect, response.hovered());
    ui.painter().galley(
        egui::pos2(icon_rect.right() + gap, rect.center().y - galley.size().y / 2.0),
        galley,
        TEXT_DIM,
    );

    if response.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
    }

    let mut picked = None;
    egui::popup_below_widget(ui, popup_id, &response, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(190.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        for sort in SortOrder::ALL {
            if dropdown_row(ui, lang.sort_label(sort), sort == sort_order) {
                picked = Some(sort);
            }
        }
    });

    picked
}

fn platform_dropdown(
    ui: &mut egui::Ui,
    lang: Language,
    apps: &[crate::data::AppEntry],
    platform_filter: Option<Platform>,
) -> Option<Option<Platform>> {
    let popup_id = ui.make_persistent_id("platform_dropdown");
    let label = lang.platform_label(platform_filter);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), ACCENT_CYAN));
    let size = egui::vec2((galley.size().x + 34.0).max(96.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));

    let border = if open || platform_filter.is_some() { ACCENT_CYAN } else { SEPARATOR };
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
        let all_label = format!("All consoles ({})", apps.len());
        if dropdown_row(ui, &all_label, platform_filter.is_none()) {
            picked = Some(None);
        }
        for plat in Platform::ALL {
            let count = apps.iter().filter(|app| {
                match plat {
                    Platform::Vita => app.platform == Platform::Vita,
                    Platform::Psp => matches!(app.platform, Platform::Psp | Platform::NpsPsp),
                    Platform::NpsPsx => app.platform == Platform::NpsPsx,
                    Platform::NpsVita => app.platform == Platform::NpsVita,
                    Platform::Plugin => app.platform == Platform::Plugin,
                    _ => false,
                }
            }).count();
            if count == 0 {
                continue;
            }
            let row_label = format!("{} ({})", lang.platform_label(Some(plat)), count);
            if dropdown_row(ui, &row_label, platform_filter == Some(plat)) {
                picked = Some(Some(plat));
            }
        }
    });

    picked
}

fn category_dropdown(
    ui: &mut egui::Ui,
    lang: Language,
    apps: &[crate::data::AppEntry],
    category_filter: Option<Category>,
) -> Option<Option<Category>> {
    let popup_id = ui.make_persistent_id("category_dropdown");
    let label = lang.category_label(category_filter);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), ACCENT_CYAN));
    let size = egui::vec2((galley.size().x + 34.0).max(96.0), 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover_t = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let open = ui.memory(|mem| mem.is_popup_open(popup_id));

    let border = if open || category_filter.is_some() { ACCENT_CYAN } else { SEPARATOR };
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
        let all_label = format!("All categories ({})", apps.len());
        if dropdown_row(ui, &all_label, category_filter.is_none()) {
            picked = Some(None);
        }
        for category in Category::ALL {
            let count = apps.iter().filter(|app| app.category == category).count();
            if count == 0 {
                continue;
            }
            let row_label = format!("{} ({})", lang.category_label(Some(category)), count);
            if dropdown_row(ui, &row_label, category_filter == Some(category)) {
                picked = Some(Some(category));
            }
        }
    });

    picked
}

fn dropdown_row(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let hover_t = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);

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
    size: f32,
    focused: bool,
) -> CardResponse {
    let (full_rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let press_t = ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), PRESS_ANIM_SECS);
    let focus_t = ctx.animate_bool_with_time(response.id.with("focus"), focused, HOVER_ANIM_SECS);
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

    CardResponse { clicked: response.clicked(), rect: full_rect }
}

/// The one image a tile draws: the icon when there is one, the cover art
/// otherwise. The grid used to request *both* for every tile even though only
/// the icon is ever shown, doubling traffic against a rate-limited server.
fn tile_art_url(entry: &crate::data::AppEntry) -> Option<&str> {
    entry.icon_url.as_deref().or(entry.cover_url.as_deref())
}

fn draw_cover(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let color = category_color(entry.category);

    ui.painter().rect_filled(rect, CARD_RADIUS, BG_DEEP);
    ui.painter().rect_stroke(rect, CARD_RADIUS, egui::Stroke::new(1.0_f32, color.gamma_multiply(0.5)), egui::StrokeKind::Inside);

    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, CARD_RADIUS, TEXT_WHITE).with_texture(
                    texture.id(),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                ),
            );
            return;
        }
    }

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
        // No repaint request here: the grid loop already asks for one on behalf
        // of every tile whose art is still in flight.
        return;
    }

    let letter = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        // Rounded so a handful of tile widths can't spawn a handful of
        // near-identical atlas entries.
        font((rect.width() * 0.42).round()),
        TEXT_WHITE,
    );
}

fn tile_label(ui: &mut egui::Ui, rect: egui::Rect, name: &str) {
    let scrim_height = 42.0;
    let scrim = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - scrim_height),
        rect.right_bottom(),
    );
    // The scrim has to stop short of the tile's rounded bottom corners: a plain
    // gradient quad has square corners and used to poke out past the card edge.
    // So the gradient covers everything above the corner arc, and a rounded
    // rect caps the last `CARD_RADIUS` pixels at full strength.
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
        egui::Color32::from_rgb(0xf5, 0x9e, 0x0b).gamma_multiply(0.85) // NPS gets amber color
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

/// Tappable 1-5 star row for submitting a rating; returns the score if a star was clicked.
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
    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(0xea, 0x58, 0x0c).gamma_multiply(0.8)); // region badge gets a distinct orange
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

fn source_badge(ui: &mut egui::Ui, label: &str) {
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(label.to_uppercase(), font(FONT_MICRO), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(0x6d, 0x28, 0xd9).gamma_multiply(0.35)); // Purple backdrop
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0xa7, 0x8b, 0xfa).gamma_multiply(0.8)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, TEXT_WHITE);
}

/// Warning sign, drawn rather than typed. The "⚠️" this replaces is
/// `U+26A0 U+FE0F`, and the bundled fonts have no glyph for that trailing
/// variation selector — it rendered as a stray empty box.
fn warning_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4_f32, color);
    let top = center + egui::vec2(0.0, -radius);
    let left = center + egui::vec2(-radius * 0.95, radius * 0.75);
    let right = center + egui::vec2(radius * 0.95, radius * 0.75);
    painter.line_segment([top, left], stroke);
    painter.line_segment([left, right], stroke);
    painter.line_segment([right, top], stroke);
    // The exclamation mark inside.
    painter.line_segment(
        [center + egui::vec2(0.0, -radius * 0.28), center + egui::vec2(0.0, radius * 0.28)],
        stroke,
    );
    painter.circle_filled(center + egui::vec2(0.0, radius * 0.55), 0.9, color);
}

/// A warning pill that grows to fit. `requirements` routinely arrives as a
/// multi-line list ("- libshacccg.suprx\n- kubridge.skprx\n- ..."), which the
/// old fixed 16px-tall, no-wrap pill spilled straight out of.
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
    scroll_offset: f32,
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

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP).inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {

            if paint_hero(ui, icons, entry) {
                ui.ctx().request_repaint();
            }
            egui::ScrollArea::vertical()
                .id_salt(&entry.id)
                .vertical_scroll_offset(scroll_offset)
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
                                    if let Some(ref region) = entry.region {
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
                                        if play_install_button(ui, label) {
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

                // Publisher / developer / system, when the catalog scraped them.
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
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);

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
        let clear_hover =
            ctx.animate_bool_with_time(clear.id.with("hover"), clear.hovered(), HOVER_ANIM_SECS);
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
    let ctx = ui.ctx();
    let press_t = ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), PRESS_ANIM_SECS);
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
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
        ui.ctx().request_repaint();
    }

    response.clicked()
}

fn play_install_button(ui: &mut egui::Ui, label: &str) -> bool {
    let desired = egui::vec2(130.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let ctx = ui.ctx();
    let press_t = ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), PRESS_ANIM_SECS);
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));

    let bg = GREEN_PLAY.lerp_to_gamma(GREEN_PLAY_HOVER, hover_t);
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
                // Only ask the server for shots at or near the viewport. A game
                // with eight screenshots would otherwise fire eight requests the
                // moment its page opened, which is enough to hit the rate limit
                // on its own.
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
        let mut mesh = egui::Mesh::with_texture(texture.id());
        mesh.add_rect_with_uv(
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            TEXT_WHITE,
        );
        ui.painter().add(egui::Shape::mesh(mesh));
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
        ui.ctx().request_repaint();
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
        Category::Game => egui::Color32::from_rgb(0x3b, 0x82, 0xf6),
        Category::Emulator => egui::Color32::from_rgb(0xf5, 0x9e, 0x0b),
        Category::Utility => egui::Color32::from_rgb(0x10, 0xb9, 0x81),
        Category::Port => egui::Color32::from_rgb(0xa8, 0x55, 0xf7),
        Category::Plugin => egui::Color32::from_rgb(0x06, 0xb6, 0xd4),
        Category::Media => egui::Color32::from_rgb(0xec, 0x48, 0x99),
        Category::Theme => egui::Color32::from_rgb(0xea, 0xb3, 0x08),
        Category::Other => egui::Color32::from_rgb(0x64, 0x74, 0x8b),
    }
}

fn draw_icon(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, entry: &crate::data::AppEntry) {
    let color = category_color(entry.category);
    let corner_r = rect.width() * 0.22;

    ui.painter().rect_filled(rect, corner_r, BG_DEEP);
    ui.painter().rect_stroke(rect, corner_r, egui::Stroke::new(1.5_f32, color), egui::StrokeKind::Inside);

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
        ui.ctx().request_repaint();
        return;
    }

    let letter = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        // Rounded so a handful of tile widths can't spawn a handful of
        // near-identical atlas entries.
        font((rect.width() * 0.42).round()),
        TEXT_WHITE,
    );
}
