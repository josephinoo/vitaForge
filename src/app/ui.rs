use super::i18n::Language;
use super::icons::IconCache;
use super::{App, AppState};
use crate::data::{Category, SortOrder};
use crate::input::AppCommand;
use crate::install::installed::{InstallState, InstalledIndex};

const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(0x0c, 0x10, 0x17); 
const BG_HEADER: egui::Color32 = egui::Color32::from_rgb(0x13, 0x1a, 0x26); 
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x23, 0x33); 
const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(0x25, 0x31, 0x47); 
const ACCENT_STEAM: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x9f, 0xff); 
const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(0x38, 0xbd, 0xf8); 
const GREEN_PLAY: egui::Color32 = egui::Color32::from_rgb(0x47, 0xa0, 0x1d); 
const GREEN_PLAY_HOVER: egui::Color32 = egui::Color32::from_rgb(0x5c, 0xba, 0x2a);
const SEPARATOR: egui::Color32 = egui::Color32::from_rgb(0x28, 0x35, 0x4d);
const TEXT_WHITE: egui::Color32 = egui::Color32::from_rgb(0xf8, 0xfa, 0xfc);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(0x94, 0xa3, 0xb8);
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(0x64, 0x74, 0x8b);
const STAR_GOLD: egui::Color32 = egui::Color32::from_rgb(0xfb, 0xbf, 0x24);

const CARD_RADIUS: f32 = 8.0;
const HINT_BAR_HEIGHT: f32 = 36.0;
const SEARCH_FIELD_WIDTH: f32 = 178.0;
const SEARCH_FIELD_HEIGHT: f32 = 30.0;
const SEARCH_CLEAR_SIZE: f32 = 18.0;

pub const GRID_COLUMNS: usize = 3;
const GRID_SPACING: f32 = 12.0;
const CARD_HEIGHT: f32 = 108.0;
const SCREEN_MARGIN: f32 = 16.0;
const SCROLLBAR_RESERVE: f32 = 14.0;

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

pub fn build_ui(ctx: &egui::Context, app: &App) -> Vec<AppCommand> {
    match &app.state {
        AppState::Loading => loading_screen(ctx, app.lang),
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
            catalog.sort_order,
            catalog.selection_active.then_some(catalog.selected),
            catalog.scroll_to_selected,
        ),
        AppState::Detail { app: entry, scroll_offset, .. } => {
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
            )
        }
    }
}

