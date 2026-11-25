use crate::app::icons::IconCache;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::grid::cover_fit_uv;
use crate::app::ui::theme::*;
use crate::input::AppCommand;
use crate::roms::{ROM_CONSOLES, RomEntry};

pub struct RomsView<'a> {
    pub rom_type: &'a str,
    pub genre_filter: Option<&'a str>,
    pub region_filter: Option<&'a str>,
    pub loading: bool,
    pub status: Option<&'a str>,
    pub entries: &'a [RomEntry],
    pub genre_counts: &'a [(String, usize)],
    pub region_counts: &'a [(String, usize)],
    pub selected: usize,
    pub scroll_to_selected: bool,
    pub sidebar_focus: bool,
    pub sidebar_cursor: usize,
}

pub(crate) fn roms_view(
    ui: &mut egui::Ui,
    icons: &IconCache,
    search_query: &str,
    view: RomsView<'_>,
    commands: &mut Vec<AppCommand>,
) {
    let total_h = ui.available_height().max(100.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), total_h),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);

            // Left console sidebar (matching VitaForge catalog sidebar)
            let sidebar_w = 178.0;
            egui::Frame::new()
                .fill(pal().bg_card.gamma_multiply(0.75))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(sidebar_w - 20.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                        for (i, &(code, label)) in ROM_CONSOLES.iter().enumerate() {
                            let active = view.rom_type == code;
                            let focused = view.sidebar_focus && view.sidebar_cursor == i;
                            let count = active.then_some(view.entries.len());
                            if console_sidebar_row(ui, label, count, active, focused) && !active {
                                commands.push(AppCommand::RomsSelectConsole(code.to_owned()));
                            }
                        }
                    });
                });

            // Right 5-column ROMs grid constrained strictly to available height above footer
            ui.vertical(|ui| {
                if view.scroll_to_selected {
                    icons.note_scrolling();
                }
                if !view.genre_counts.is_empty() || !view.region_counts.is_empty() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 28.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if let Some(genre) = filter_combo(ui, "roms_genre_filter", "All genres", view.genre_counts, view.genre_filter) {
                                commands.push(AppCommand::RomsSetGenre(genre));
                            }
                            if let Some(region) = filter_combo(ui, "roms_region_filter", "All regions", view.region_counts, view.region_filter) {
                                commands.push(AppCommand::RomsSetRegion(region));
                            }
                        },
                    );
                    ui.add_space(6.0);
                }

                if let Some(status) = view.status {
                    ui.horizontal(|ui| {
                        if view.loading {
                            ui.spinner();
                        }
                        ui.label(egui::RichText::new(status).size(FONT_SMALL).color(pal().text_dim));
                    });
                    ui.add_space(4.0);
                }

                let needle = search_query.trim().to_lowercase();
                let filtered: Vec<(usize, &RomEntry)> = view
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.matches(&needle, view.genre_filter, view.region_filter))
                    .collect();

                if filtered.is_empty() {
                    ui.add_space(18.0);
                    let msg = if view.loading {
                        format!("Loading {} ROMs...", view.rom_type.to_uppercase())
                    } else {
                        "No matching ROMs.".to_owned()
                    };
                    ui.label(egui::RichText::new(msg).size(FONT_LARGE).color(pal().text_dim));
                    return;
                }

                let cols = GRID_COLUMNS;
                let gap = 12.0;
                let avail_w = ui.available_width().max(200.0);
                let avail_h = ui.available_height().max(80.0);
                let card_w = ((avail_w - gap * (cols as f32 - 1.0)) / cols as f32).floor().max(80.0);
                let art_h = card_w;
                let label_h = 32.0;
                let row_h = art_h + label_h;
                let num_rows = filtered.len().div_ceil(cols);
                let badge = view.rom_type.to_uppercase();
                let selected_pos = view.selected.min(filtered.len().saturating_sub(1));

                egui::ScrollArea::vertical()
                    .max_height(avail_h.min(row_h * 2.0 + gap))
                    .auto_shrink([false, false])
                    .show_rows(ui, row_h, num_rows, |ui, row_range| {
                        ui.spacing_mut().item_spacing.y = gap;
                        for row in row_range {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(gap, 0.0);
                                let start = row * cols;
                                let end = (start + cols).min(filtered.len());
                                for (pos, &(idx, entry)) in filtered[start..end].iter().enumerate() {
                                    let grid_pos = start + pos;
                                    let is_selected = !view.sidebar_focus && grid_pos == selected_pos;
                                    let resp = rom_card(
                                        ui,
                                        icons,
                                        entry,
                                        &badge,
                                        card_w,
                                        art_h,
                                        label_h,
                                        is_selected,
                                    );
                                    if is_selected && view.scroll_to_selected {
                                        resp.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if resp.clicked() {
                                        commands.push(AppCommand::RomsOpen(idx));
                                    }
                                }
                            });
                        }
                    });
            });
        },
    );
}

