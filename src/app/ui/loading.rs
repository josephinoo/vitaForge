use crate::app::i18n::Language;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::self_update_pill;
use crate::input::AppCommand;

pub(crate) fn loading_screen(
    ctx: &egui::Context,
    lang: Language,
    install_progress: Option<&crate::install::Progress>,
    self_update: Option<&crate::app::SelfUpdateInfo>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_background(ui.painter(), rect);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.with_layout(
                    egui::Layout::top_down(egui::Align::Center)
                        .with_cross_align(egui::Align::Center),
                    |ui| {
                        let total_content_height = 360.0;
                        let pad_y = ((rect.height() - total_content_height) / 2.0).max(0.0);
                        ui.add_space(pad_y);
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(168.0, 168.0), egui::Sense::hover());
                        let logo = logo_texture(ui.ctx());
                        ui.painter().image(
                            logo.id(),
                            icon_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new("VitaForge")
                                .size(FONT_HEADLINE)
                                .strong()
                                .color(ACCENT_STEAM),
                        );
                        ui.label(
                            egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                .size(FONT_SMALL)
                                .color(TEXT_DIM),
                        );
                        ui.label(
                            egui::RichText::new("by josephinoo")
                                .size(FONT_SMALL)
                                .color(TEXT_FAINT),
                        );
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
                            ui.label(
                                egui::RichText::new(progress.label())
                                    .color(TEXT_DIM)
                                    .size(FONT_BODY),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(
                                    "Bringing catalog information from databases...",
                                )
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
                            let dot_angle =
                                angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
                            let pos = center
                                + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32)
                                    * radius;
                            let alpha = (i as f32 / n_dots as f32).powf(1.5);
                            ui.painter().circle_filled(
                                pos,
                                2.2,
                                ACCENT_STEAM.gamma_multiply(0.2 + 0.8 * alpha),
                            );
                        }
                    },
                );
            });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(200));
    commands
}
