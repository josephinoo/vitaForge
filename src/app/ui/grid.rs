use crate::app::icons::IconCache;
use crate::app::tile_art_url;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::{install_marker, CardResponse};
use crate::data::{Category, Platform};
use crate::input::AppCommand;
use crate::install::installed::InstalledIndex;

pub(crate) fn cover_fit_uv(texture_size: egui::Vec2, rect: egui::Rect) -> egui::Rect {
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

pub(crate) fn draw_cover(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    entry: &crate::data::AppEntry,
) {
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            let texture_size = texture.size_vec2();
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, CARD_RADIUS, pal().text)
                    .with_texture(texture.id(), cover_fit_uv(texture_size, rect)),
            );
            return;
        }
    }

    paint_cover_placeholder(
        ui,
        rect,
        entry,
        art.is_some_and(|url| icons.is_loading(url, crate::app::icons::MAX_ICON_SIDE)),
    );
}

fn paint_cover_placeholder(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    entry: &crate::data::AppEntry,
    loading: bool,
) {
    use crate::app::ui::chrome_icons::{self, ChromeIcon};

    let accent = category_color(entry.category);
    let is_plugin = matches!(entry.category, Category::Plugin)
        || matches!(entry.platform, Platform::Plugin);

    if is_plugin {
        // Material adaptive-icon vibe: saturated plate + inset squircle (not Vita LiveArea wash).
        let plate = egui::Color32::from_rgb(0x00, 0x96, 0x88); // teal-700
        let plate_hi = egui::Color32::from_rgb(0x26, 0xa6, 0x9a);
        ui.painter().rect_filled(rect, CARD_RADIUS + 2.0, plate.lerp_to_gamma(pal().bg_deep, 0.18));
        let inset = rect.shrink((rect.width().min(rect.height()) * 0.11).clamp(8.0, 16.0));
        let squircle_r = inset.width().min(inset.height()) * 0.28;
        ui.painter().rect_filled(inset, squircle_r, plate_hi);
        // Soft top highlight band (cheap “elevation”).
        let band = egui::Rect::from_min_max(
            inset.left_top(),
            egui::pos2(inset.right(), inset.top() + inset.height() * 0.38),
        );
        ui.painter().rect_filled(
            band,
            squircle_r,
            egui::Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 28),
        );
    } else {
        ui.painter().rect_filled(
            rect,
            CARD_RADIUS,
            accent.gamma_multiply(0.22).lerp_to_gamma(pal().bg_card, 0.55),
        );
        let wash = egui::Rect::from_center_size(
            rect.center() - egui::vec2(0.0, rect.height() * 0.06),
            egui::vec2(rect.width() * 0.72, rect.height() * 0.55),
        );
        ui.painter().circle_filled(
            wash.center(),
            wash.width().min(wash.height()) * 0.42,
            accent.gamma_multiply(0.18),
        );
    }

    let icon = ChromeIcon::for_category(entry.category);
    let icon_side = if is_plugin {
        (rect.width().min(rect.height()) * 0.42).clamp(26.0, 52.0)
    } else {
        (rect.width().min(rect.height()) * 0.38).clamp(18.0, 42.0)
    };
    let icon_rect = egui::Rect::from_center_size(
        rect.center() - egui::vec2(0.0, if is_plugin { rect.height() * 0.04 } else { rect.height() * 0.08 }),
        egui::vec2(icon_side, icon_side),
    );
    let tint = if loading {
        pal().text.gamma_multiply(0.55)
    } else if is_plugin {
        pal().text
    } else {
        pal().text.gamma_multiply(0.88)
    };
    chrome_icons::paint(ui, icon, icon_rect, tint);

    // Monogram under glyph (Android launcher habit) — plugins included.
    let letter = entry
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    ui.painter().text(
        egui::pos2(
            rect.center().x,
            if is_plugin {
                icon_rect.bottom() + 4.0
            } else {
                icon_rect.bottom() + 6.0
            },
        ),
        egui::Align2::CENTER_TOP,
        letter,
        font((rect.width().min(rect.height()) * if is_plugin { 0.16 } else { 0.18 }).clamp(11.0, 18.0)),
        pal().text.gamma_multiply(if loading {
            0.55
        } else if is_plugin {
            0.90
        } else {
            0.75
        }),
    );

    if loading {
        let bar_w = rect.width() * 0.42;
        let bar_h = 3.0;
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.bottom() - 14.0),
            egui::vec2(bar_w, bar_h),
        );
        ui.painter()
            .rect_filled(bar, 2.0, accent.gamma_multiply(0.25));
        let fill = egui::Rect::from_min_size(bar.left_top(), egui::vec2(bar_w * 0.6, bar_h));
        ui.painter()
            .rect_filled(fill, 2.0, accent.gamma_multiply(0.9));
    }
}

