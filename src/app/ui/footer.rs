use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::theme::*;
use crate::app::ui::widgets::button_texture;
use crate::input::{AppCommand, StoreTab};
use crate::install::installed::InstalledIndex;

pub(crate) use crate::app::ui::widgets::Glyph;

pub(crate) enum StatusNote {
    RateLimited(String),
    Overview {
        installed: usize,
        updates: usize,
        storage: Option<(f64, f64)>,
        battery: Option<u8>,
        clock: String,
    },
}

pub(crate) fn status_note(installed: &InstalledIndex, icons: &IconCache) -> StatusNote {
    if let Some(left) = icons.rate_limited_for() {
        let secs = left.as_secs();
        return StatusNote::RateLimited(if secs >= 60 {
            format!("Artwork resumes in {}m {:02}s", secs / 60, secs % 60)
        } else {
            format!("Artwork resumes in {}s", secs.max(1))
        });
    }
    let (installed_count, outdated_count) = installed.counts();
    let storage = crate::app::sysinfo::storage("ux0:").map(|(used, total)| {
        let gb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        (gb(used), gb(total))
    });
    let battery = crate::app::sysinfo::battery_percent();
    let clock = crate::app::sysinfo::clock_hhmm();
    StatusNote::Overview {
        installed: installed_count,
        updates: outdated_count,
        storage,
        battery,
        clock,
    }
}

/// `reserve` is the width the button hints need; a chip that would eat into it is skipped.
fn status_note_widget(ui: &mut egui::Ui, lang: Language, note: StatusNote, reserve: f32) -> Option<StoreTab> {
    let mut clicked = None;
    match note {
        StatusNote::RateLimited(message) => {
            let _ = status_chip_inline(
                ui,
                "status_chip_rate_limited",
                &message,
                ChromeIcon::Alert,
                pal().warning,
                reserve,
            );
        }
        StatusNote::Overview {
            installed,
            updates,
            storage,
            battery,
            clock,
        } => {
            if updates > 0 {
                let label = lang.status_updates(updates);
                if status_chip_inline(
                    ui,
                    "status_chip_updates",
                    &label,
                    ChromeIcon::UpdateAvailable,
                    pal().warning,
                    reserve,
                ) {
                    clicked = Some(StoreTab::Updates);
                }
            }
            if let Some(pct) = battery {
                let color = if pct <= 15 {
                    pal().danger
                } else if pct <= 25 {
                    pal().warning
                } else {
                    pal().text_dim
                };
                let _ = status_chip_battery(ui, pct, color, reserve);
            }
            if !clock.is_empty() && clock != "--:--" {
                let _ = status_chip_inline(
                    ui,
                    "status_chip_clock",
                    &clock,
                    ChromeIcon::Recent,
                    pal().text_dim,
                    reserve,
                );
            }
            let installed_label = lang.status_installed(installed);
            if status_chip_inline(
                ui,
                "status_chip_installed",
                &installed_label,
                ChromeIcon::Installed,
                pal().success,
                reserve,
            ) {
                clicked = Some(StoreTab::Library);
            }
            if let Some((used, total)) = storage {
                let _ = status_chip_inline(
                    ui,
                    "status_chip_storage",
                    &format!("{used:.1}/{total:.1} GB"),
                    ChromeIcon::Storage,
                    pal().text_dim,
                    reserve,
                );
            }
        }
    }
    clicked
}

fn status_chip_inline(
    ui: &mut egui::Ui,
    id: &'static str,
    text: &str,
    icon: ChromeIcon,
    color: egui::Color32,
    reserve: f32,
) -> bool {
    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text.to_owned(), font(FONT_MICRO), color));
    // 8 pad + 12 icon + 4 gap + text + 8 pad = 32.
    let size = egui::vec2(galley.size().x + 32.0, 26.0);
    if size.x > ui.available_width() - reserve {
        return false;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let touch_rect = rect.expand2(egui::vec2(4.0, 6.0));
    let response = ui.interact(touch_rect, ui.id().with(id), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 13.0, color.gamma_multiply(0.13));
    ui.painter().rect_stroke(
        rect,
        13.0,
        egui::Stroke::new(1.0_f32, color.gamma_multiply(0.34)),
        egui::StrokeKind::Inside,
    );
    chrome_icons::paint(
        ui,
        icon,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 14.0, rect.center().y),
            egui::Vec2::splat(12.0),
        ),
        color,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    response.clicked()
}

