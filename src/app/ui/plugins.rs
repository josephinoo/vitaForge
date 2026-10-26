use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::theme::*;
use crate::app::ui::widgets::pill_button;
use crate::input::AppCommand;
use crate::install::tai::{Severity, TaiConfig};
use std::path::Path;

pub struct PluginsView {
    pub lang: Language,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PluginInspectorTab {
    Visual = 0,
    Raw = 1,
    Json = 2,
}

pub fn render_plugins_manager(
    ui: &mut egui::Ui,
    lang: Language,
    commands: &mut Vec<AppCommand>,
) {
    ui.spacing_mut().item_spacing.y = 10.0;

    let config = match TaiConfig::load_active() {
        Ok(c) => c,
        Err(e) => {
            ui.label(
                egui::RichText::new(format!("Error loading taiHEN config: {e:#}"))
                    .color(pal().danger),
            );
            return;
        }
    };

    let report = config.validate();

    // Check if both ux0 and ur0 exist
    let ux0_exists = Path::new("ux0:tai/config.txt").exists();
    let ur0_exists = Path::new("ur0:tai/config.txt").exists();

    if ux0_exists && ur0_exists {
        egui::Frame::NONE
            .fill(pal().warning.gamma_multiply(0.15))
            .corner_radius(CARD_RADIUS)
            .stroke(egui::Stroke::new(1.0, pal().warning))
            .inner_margin(egui::vec2(14.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(match lang {
                            Language::Spanish => "Conflicto: Existen ux0:tai y ur0:tai al mismo tiempo.",
                            _ => "Conflict: Both ux0:tai and ur0:tai exist simultaneously.",
                        })
                        .size(FONT_BODY)
                        .color(pal().warning),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(
                            ui,
                            match lang {
                                Language::Spanish => "Consolidar en ur0 y Desactivar ux0",
                                _ => "Consolidate into ur0 & Disable ux0",
                            },
                            false,
                        ) {
                            commands.push(AppCommand::ConsolidateTaiConfig);
                        }
                    });
                });
            });
        ui.add_space(4.0);
    }

    // 1. Health & Status Summary Banner
    egui::Frame::NONE
        .fill(glass(pal().bg_card))
        .corner_radius(CARD_RADIUS)
        .stroke(egui::Stroke::new(1.0, pal().glass_edge))
        .inner_margin(egui::vec2(14.0, 12.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (status_text, status_color, status_icon) = if report.has_errors() {
                    (
                        match lang {
                            Language::Spanish => "Errores detectados en config.txt",
                            _ => "Errors detected in config.txt",
                        },
                        pal().danger,
                        ChromeIcon::Alert,
                    )
                } else if !report.warnings().is_empty() {
                    (
                        match lang {
                            Language::Spanish => "Advertencias en la configuración",
                            _ => "Configuration warnings",
                        },
                        pal().warning,
                        ChromeIcon::Alert,
                    )
                } else {
                    (
                        match lang {
                            Language::Spanish => "Configuración taiHEN saludable",
                            _ => "taiHEN configuration healthy",
                        },
                        pal().success,
                        ChromeIcon::Check,
                    )
                };

                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(ui.cursor().left() + 10.0, ui.cursor().top() + 10.0),
                    egui::Vec2::splat(16.0),
                );
                chrome_icons::paint(ui, status_icon, icon_rect, status_color);
                ui.add_space(24.0);

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(status_text)
                            .size(FONT_BODY)
                            .strong()
                            .color(status_color),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}", config.path.display()))
                            .size(FONT_SMALL)
                            .color(pal().text_dim),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(
                        ui,
                        match lang {
                            Language::Spanish => "Reiniciar Seguro",
                            _ => "Safe Reboot",
                        },
                        false,
                    ) {
                        commands.push(AppCommand::SafeReboot);
                    }
                    if pill_button(
                        ui,
                        match lang {
                            Language::Spanish => "Recargar en Vivo",
                            _ => "Live Reload",
                        },
                        false,
                    ) {
                        let _ = crate::install::tai::tai_reload_config();
                    }
                    if pill_button(
                        ui,
                        match lang {
                            Language::Spanish => "Auto-ordenar *KERNEL",
                            _ => "Auto-sort *KERNEL",
                        },
                        false,
                    ) {
                        commands.push(AppCommand::AutoSortKernel);
                    }
                });
            });

            // Display any issues
            if !report.issues.is_empty() {
                ui.add_space(8.0);
                for issue in &report.issues {
                    let col = match issue.severity {
                        Severity::Error => pal().danger,
                        Severity::Warning => pal().warning,
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("•").color(col));
                        let prefix = issue.section.as_deref().map(|s| format!("[{s}] ")).unwrap_or_default();
                        ui.label(
                            egui::RichText::new(format!("{prefix}{}", issue.message))
                                .size(FONT_SMALL)
                                .color(pal().text),
                        );
                    });
                }
            }
        });

    ui.add_space(4.0);

    // Safe Mode Rescue Tip Callout
    egui::Frame::NONE
        .fill(glass(pal().bg_card))
        .corner_radius(CARD_RADIUS)
        .stroke(egui::Stroke::new(1.0, pal().glass_edge))
        .inner_margin(egui::vec2(14.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(match lang {
                        Language::Spanish => "💡 Tip de rescate: Si algún plugin de terceros causa bootloop al reiniciar, mantén presionado el gatillo L al encender la consola para arrancar en modo seguro sin plugins.",
                        _ => "💡 Rescue Tip: If a third-party plugin causes a boot loop, hold the L trigger while powering on to boot safely with plugins disabled.",
                    })
                    .size(FONT_SMALL)
                    .color(pal().text_dim),
                );
            });
        });

    ui.add_space(4.0);

    // Inspector Mode Selector
    let tab_id = egui::Id::new("plugins_inspector_tab");
    let current_tab_val: usize = ui.ctx().data_mut(|d| *d.get_temp_mut_or_default(tab_id));
    let current_tab = match current_tab_val {
        1 => PluginInspectorTab::Raw,
        2 => PluginInspectorTab::Json,
        _ => PluginInspectorTab::Visual,
    };

    ui.horizontal(|ui| {
        let is_visual = current_tab == PluginInspectorTab::Visual;
        let is_raw = current_tab == PluginInspectorTab::Raw;
        let is_json = current_tab == PluginInspectorTab::Json;

        if pill_button(ui, match lang { Language::Spanish => "Lista Visual", _ => "Visual List" }, is_visual) {
            ui.ctx().data_mut(|d| *d.get_temp_mut_or_default(tab_id) = PluginInspectorTab::Visual as usize);
        }
        if pill_button(ui, "config.txt", is_raw) {
            ui.ctx().data_mut(|d| *d.get_temp_mut_or_default(tab_id) = PluginInspectorTab::Raw as usize);
        }
        if pill_button(ui, "JSON Inspector", is_json) {
            ui.ctx().data_mut(|d| *d.get_temp_mut_or_default(tab_id) = PluginInspectorTab::Json as usize);
        }
    });

    ui.add_space(4.0);

    match current_tab {
        PluginInspectorTab::Visual => {
            // Visual Section & Plugin Cards
            for sec in &config.sections {
                egui::Frame::NONE
                    .fill(glass(pal().bg_card))
                    .corner_radius(CARD_RADIUS)
                    .stroke(egui::Stroke::new(1.0, pal().glass_edge))
                    .inner_margin(egui::vec2(14.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&sec.name)
                                    .size(FONT_LARGE)
                                    .strong()
                                    .color(pal().accent),
                            );
                            let count = sec.plugins().count();
                            ui.label(
                                egui::RichText::new(format!("({count})"))
                                    .size(FONT_SMALL)
                                    .color(pal().text_dim),
                            );
                        });

                        ui.add_space(6.0);

                        let plugins: Vec<_> = sec.plugins().collect();
                        if plugins.is_empty() {
                            ui.label(
                                egui::RichText::new(match lang {
                                    Language::Spanish => "No hay plugins en esta sección",
                                    _ => "No plugins in this section",
                                })
                                .size(FONT_SMALL)
                                .color(pal().text_dim),
                            );
                        } else {
                            for (path, enabled, comment) in plugins {
                                ui.horizontal(|ui| {
                                    let dot_color = if enabled {
                                        pal().success
                                    } else {
                                        pal().text.gamma_multiply(0.2)
                                    };
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 3.5, dot_color);

                                    let text_color = if enabled { pal().text } else { pal().text_dim };
                                    ui.label(
                                        egui::RichText::new(path)
                                            .size(FONT_BODY)
                                            .color(text_color),
                                    );

                                    if let Some(c) = comment {
                                        ui.label(
                                            egui::RichText::new(format!("#{c}"))
                                                .size(FONT_SMALL)
                                                .color(pal().text_dim),
                                        );
                                    }

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if pill_button(
                                            ui,
                                            match lang {
                                                Language::Spanish => "Eliminar",
                                                _ => "Remove",
                                            },
                                            false,
                                        ) {
                                            commands.push(AppCommand::RemovePlugin {
                                                path: path.to_string(),
                                            });
                                        }

                                        let toggle_label = if enabled {
                                            match lang {
                                                Language::Spanish => "Desactivar",
                                                _ => "Disable",
                                            }
                                        } else {
                                            match lang {
                                                Language::Spanish => "Activar",
                                                _ => "Enable",
                                            }
                                        };
                                        if pill_button(ui, toggle_label, false) {
                                            commands.push(AppCommand::TogglePlugin {
                                                path: path.to_string(),
                                                enabled: !enabled,
                                            });
                                        }
                                    });
                                });
                                ui.add_space(2.0);
                            }
                        }
                    });
                ui.add_space(4.0);
            }
        }
        PluginInspectorTab::Raw => {
            // Raw config.txt Mono Text Viewer
            let raw_text = config.to_config_txt();
            egui::Frame::NONE
                .fill(glass(pal().bg_card))
                .corner_radius(CARD_RADIUS)
                .stroke(egui::Stroke::new(1.0, pal().glass_edge))
                .inner_margin(egui::vec2(12.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} ({} lines)", config.path.display(), raw_text.lines().count()))
                            .size(FONT_SMALL)
                            .color(pal().text_dim),
                    );
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut raw_text.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                            );
                        });
                });
        }
        PluginInspectorTab::Json => {
            // JSON Inspector
            let json_text = config.to_json().unwrap_or_else(|e| format!("Error: {e}"));
            egui::Frame::NONE
                .fill(glass(pal().bg_card))
                .corner_radius(CARD_RADIUS)
                .stroke(egui::Stroke::new(1.0, pal().glass_edge))
                .inner_margin(egui::vec2(12.0, 10.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(match lang {
                            Language::Spanish => "Estructura AST serializada en JSON",
                            _ => "Serialized AST JSON Structure",
                        })
                        .size(FONT_SMALL)
                        .color(pal().text_dim),
                    );
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut json_text.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                            );
                        });
                });
        }
    }
}