pub(crate) fn tile_label_below(ui: &mut egui::Ui, rect: egui::Rect, name: &str, focused: bool) {
    if rect.height() <= 0.0 {
        return;
    }
    let mut job =
        egui::text::LayoutJob::simple(name.to_owned(), font(FONT_SMALL), pal().text, rect.width());
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts(|f| f.layout_job(job));
    if focused && galley.elided {
        // A long title on the focused tile scrolls past instead of ending in "…".
        let line = ui.fonts(|f| f.layout_no_wrap(name.to_owned(), font(FONT_SMALL), pal().text));
        let gap = 28.0;
        let span = line.size().x + gap;
        let time = ui.input(|i| i.time) as f32;
        let travel = ((time - 0.8).max(0.0) * 32.0) % span;
        let painter = ui.painter().with_clip_rect(rect);
        let y = rect.top() + (rect.height() - line.size().y) / 2.0;
        painter.galley(
            egui::pos2(rect.left() - travel, y),
            line.clone(),
            pal().text,
        );
        painter.galley(egui::pos2(rect.left() - travel + span, y), line, pal().text);
        ui.ctx().request_repaint();
        return;
    }
    let pos = egui::pos2(rect.left(), rect.top());
    ui.painter().galley(pos, galley, pal().text);
}

pub(crate) fn tile_platform_badge(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    entry: &crate::data::AppEntry,
) {
    let text = match entry.category {
        Category::Port => "PORT",
        Category::PspGame => "PSP",
        Category::Ps1Game => "PS1",
        Category::Plugin => "PLUG",
        _ => match entry.platform {
            Platform::Vita | Platform::NpsVita => "VITA",
            Platform::Psp | Platform::NpsPsp => "PSP",
            Platform::NpsPsx => "PS1",
            Platform::Plugin => "PLUG",
        },
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font(FONT_MICRO), pal().text));
    let size = egui::vec2(galley.size().x + 10.0, 14.0);
    let badge = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 5.0, rect.bottom() - size.y - 5.0),
        size,
    );
    ui.painter().rect_filled(
        badge,
        RADIUS_XS,
        egui::Color32::from_rgba_unmultiplied(0x4a, 0x4a, 0x52, 200),
    );
    ui.painter()
        .galley(badge.center() - galley.size() / 2.0, galley, pal().text);
    if entry.data_url.is_some() || entry.data_size_bytes > 0 {
        let data_badge = egui::Rect::from_min_size(
            egui::pos2(badge.right() + 3.0, badge.top()),
            egui::vec2(size.y, size.y),
        );
        ui.painter().rect_filled(
            data_badge,
            RADIUS_XS,
            pal().accent.gamma_multiply(0.86),
        );
        crate::app::ui::chrome_icons::paint(
            ui,
            crate::app::ui::chrome_icons::ChromeIcon::GameData,
            data_badge.shrink(2.0),
            pal().text,
        );
    }
}

pub(crate) fn cover_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    entry: &crate::data::AppEntry,
    card_width: f32,
    card_height: f32,
    focused: bool,
) -> CardResponse {
    let (full_rect, response) =
        ui.allocate_exact_size(egui::vec2(card_width, card_height), egui::Sense::click());
    if !ui.is_rect_visible(full_rect) {
        return CardResponse { clicked: false };
    }
    let ctx = ui.ctx().clone();
    let anim_id = egui::Id::new(("cover_card", entry.id.as_str()));
    let press_t = ctx.animate_bool(anim_id.with("press"), response.is_pointer_button_down_on());
    let focus_t = ctx.animate_bool_with_time(anim_id.with("focus"), focused, 0.12);
    let select_inset = ((1.0 - 1.0 / SELECT_SCALE) * full_rect.width() * 0.5).clamp(0.0, 2.0);
    let rest_inset = 1.5 + press_t * PRESS_SHRINK;
    let inset = rest_inset + (select_inset - rest_inset) * focus_t;
    let rect = full_rect.shrink(inset.max(0.0));
    let image_rect = egui::Rect::from_min_size(
        rect.left_top(),
        egui::vec2(
            rect.width(),
            (rect.height() - TILE_LABEL_HEIGHT - TILE_LABEL_GAP).max(0.0),
        ),
    );
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), image_rect.bottom() + TILE_LABEL_GAP),
        rect.right_bottom(),
    );
    if focus_t > 0.01 {
        let glow = pal().focus_glow.gamma_multiply(focus_t);
        ui.painter().rect_stroke(
            image_rect.expand(2.0 * focus_t),
            CARD_RADIUS + 2.0 * focus_t,
            egui::Stroke::new(FOCUS_STROKE * focus_t, glow),
            egui::StrokeKind::Outside,
        );
    }
    draw_cover(ui, icons, image_rect, entry);
    tile_platform_badge(ui, image_rect, entry);
    install_marker(ui.painter(), image_rect, installed.state(entry));
    tile_label_below(ui, label_rect, &entry.name, focus_t > 0.5);
    if focus_t > 0.01 {
        let stroke = pal().accent_soft.gamma_multiply(0.35 + 0.65 * focus_t);
        ui.painter().rect_stroke(
            image_rect,
            CARD_RADIUS,
            egui::Stroke::new(FOCUS_STROKE * (0.5 + 0.5 * focus_t), stroke),
            egui::StrokeKind::Outside,
        );
    }
    CardResponse {
        clicked: response.clicked(),
    }
}