/// `Some(choice)` when the user picks a row; `Some(None)` is "all".
fn filter_combo(
    ui: &mut egui::Ui,
    id: &str,
    all: &str,
    options: &[(String, usize)],
    current: Option<&str>,
) -> Option<Option<String>> {
    if options.is_empty() {
        return None;
    }
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.unwrap_or(all))
        .show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), all).clicked() {
                picked = Some(None);
            }
            for (option, count) in options {
                if ui.selectable_label(current == Some(option.as_str()), format!("{option} ({count})")).clicked() {
                    picked = Some(Some(option.clone()));
                }
            }
        });
    picked
}

fn console_sidebar_row(
    ui: &mut egui::Ui,
    label: &str,
    count: Option<usize>,
    active: bool,
    focused: bool,
) -> bool {
    let h = 34.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    if active {
        ui.painter().rect_filled(rect, 8.0, pal().primary);
    } else if focused || resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, pal().bg_card_hover);
    }
    if focused {
        ui.painter().rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    let text_color = if active || focused { egui::Color32::WHITE } else { pal().text_dim };
    let ic_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        egui::Vec2::splat(16.0),
    );
    chrome_icons::paint(ui, ChromeIcon::Games, ic_rect, text_color);
    ui.painter().text(
        egui::pos2(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_SMALL),
        text_color,
    );
    if let Some(n) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            font(FONT_MICRO),
            if active { egui::Color32::WHITE } else { pal().text_faint },
        );
    }
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn rom_card(
    ui: &mut egui::Ui,
    icons: &IconCache,
    entry: &RomEntry,
    badge: &str,
    card_w: f32,
    art_h: f32,
    label_h: f32,
    selected: bool,
) -> egui::Response {
    let (total_rect, resp) = ui.allocate_exact_size(
        egui::vec2(card_w, art_h + label_h),
        egui::Sense::click(),
    );
    let art_rect = egui::Rect::from_min_size(total_rect.min, egui::vec2(card_w, art_h));

    let mut drew_tex = false;
    if let Some(url) = entry.icon_url.as_deref() {
        if let Some(tex) = icons.get(ui.ctx(), url) {
            let tex_size = tex.size_vec2();
            ui.painter().add(
                egui::epaint::RectShape::filled(art_rect, CARD_RADIUS, egui::Color32::WHITE)
                    .with_texture(tex.id(), cover_fit_uv(tex_size, art_rect)),
            );
            drew_tex = true;
        }
    }
    if !drew_tex {
        ui.painter().rect_filled(art_rect, CARD_RADIUS, pal().bg_card);
        let initial = entry.title.chars().next().unwrap_or('?').to_ascii_uppercase().to_string();
        ui.painter().text(
            art_rect.center(),
            egui::Align2::CENTER_CENTER,
            initial,
            font(FONT_LARGE),
            pal().text_dim,
        );
    }

    let stroke = if selected {
        egui::Stroke::new(2.5, pal().primary)
    } else if resp.hovered() {
        egui::Stroke::new(2.0, pal().primary.gamma_multiply(0.7))
    } else {
        egui::Stroke::new(1.0, pal().separator)
    };
    ui.painter().rect_stroke(art_rect, CARD_RADIUS, stroke, egui::StrokeKind::Inside);

    let badge_galley = ui.fonts(|f| f.layout_no_wrap(badge.to_owned(), font(FONT_MICRO), egui::Color32::WHITE));
    let badge_rect = egui::Rect::from_min_size(
        egui::pos2(art_rect.left() + 6.0, art_rect.bottom() - badge_galley.size().y - 10.0),
        egui::vec2(badge_galley.size().x + 10.0, badge_galley.size().y + 4.0),
    );
    ui.painter().rect_filled(
        badge_rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 185),
    );
    ui.painter().galley(
        egui::pos2(badge_rect.left() + 5.0, badge_rect.top() + 2.0),
        badge_galley,
        egui::Color32::WHITE,
    );

    let title_rect = egui::Rect::from_min_size(
        egui::pos2(total_rect.left() + 2.0, art_rect.bottom() + 4.0),
        egui::vec2(card_w - 4.0, label_h - 4.0),
    );
    let mut title_ui = ui.new_child(egui::UiBuilder::new().max_rect(title_rect));
    title_ui.set_clip_rect(title_rect.intersect(ui.clip_rect()));
    title_ui.add(
        egui::Label::new(
            egui::RichText::new(&entry.title)
                .size(FONT_SMALL)
                .color(if selected || resp.hovered() { pal().text } else { pal().text_dim }),
        )
        .wrap(),
    );

    resp
}