fn loading_screen(ctx: &egui::Context, lang: Language) -> Vec<AppCommand> {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP))
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
                    |ui| {
                        let total_content_height = 240.0;
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
                            egui::FontId::proportional(44.0),
                            TEXT_WHITE,
                        );

                        ui.add_space(18.0);
                        ui.label(egui::RichText::new("VitaF").size(28.0).strong().color(TEXT_WHITE));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(lang.loading_msg()).color(TEXT_DIM).size(13.5));
                        ui.add_space(24.0);
                        ui.add(egui::Spinner::new().size(32.0).color(ACCENT_STEAM));
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
    sort_order: SortOrder,
    selected: Option<usize>,
    scroll_to_selected: bool,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();

    egui::TopBottomPanel::top("header")
        .frame(egui::Frame::NONE.fill(BG_HEADER).inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(lang.discover()).size(22.0).strong().color(TEXT_WHITE));
                    ui.add_space(6.0);
                    let badge_rect = ui.allocate_exact_size(egui::vec2(64.0, 18.0), egui::Sense::hover()).0;
                    ui.painter().rect_filled(badge_rect, 4.0, BG_CARD);
                    ui.painter().text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        lang.apps_count(filtered_indices.len()),
                        egui::FontId::proportional(10.0),
                        ACCENT_CYAN,
                    );
                });

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
        });

    egui::TopBottomPanel::top("filters")
        .frame(egui::Frame::NONE.fill(BG_HEADER).inner_margin(egui::vec2(SCREEN_MARGIN, 4.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                shoulder_badge(ui, "L1");
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    if category_pill(ui, lang.category_label(None), category_filter.is_none()) {
                        commands.push(AppCommand::SetCategoryFilter(None));
                    }
                    for category in Category::ALL {
                        let active = category_filter == Some(category);
                        if category_pill(ui, lang.category_label(Some(category)), active) {
                            commands.push(AppCommand::SetCategoryFilter(if active { None } else { Some(category) }));
                        }
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    shoulder_badge(ui, "R1");
                });
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(lang.sort_by_prefix()).color(TEXT_FAINT).size(10.5).strong());
                ui.add_space(4.0);
                for sort in SortOrder::ALL {
                    let active = sort_order == sort;
                    if sort_pill(ui, lang.sort_label(sort), active) {
                        commands.push(AppCommand::SetSortOrder(sort));
                    }
                }
            });
            ui.add_space(8.0);
        });

    button_hints(
        ctx,
        &[
            (Glyph::Cross, lang.btn_open()),
            (Glyph::Circle, lang.btn_back()),
            (Glyph::Triangle, lang.btn_search()),
            (Glyph::Square, lang.btn_category()),
        ],
    );

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP).inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            if filtered_indices.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new(lang.no_results()).size(18.0).color(TEXT_DIM));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(lang.no_results_sub()).size(13.0).color(TEXT_FAINT));
                });
                return;
            }

            let available = ui.available_width() - SCROLLBAR_RESERVE;
            let card_width = (available - GRID_SPACING * (GRID_COLUMNS as f32 - 1.0)) / GRID_COLUMNS as f32;

            let show_hero = search_query.is_empty() && category_filter.is_none() && filtered_indices.len() > 1;

            let total_items = filtered_indices.len();
            let start_offset = if show_hero { 1 } else { 0 };
            let remaining_count = total_items.saturating_sub(start_offset);
            let total_rows = remaining_count.div_ceil(GRID_COLUMNS);
            let row_height = CARD_HEIGHT + GRID_SPACING;

            let cursor = selected.unwrap_or(0);
            let target_row = if show_hero {
                if cursor == 0 { 0 } else { 1 + (cursor - 1) / GRID_COLUMNS }
            } else {
                cursor / GRID_COLUMNS
            };

            let viewport_height = ui.available_height();

            let mut scroll_area = egui::ScrollArea::vertical().id_salt("catalog_grid");
            if scroll_to_selected && selected.is_some() {
                let hero_offset = if show_hero { 140.0 + 14.0 } else { 0.0 };
                let target_offset = if show_hero && cursor == 0 {
                    0.0
                } else {
                    let grid_row_idx = if show_hero { target_row.saturating_sub(1) } else { target_row };
                    let row_top = hero_offset + (grid_row_idx as f32 * row_height);
                    let item_center = row_top + CARD_HEIGHT / 2.0;
                    (item_center - viewport_height / 2.0).max(0.0)
                };
                scroll_area = scroll_area.vertical_scroll_offset(target_offset);
            }

            scroll_area.show_rows(ui, row_height, total_rows + if show_hero { 1 } else { 0 }, |ui, row_range| {
                    for grid_row in row_range {
                        if show_hero && grid_row == 0 {
                            if let Some(&first_idx) = filtered_indices.first() {
                                if let Some(featured_entry) = apps.get(first_idx) {
                                    let focused = selected == Some(0);
                                    let hero_res =
                                        hero_banner_card(ui, icons, installed, lang, featured_entry, available, focused);
                                    if hero_res.clicked {
                                        commands.push(AppCommand::SelectApp {
                                            index: 0,
                                            origin: Some(hero_res.rect),
                                        });
                                    }
                                    ui.add_space(14.0);
                                }
                            }
                            continue;
                        }

                        let actual_row = if show_hero { grid_row - 1 } else { grid_row };
                        ui.horizontal(|ui| {
                            for column in 0..GRID_COLUMNS {
                                let sub_row = actual_row * GRID_COLUMNS + column;
                                let real_selected_idx = sub_row + start_offset;
                                let Some(&real_index) = filtered_indices.get(real_selected_idx) else { break };
                                let Some(entry) = apps.get(real_index) else { continue };

                                let focused = selected == Some(real_selected_idx);
                                let card = ui
                                    .push_id(&entry.id, |ui| {
                                        app_card(ui, icons, installed, entry, card_width, focused)
                                    })
                                    .inner;

                                if card.clicked {
                                    commands.push(AppCommand::SelectApp {
                                        index: real_selected_idx,
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
                });
        });

    commands
}

pub struct CardResponse {
    pub clicked: bool,
    pub rect: egui::Rect,
}

fn shoulder_badge(ui: &mut egui::Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 22.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4.0, BG_CARD);
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), TEXT_DIM);
}

fn hero_banner_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    entry: &crate::data::AppEntry,
    width: f32,
    focused: bool,
) -> CardResponse {
    let hero_height = 140.0;
    let (full_rect, response) = ui.allocate_exact_size(egui::vec2(width, hero_height), egui::Sense::click());
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let press_t = ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), PRESS_ANIM_SECS);
    let focus_t = ctx.animate_bool_with_time(response.id.with("focus"), focused, HOVER_ANIM_SECS);
    let rect = full_rect.shrink(press_t * PRESS_SHRINK);

    let bg = BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t.max(focus_t));
    ui.painter().rect_filled(rect, CARD_RADIUS, bg);

    let border_color = if focus_t > 0.0 {
        ACCENT_CYAN
    } else {
        SEPARATOR
    };
    ui.painter().rect_stroke(
        rect,
        CARD_RADIUS,
        egui::Stroke::new(if focus_t > 0.0 { 2.0_f32 } else { 1.0_f32 }, border_color),
        egui::StrokeKind::Inside,
    );

    install_marker(&ui.painter().with_clip_rect(rect), rect, installed.state(&entry.titleid));

    let inner = rect.shrink(14.0);
    let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    content_ui.shrink_clip_rect(inner);
    content_ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let icon_rect = ui.allocate_exact_size(egui::vec2(84.0, 84.0), egui::Sense::hover()).0;
        draw_icon(ui, icons, icon_rect, entry);
        ui.add_space(14.0);

        let available_text = ui.available_width() - 120.0;
        ui.vertical(|ui| {
            ui.set_width(available_text.max(100.0));

            ui.horizontal(|ui| {
                let badge = ui.allocate_exact_size(egui::vec2(80.0, 18.0), egui::Sense::hover()).0;
                ui.painter().rect_filled(badge, 4.0, ACCENT_STEAM.gamma_multiply(0.3));
                ui.painter().text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    lang.featured(),
                    egui::FontId::proportional(10.0),
                    TEXT_WHITE,
                );
                category_badge(ui, entry.category);
            });

            ui.add_space(4.0);
            ui.add(egui::Label::new(egui::RichText::new(&entry.name).size(18.0).color(TEXT_WHITE).strong()).truncate());
            ui.add(egui::Label::new(egui::RichText::new(lang.by_author(&entry.author)).color(TEXT_DIM).size(12.0)).truncate());

            ui.add_space(4.0);
            rating_stars(ui, entry.rating);
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let btn_rect = ui.allocate_exact_size(egui::vec2(110.0, 36.0), egui::Sense::hover()).0;
            let btn_bg = if focus_t > 0.0 { GREEN_PLAY_HOVER } else { GREEN_PLAY };
            ui.painter().rect_filled(btn_rect, 6.0, btn_bg);
            ui.painter().text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                lang.view_details(),
                egui::FontId::proportional(11.5),
                TEXT_WHITE,
            );
        });
    });

    CardResponse { clicked: response.clicked(), rect: full_rect }
}