pub(crate) fn featured_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    entry: &crate::data::AppEntry,
    label: &str,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::click());
    ui.painter().rect_filled(rect, CARD_RADIUS, pal().bg_card);
    ui.painter().rect_stroke(
        rect,
        CARD_RADIUS,
        egui::Stroke::new(1.0_f32, pal().separator),
        egui::StrokeKind::Inside,
    );
    let art = egui::Rect::from_min_max(
        egui::pos2(rect.right() - rect.width() * 0.24, rect.top()),
        rect.right_bottom(),
    );
    draw_cover(ui, icons, art, entry);
    ui.painter().text(
        rect.left_top() + egui::vec2(14.0, 8.0),
        egui::Align2::LEFT_TOP,
        label,
        font(FONT_MICRO),
        pal().accent_soft,
    );
    ui.painter().text(
        rect.left_top() + egui::vec2(110.0, 8.0),
        egui::Align2::LEFT_TOP,
        &entry.name,
        font(FONT_TITLE),
        pal().text,
    );
    ui.painter().text(
        rect.left_top() + egui::vec2(110.0, 30.0),
        egui::Align2::LEFT_TOP,
        format!("{} · {}", entry.platform.label(), entry.author),
        font(FONT_SMALL),
        pal().text_dim,
    );
    response.clicked()
}

pub(crate) fn catalog_grid(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    apps: &[crate::data::AppEntry],
    filtered_indices: &[usize],
    selected: Option<usize>,
    scroll_to_selected: bool,
    scroll_reset: bool,
    commands: &mut Vec<AppCommand>,
) {
    let free_h = (ui.available_height() - GRID_BOTTOM_PAD).max(80.0);
    let available = (ui.available_width() - SCROLLBAR_RESERVE).max(0.0);
    let label_extra = TILE_LABEL_HEIGHT + TILE_LABEL_GAP;
    let max_card_w =
        (available - GRID_COL_SPACING * (GRID_COLUMNS as f32 - 1.0)) / GRID_COLUMNS as f32;
    let card_width = max_card_w.max(1.0);
    let natural_card_height = (card_width / COVER_ASPECT).max(1.0) + label_extra;
    // The catalog always presents two complete rows.
    let card_height = ((free_h - GRID_BOTTOM_PAD - GRID_ROW_SPACING) / 2.0)
        .max(label_extra + 1.0)
        .min(natural_card_height);
    let row_height = card_height + GRID_ROW_SPACING;
    let viewport_h = (row_height * 2.0).min(free_h);
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
        ui.set_height(row_height * total_rows as f32 + GRID_BOTTOM_PAD);

        if scroll_to_selected
            && !scroll_reset
            && let Some(cursor) = selected
        {
            let row = cursor / GRID_COLUMNS;
            let row_top = ui.max_rect().top() + row as f32 * row_height;

            let target_top = if row == 0 {
                ui.max_rect().top()
            } else {
                row_top
            };
            let row_rect = egui::Rect::from_x_y_ranges(
                ui.max_rect().x_range(),
                target_top..=(row_top + row_height),
            );
            ui.scroll_to_rect(row_rect, None);
        }
        let grid_min = viewport.min.y.max(0.0);
        let grid_max = viewport.max.y.max(0.0);
        let mut min_row = (grid_min / row_height).floor() as usize;
        let mut max_row = (grid_max / row_height).ceil() as usize;
        if max_row > total_rows {
            let diff = max_row.saturating_sub(min_row);
            max_row = total_rows;
            min_row = total_rows.saturating_sub(diff);
        }
        let y_min = ui.max_rect().top() + min_row as f32 * row_height;
        let y_max = ui.max_rect().top() + max_row as f32 * row_height;
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
                        let Some(&real_index) = filtered_indices.get(item_index) else {
                            break;
                        };
                        let Some(entry) = apps.get(real_index) else {
                            continue;
                        };
                        if let Some(delay) = tile_art_url(entry).and_then(|url| {
                            icons.repaint_delay(url, crate::app::icons::MAX_ICON_SIDE)
                        }) {
                            art_wakeup =
                                Some(art_wakeup.map_or(delay, |soonest| soonest.min(delay)));
                        }
                        let card = ui
                            .push_id((entry.platform.label(), entry.id.as_str()), |ui| {
                                cover_card(
                                    ui,
                                    icons,
                                    installed,
                                    entry,
                                    card_width,
                                    card_height,
                                    selected == Some(item_index),
                                )
                            })
                            .inner;
                        if card.clicked {
                            commands.push(AppCommand::SelectApp { index: item_index });
                        }
                        if column + 1 < GRID_COLUMNS {
                            ui.add_space(col_spacing);
                        }
                    }
                });
                ui.add_space(GRID_ROW_SPACING);
            }
            let prefetch_start = min_row.saturating_sub(1) * GRID_COLUMNS;
            let prefetch_end =
                ((max_row + 1) * GRID_COLUMNS + ART_LOOKAHEAD).min(filtered_indices.len());
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
}
