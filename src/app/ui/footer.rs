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
    StatusNote::Overview {
        installed: installed_count,
        updates: outdated_count,
        storage,
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
    // 10 pad + 12 icon + 6 gap + text + 10 pad.
    let size = egui::vec2(galley.size().x + 38.0, 22.0);
    if size.x > ui.available_width() - reserve {
        return false;
    }
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let response = ui.interact(rect, ui.id().with(id), egui::Sense::click());
    ui.painter()
        .rect_filled(rect, 11.0, color.gamma_multiply(0.13));
    ui.painter().rect_stroke(
        rect,
        11.0,
        egui::Stroke::new(1.0_f32, color.gamma_multiply(0.34)),
        egui::StrokeKind::Inside,
    );
    chrome_icons::paint(
        ui,
        icon,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            egui::Vec2::splat(12.0),
        ),
        color,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 28.0, rect.center().y - galley.size().y * 0.5),
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
                        && let Some(tab) = status_note_widget(ui, lang, note, hints_width(ui, hints) + 12.0)
                    {
                        commands.push(AppCommand::SetStoreTab(tab));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_min_height(row.height());
                        ui.spacing_mut().item_spacing.x = 5.0;
                        for (i, (glyph, label)) in hints.iter().enumerate().rev() {
                            let visual_i = i;
                            if visual_i == 2 || visual_i == 4 {
                                ui.add_space(8.0);
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
                                        egui::vec2(if *glyph == Glyph::Start { 40.0 } else { 32.0 }, GLYPH),
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
                            ui.add_space(4.0);
                        }
                    });
                });
            });
        });
    commands
}

/// Width the right-hand hints take; mirrors the layout in `button_hints`.
fn hints_width(ui: &egui::Ui, hints: &[(Glyph, &str)]) -> f32 {
    const GAP: f32 = 5.0;
    hints
        .iter()
        .enumerate()
        .map(|(i, (glyph, label))| {
            let text = ui.fonts(|f| f.layout_no_wrap((*label).to_owned(), font(FONT_MICRO), pal().text_dim).size().x);
            let icon = match glyph {
                Glyph::Start => 40.0,
                Glyph::Shoulders => 32.0,
                _ => 14.0,
            };
            text + icon + 2.0 * GAP + if i == 2 || i == 4 { 8.0 } else { 0.0 }
        })
        .sum()
}