fn app_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    entry: &crate::data::AppEntry,
    width: f32,
    focused: bool,
) -> CardResponse {
    let (full_rect, response) = ui.allocate_exact_size(egui::vec2(width, CARD_HEIGHT), egui::Sense::click());
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);
    let press_t = ctx.animate_bool_with_time(response.id.with("press"), response.is_pointer_button_down_on(), PRESS_ANIM_SECS);
    let focus_t = ctx.animate_bool_with_time(response.id.with("focus"), focused, HOVER_ANIM_SECS);
    let rect = full_rect.shrink(press_t * PRESS_SHRINK);

    let bg = BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t.max(focus_t));
    ui.painter().rect_filled(rect, CARD_RADIUS, bg);

    if focus_t > 0.0 {
        ui.painter().rect_stroke(
            rect,
            CARD_RADIUS,
            egui::Stroke::new(2.0_f32, ACCENT_CYAN),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_stroke(
            rect,
            CARD_RADIUS,
            egui::Stroke::new(1.0_f32, SEPARATOR),
            egui::StrokeKind::Inside,
        );
    }

    let inner = rect.shrink(10.0);
    let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    content_ui.shrink_clip_rect(inner);
    content_ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let icon_rect = ui.allocate_exact_size(egui::vec2(58.0, 58.0), egui::Sense::hover()).0;
        draw_icon(ui, icons, icon_rect, entry);
        ui.add_space(10.0);

        let text_width = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(text_width);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(&entry.name).size(13.5).color(TEXT_WHITE).strong()).truncate());
            });
            ui.add(egui::Label::new(egui::RichText::new(&entry.author).color(TEXT_DIM).size(11.0)).truncate());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                rating_stars(ui, entry.rating);
                ui.add_space(4.0);
                category_badge(ui, entry.category);
            });
        });
    });

    install_marker(&ui.painter().with_clip_rect(rect), rect, installed.state(&entry.titleid));

    CardResponse { clicked: response.clicked(), rect: full_rect }
}