fn status_chip_battery(ui: &mut egui::Ui, percent: u8, color: egui::Color32, reserve: f32) -> bool {
    let text = format!("{percent}%");
    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text, font(FONT_MICRO), color));
    let size = egui::vec2(galley.size().x + 34.0, 26.0);
    if size.x > ui.available_width() - reserve {
        return false;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let touch_rect = rect.expand2(egui::vec2(4.0, 6.0));
    let response = ui.interact(touch_rect, ui.id().with("status_chip_battery"), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 13.0, color.gamma_multiply(0.13));
    ui.painter().rect_stroke(
        rect,
        13.0,
        egui::Stroke::new(1.0_f32, color.gamma_multiply(0.34)),
        egui::StrokeKind::Inside,
    );
    let bw = 13.0_f32;
    let bh = 8.0_f32;
    let bcenter = egui::pos2(rect.left() + 14.0, rect.center().y);
    let body = egui::Rect::from_center_size(bcenter - egui::vec2(1.0, 0.0), egui::vec2(bw, bh));
    ui.painter().rect_stroke(body, 1.5, egui::Stroke::new(1.0_f32, color), egui::StrokeKind::Inside);
    let tip = egui::Rect::from_min_size(egui::pos2(body.right(), body.center().y - 1.5), egui::vec2(1.5, 3.0));
    ui.painter().rect_filled(tip, 0.5, color);
    let fill_w = ((bw - 3.0) * (percent as f32 / 100.0)).clamp(1.0, bw - 3.0);
    let fill = egui::Rect::from_min_size(body.min + egui::vec2(1.5, 1.5), egui::vec2(fill_w, bh - 3.0));
    ui.painter().rect_filled(fill, 0.5, color);

    ui.painter().galley(
        egui::pos2(rect.left() + 25.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    response.clicked()
}

pub(crate) fn button_hints(
    ctx: &egui::Context,
    lang: Language,
    hints: &[(Glyph, &str)],
    note: Option<StatusNote>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    const PAD_Y: f32 = 6.0;
    const GLYPH: f32 = 14.0;
    egui::TopBottomPanel::bottom("hints")
        .exact_height(HINT_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(pal().bg_header))
        .show(ctx, |ui| {
            let panel = ui.max_rect();
            let row = egui::Rect::from_min_max(
                egui::pos2(panel.left() + SCREEN_MARGIN, panel.top() + PAD_Y),
                egui::pos2(panel.right() - SCREEN_MARGIN, panel.bottom() - PAD_Y),
            );
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row), |ui| {
                ui.set_min_height(row.height());
                ui.spacing_mut().item_spacing.x = 5.0;

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_height(row.height());
                    if let Some(note) = note
                        && let Some(tab) = status_note_widget(ui, lang, note, hints_width(ui, hints) + 16.0)
                    {
                        commands.push(AppCommand::SetStoreTab(tab));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_min_height(row.height());
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (i, (glyph, label)) in hints.iter().enumerate().rev() {
                            let visual_i = i;
                            if visual_i == 2 || visual_i == 4 {
                                ui.add_space(5.0);
                            }
                            let galley = ui.fonts(|f| {
                                f.layout_no_wrap((*label).to_owned(), font(FONT_MICRO), pal().text_dim)
                            });
                            let (text_rect, _) = ui.allocate_exact_size(
                                egui::vec2(galley.size().x, GLYPH),
                                egui::Sense::hover(),
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    text_rect.left(),
                                    text_rect.center().y - galley.size().y * 0.5,
                                ),
                                galley,
                                pal().text_dim,
                            );
                            match glyph {
                                Glyph::Shoulders | Glyph::Start => {
                                    let text = if *glyph == Glyph::Start { "START" } else { "L / R" };
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(if *glyph == Glyph::Start { 36.0 } else { 28.0 }, GLYPH),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, RADIUS_XS, pal().bg_card);
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        text,
                                        font(FONT_MICRO),
                                        pal().text_dim,
                                    );
                                }
                                _ => {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(GLYPH, GLYPH),
                                        egui::Sense::hover(),
                                    );
                                    if let Some(texture) = button_texture(ui.ctx(), *glyph) {
                                        ui.painter().image(
                                            texture.id(),
                                            rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                }
                            }
                            ui.add_space(5.0);
                        }
                    });
                });
            });
        });
    commands
}

/// Width the right-hand hints take; mirrors the layout in `button_hints`.
fn hints_width(ui: &egui::Ui, hints: &[(Glyph, &str)]) -> f32 {
    hints
        .iter()
        .enumerate()
        .map(|(i, (glyph, label))| {
            let text = ui.fonts(|f| f.layout_no_wrap((*label).to_owned(), font(FONT_MICRO), pal().text_dim).size().x);
            let icon = match glyph {
                Glyph::Start => 36.0,
                Glyph::Shoulders => 28.0,
                _ => 14.0,
            };
            text + icon + 3.0 + 5.0 + if i == 2 || i == 4 { 5.0 } else { 0.0 }
        })
        .sum()
}
