use crate::app::i18n::Language;
use crate::app::icons::IconCache;
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

fn status_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    kind: StatusIcon,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.35_f32, color);
    match kind {
        StatusIcon::Library => {
            let rect = egui::Rect::from_center_size(center, egui::vec2(9.0, 7.0));
            painter.rect_stroke(rect, 1.5, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(rect.left(), center.y - 1.0),
                    egui::pos2(rect.right(), center.y - 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y - 1.0),
                    egui::pos2(center.x, rect.bottom()),
                ],
                stroke,
            );
        }
        StatusIcon::Update => {
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + 4.0),
                    egui::pos2(center.x, center.y - 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - 3.0, center.y - 1.0),
                    egui::pos2(center.x, center.y - 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x + 3.0, center.y - 1.0),
                    egui::pos2(center.x, center.y - 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - 4.0, center.y + 4.0),
                    egui::pos2(center.x + 4.0, center.y + 4.0),
                ],
                stroke,
            );
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
            painter.add(egui::Shape::convex_polygon(
                points.to_vec(),
                color.gamma_multiply(0.2),
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y - 2.0),
                    egui::pos2(center.x, center.y + 1.0),
                ],
                stroke,
            );
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

fn status_note_widget(ui: &mut egui::Ui, lang: Language, note: StatusNote) -> Option<StoreTab> {
    let mut clicked = None;
    match note {
        StatusNote::RateLimited(message) => {
            let _ = status_chip_inline(ui, "status_chip_rate_limited", &message, StatusIcon::Alert, STAR_GOLD);
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
                    StatusIcon::Update,
                    STAR_GOLD,
                ) {
                    clicked = Some(StoreTab::Updates);
                }
            }
            let installed_label = lang.status_installed(installed);
            if status_chip_inline(
                ui,
                "status_chip_installed",
                &installed_label,
                StatusIcon::Library,
                GREEN_PLAY,
            ) {
                clicked = Some(StoreTab::Library);
            }
            if let Some((used, total)) = storage {
                let _ = status_chip_inline(
                    ui,
                    "status_chip_storage",
                    &format!("{used:.1}/{total:.1} GB"),
                    StatusIcon::Storage,
                    TEXT_DIM,
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
    icon: StatusIcon,
    color: egui::Color32,
) -> bool {
    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text.to_owned(), font(FONT_MICRO), color));
    let size = egui::vec2(galley.size().x + 28.0, 22.0);
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
    status_icon(
        ui.painter(),
        egui::pos2(rect.left() + 11.0, rect.center().y),
        icon,
        color,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 20.0, rect.center().y - galley.size().y * 0.5),
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
        .frame(egui::Frame::NONE.fill(BG_HEADER))
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
                        && let Some(tab) = status_note_widget(ui, lang, note)
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
                                f.layout_no_wrap((*label).to_owned(), font(FONT_MICRO), TEXT_DIM)
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
                                TEXT_DIM,
                            );
                            match glyph {
                                Glyph::Shoulders => {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(32.0, GLYPH),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD);
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "L / R",
                                        font(FONT_MICRO),
                                        TEXT_DIM,
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