/// A corner dot, not a text badge: the name and author need the width.
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
        // An arrow, so it reads without relying on colour alone.
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
        f.layout_no_wrap(category.label_upper().to_owned(), egui::FontId::proportional(8.5), TEXT_WHITE)
    });
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;

    ui.painter().rect_filled(rect, 4.0, color.gamma_multiply(0.3));
    // Reuse the galley instead of laying the text out twice.
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

fn square_glyph(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    let half = radius * 0.55;
    let rect = egui::Rect::from_center_size(center, egui::vec2(half * 2.0, half * 2.0));
    painter.rect_stroke(rect, 1.0, egui::Stroke::new(2.0_f32, color), egui::StrokeKind::Outside);
}

enum Glyph {
    Cross,
    Circle,
    Triangle,
    Square,
}

fn button_hints(ctx: &egui::Context, hints: &[(Glyph, &str)]) {
    egui::TopBottomPanel::bottom("hints")
        .exact_height(HINT_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(BG_HEADER).inner_margin(egui::vec2(SCREEN_MARGIN, 0.0)))
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (glyph, label) in hints {
                    ui.label(egui::RichText::new(*label).color(TEXT_DIM).size(12.0));
                    ui.add_space(4.0);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    let glyph_color = match glyph {
                        Glyph::Cross => egui::Color32::from_rgb(0x38, 0xbd, 0xf8),
                        Glyph::Circle => egui::Color32::from_rgb(0xf8, 0x71, 0x71),
                        Glyph::Triangle => egui::Color32::from_rgb(0x34, 0xd3, 0x99),
                        Glyph::Square => egui::Color32::from_rgb(0xf4, 0x72, 0xb6),
                    };
                    match glyph {
                        Glyph::Cross => cross_glyph(ui.painter(), rect.center(), 8.0, glyph_color),
                        Glyph::Circle => circle_glyph(ui.painter(), rect.center(), 8.0, glyph_color),
                        Glyph::Triangle => triangle_glyph(ui.painter(), rect.center(), 8.0, glyph_color),
                        Glyph::Square => square_glyph(ui.painter(), rect.center(), 8.0, glyph_color),
                    }
                    ui.add_space(14.0);
                }
            });
        });
}

