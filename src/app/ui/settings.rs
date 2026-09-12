use crate::app::i18n::Language;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::{back_button, dropdown_row};
use crate::input::AppCommand;

pub(crate) fn settings_modal(
    ctx: &egui::Context,
    lang: Language,
    selected: usize,
    catalog_count: usize,
    stats: &crate::data::cache_manager::CacheStats,
    cache_notice: Option<&str>,
    install_notifications: bool,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::new("settings_modal_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            let panel = egui::Rect::from_center_size(screen.center(), egui::vec2(650.0, 410.0));
            let backdrop = ui.interact(screen, ui.id().with("dismiss"), egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(175));
            if backdrop.clicked()
                && !backdrop
                    .interact_pointer_pos()
                    .is_some_and(|pos| panel.contains(pos))
            {
                commands.push(AppCommand::CloseSettings);
            }
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                egui::Frame::new()
                    .fill(BG_CARD)
                    .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                    .corner_radius(egui::CornerRadius::same(CARD_RADIUS as u8))
                    .inner_margin(egui::Margin::same(20))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(lang.settings_title())
                                    .size(FONT_LARGE)
                                    .strong()
                                    .color(TEXT_WHITE),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if back_button(ui, lang.back()) {
                                        commands.push(AppCommand::CloseSettings);
                                    }
                                },
                            );
                        });
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(180.0);
                                ui.label(
                                    egui::RichText::new(lang.language_label())
                                        .size(FONT_SMALL)
                                        .color(TEXT_DIM),
                                );
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
                                if dropdown_row(
                                    ui,
                                    &format!(
                                        "{}: {}",
                                        lang.settings_install_notifications(),
                                        if install_notifications {
                                            lang.enabled()
                                        } else {
                                            lang.disabled()
                                        }
                                    ),
                                    selected == 8,
                                ) {
                                    commands.push(AppCommand::ToggleInstallNotifications);
                                }
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(lang.settings_storage())
                                        .size(FONT_SMALL)
                                        .color(TEXT_DIM),
                                );
                                ui.add_space(6.0);
                                let info_rows = [
                                    (
                                        lang.settings_version(),
                                        format!("VitaForge {}", env!("CARGO_PKG_VERSION")),
                                    ),
                                    (lang.settings_catalog(), format!("{catalog_count}")),
                                    (
                                        lang.settings_icon_cache(),
                                        crate::data::cache_manager::format_bytes(stats.icons_bytes),
                                    ),
                                    (
                                        lang.settings_catalog_cache(),
                                        crate::data::cache_manager::format_bytes(
                                            stats.catalog_bytes,
                                        ),
                                    ),
                                    (
                                        lang.settings_hash_cache(),
                                        crate::data::cache_manager::format_bytes(
                                            stats.hashes_bytes,
                                        ),
                                    ),
                                    (
                                        lang.settings_total(),
                                        crate::data::cache_manager::format_bytes(stats.total_bytes),
                                    ),
                                ];
                                for (label, value) in info_rows {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(label)
                                                .size(FONT_MICRO)
                                                .color(TEXT_DIM),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(value)
                                                        .size(FONT_MICRO)
                                                        .color(TEXT_WHITE),
                                                );
                                            },
                                        );
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
                            ui.label(
                                egui::RichText::new(notice)
                                    .size(FONT_SMALL)
                                    .color(GREEN_PLAY),
                            );
                        }
                    });
            });
        });
    commands
}