/// Two prebuilt runs instead of five label widgets per card per frame.
fn rating_stars(ui: &mut egui::Ui, rating: f32) {
    const FILLED: [&str; 6] = ["", "★", "★★", "★★★", "★★★★", "★★★★★"];
    const EMPTY: [&str; 6] = ["★★★★★", "★★★★", "★★★", "★★", "★", ""];

    let filled = ((rating + 0.5).floor().max(0.0) as usize).min(5);
    let font = egui::FontId::proportional(11.0);
    let gold = ui.fonts(|f| f.layout_no_wrap(FILLED[filled].to_owned(), font.clone(), STAR_GOLD));
    let faint = ui.fonts(|f| f.layout_no_wrap(EMPTY[filled].to_owned(), font, TEXT_FAINT));

    let size = egui::vec2(gold.size().x + faint.size().x, gold.size().y.max(faint.size().y));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

    let gold_width = gold.size().x;
    ui.painter().galley(rect.left_top(), gold, STAR_GOLD);
    ui.painter().galley(rect.left_top() + egui::vec2(gold_width, 0.0), faint, TEXT_FAINT);
}

/// A text badge for the detail page, where there is room to spell it out.
fn install_pill(ui: &mut egui::Ui, lang: Language, state: InstallState) {
    let (label, color) = match state {
        InstallState::Absent => return,
        InstallState::Installed => (lang.installed(), GREEN_PLAY),
        InstallState::Outdated => (lang.update_available(), STAR_GOLD),
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(9.5), color));
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
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let state = installed.state(&entry.titleid);

    egui::TopBottomPanel::top("detail_header")
        .frame(egui::Frame::NONE.fill(BG_HEADER).inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Hidden rather than dead: leaving would strand the progress.
                if busy {
                    ui.label(
                        egui::RichText::new(lang.install_in_progress()).size(12.5).color(STAR_GOLD),
                    );
                } else if back_button(ui, lang.back()) {
                    commands.push(AppCommand::BackToCatalog);
                }
            });
        });

    if busy {
        button_hints(ctx, &[]);
    } else {
        button_hints(ctx, &[(Glyph::Circle, lang.btn_back()), (Glyph::Cross, lang.btn_open())]);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG_DEEP).inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(&entry.id)
                .vertical_scroll_offset(scroll_offset)
                .show(ui, |ui| {
                egui::Frame::NONE
                    .fill(BG_CARD)
                    .corner_radius(12.0)
                    .stroke(egui::Stroke::new(1.0_f32, SEPARATOR))
                    .inner_margin(16.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon_rect = ui.allocate_exact_size(egui::vec2(84.0, 84.0), egui::Sense::hover()).0;
                            draw_icon(ui, icons, icon_rect, entry);
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(&entry.name).size(22.0).strong().color(TEXT_WHITE));
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(lang.by_author(&entry.author))
                                            .color(TEXT_DIM)
                                            .size(13.0),
                                    );
                                    ui.add_space(6.0);
                                    category_badge(ui, entry.category);
                                });
                                ui.add_space(4.0);
                                rating_stars(ui, entry.rating);
                                if state != InstallState::Absent {
                                    ui.add_space(5.0);
                                    install_pill(ui, lang, state);
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                match install {
                                    None => {
                                        let label = match state {
                                            InstallState::Absent => lang.install(),
                                            InstallState::Installed => lang.reinstall(),
                                            InstallState::Outdated => lang.update(),
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
                egui::Frame::NONE
                    .fill(BG_CARD.gamma_multiply(0.7))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&entry.description).size(14.0).color(TEXT_WHITE));
                    });
                ui.add_space(22.0);

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
                info_row(ui, lang.downloads(), &entry.downloads.to_string());
                info_row(ui, lang.rating(), &format!("{:.1} / 5", entry.rating));
                info_row(ui, lang.updated(), &entry.updated_at);
                info_row(ui, "Title ID", &entry.titleid);
                ui.add_space(20.0);
            });
        });

    commands
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(TEXT_DIM).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(13.0).strong().color(TEXT_WHITE));
        });
    });
    ui.add_space(6.0);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().min.y, egui::Stroke::new(1.0_f32, SEPARATOR));
    ui.add_space(6.0);
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).color(ACCENT_CYAN).size(11.5).strong());
    ui.add_space(4.0);
}

pub struct SearchFieldResponse {
    pub open_requested: bool,
    pub cleared: bool,
}

/// Painted, not a `TextEdit`. Text only ever arrives as one finished string from
/// the system keyboard, and a `TextEdit` held focus that reopened it by itself.
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
            egui::FontId::proportional(12.5),
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

fn category_pill(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let text_color = if active {
        egui::Color32::from_rgb(0x0f, 0x17, 0x2a)
    } else {
        TEXT_DIM
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(12.0), text_color));
    let size = egui::vec2(galley.size().x + 20.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);

    let bg = if active {
        TEXT_WHITE
    } else {
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t)
    };
    ui.painter().rect_filled(rect, rect.height() / 2.0, bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    ui.add_space(6.0);

    response.clicked()
}

fn sort_pill(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let text_color = if active { ACCENT_CYAN } else { TEXT_FAINT };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(11.0), text_color));
    let size = egui::vec2(galley.size().x + 16.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let ctx = ui.ctx();
    let hover_t = ctx.animate_bool_with_time(response.id.with("hover"), response.hovered(), HOVER_ANIM_SECS);

    let bg = if active {
        ACCENT_STEAM.gamma_multiply(0.35)
    } else {
        BG_CARD.lerp_to_gamma(BG_CARD_HOVER, hover_t)
    };
    ui.painter().rect_filled(rect, 6.0, bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text_color);
    ui.add_space(4.0);

    response.clicked()
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
        egui::FontId::proportional(13.5),
        TEXT_WHITE,
    );

    response.clicked()
}

fn install_status(ui: &mut egui::Ui, progress: &crate::install::Progress) -> bool {
    use crate::install::Progress;

    let finished = progress.is_finished();
    let text = progress.label();
    let galley = ui.fonts(|f| f.layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), TEXT_WHITE));
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
        egui::FontId::proportional(13.0),
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
        egui::FontId::proportional(14.0),
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
                draw_screenshot(ui, icons, rect, url, entry.category);
                ui.add_space(10.0);
            }
        });
    });
    ui.add_space(20.0);
}

fn draw_screenshot(ui: &mut egui::Ui, icons: &IconCache, rect: egui::Rect, url: &str, category: Category) {
    if let Some(texture) = icons.get_sized(ui.ctx(), url, super::icons::MAX_SCREENSHOT_SIDE) {
        let mut mesh = egui::Mesh::with_texture(texture.id());
        mesh.add_rect_with_uv(rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), TEXT_WHITE);
        ui.painter().add(egui::Shape::mesh(mesh));
        ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
        return;
    }

    ui.painter().rect_filled(rect, 8.0, BG_CARD);
    ui.painter().rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, SEPARATOR), egui::StrokeKind::Inside);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "?",
        egui::FontId::proportional(20.0),
        category_color(category).gamma_multiply(0.5),
    );
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
    if let Some(url) = entry.icon_url.as_deref() {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            mesh.add_rect_with_uv(rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), TEXT_WHITE);
            ui.painter().add(egui::Shape::mesh(mesh));
            return;
        }
        if icons.is_loading(url) {
            let color = category_color(entry.category);
            ui.painter().rect_filled(rect, rect.width() * 0.22, BG_CARD);
            ui.painter().rect_stroke(rect, rect.width() * 0.22, egui::Stroke::new(1.0_f32, color.gamma_multiply(0.2)), egui::StrokeKind::Inside);
            return;
        }
    }

    let color = category_color(entry.category);
    ui.painter().rect_filled(rect, rect.width() * 0.22, color.gamma_multiply(0.35));
    ui.painter().rect_stroke(rect, rect.width() * 0.22, egui::Stroke::new(1.0_f32, color), egui::StrokeKind::Inside);
    let letter = entry.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(rect.width() * 0.42),
        TEXT_WHITE,
    );
}
