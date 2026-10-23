use crate::app::i18n::Language;
use crate::app::ui::theme::*;
use crate::input::AppCommand;

// Row indices are the d-pad focus ids: 0-4 languages, 5-7 cache, 8 notifications, 9 credits.
// The tab is derived from the focused row, so no extra state is needed.
pub(crate) const SETTINGS_TABS: [&[usize]; 2] = [&[0, 1, 2, 3, 4, 8, 9], &[5, 6, 7]];

pub(crate) fn settings_tab(selected: usize) -> usize {
    SETTINGS_TABS
        .iter()
        .position(|rows| rows.contains(&selected))
        .unwrap_or(0)
}

pub(crate) fn settings_modal(
    ctx: &egui::Context,
    lang: Language,
    selected: usize,
    catalog_count: usize,
    stats: &crate::data::cache_manager::CacheStats,
    cache_notice: Option<&str>,
    install_notifications: bool,
    purge_armed: bool,
    credits: bool,
    appear: f32,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::new("settings_modal_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            ui.multiply_opacity(appear);
            let panel = egui::Rect::from_center_size(
                screen.center() + egui::vec2(0.0, 12.0 * (1.0 - appear)),
                egui::vec2(580.0, 400.0),
            );
            let backdrop = ui.interact(screen, ui.id().with("dismiss"), egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(180));
            if backdrop.clicked()
                && !backdrop
                    .interact_pointer_pos()
                    .is_some_and(|pos| panel.contains(pos))
            {
                commands.push(AppCommand::CloseSettings);
            }
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                egui::Frame::new()
                    .fill(BG_HEADER)
                    .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                    .corner_radius(egui::CornerRadius::same(RADIUS_MD as u8))
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_size(panel.size() - egui::vec2(32.0, 24.0));
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(if credits {
                                    lang.settings_credits()
                                } else {
                                    lang.settings_title()
                                })
                                .size(FONT_LARGE)
                                .strong()
                                .color(TEXT_WHITE),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close = egui::Button::new(
                                        // In credits this only steps back to settings, so say so.
                                        egui::RichText::new(if credits { lang.back() } else { "X" })
                                            .size(FONT_SMALL)
                                            .strong()
                                            .color(TEXT_WHITE),
                                    )
                                    .fill(BG_CARD)
                                    .corner_radius(RADIUS_XS);
                                    if ui.add(close.min_size(egui::vec2(32.0, 30.0))).clicked() {
                                        commands.push(AppCommand::CloseSettings);
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        if !credits {
                            let tab = settings_tab(selected);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let tab_width = (ui.available_width() - 2.0 * 22.0 - 3.0 * 6.0) / 2.0;
                                shoulder_hint(ui, "L");
                                for (index, label) in
                                    [lang.settings_general(), lang.settings_storage()]
                                        .into_iter()
                                        .enumerate()
                                {
                                    if tab_button(ui, label, index == tab, tab_width) {
                                        commands.push(AppCommand::SettingsTab(index));
                                    }
                                }
                                shoulder_hint(ui, "R");
                            });
                        }
                        ui.separator();
                        if credits {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("VitaForge")
                                    .size(FONT_LARGE)
                                    .strong()
                                    .color(TEXT_WHITE),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(lang.credits_body())
                                    .size(FONT_SMALL)
                                    .color(TEXT_DIM),
                            );
                            return;
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("settings_content")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                if settings_tab(selected) == 0 {
                                    section(ui, lang.language_label());
                                    let chips = ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                        for (index, language) in
                                            Language::ALL.into_iter().enumerate()
                                        {
                                            if chip(
                                                ui,
                                                language.label(),
                                                language == lang,
                                                selected == index,
                                            ) {
                                                commands.push(AppCommand::SetLanguage(language));
                                            }
                                        }
                                    });
                                    if selected < Language::ALL.len() {
                                        chips.response.scroll_to_me(None);
                                    }
                                    ui.add_space(4.0);
                                    ui.separator();

                                    let state = if install_notifications {
                                        lang.enabled()
                                    } else {
                                        lang.disabled()
                                    };
                                    if setting_row(
                                        ui,
                                        lang.settings_install_notifications(),
                                        selected == 8,
                                        |ui| {
                                            toggle(ui, install_notifications);
                                            ui.label(
                                                egui::RichText::new(state)
                                                    .size(FONT_MICRO)
                                                    .color(TEXT_DIM),
                                            );
                                        },
                                    ) {
                                        commands.push(AppCommand::ToggleInstallNotifications);
                                    }
                                    ui.separator();
                                    if setting_row(
                                        ui,
                                        lang.settings_credits(),
                                        selected == 9,
                                        chevron,
                                    ) {
                                        commands.push(AppCommand::OpenCredits);
                                    }
                                    ui.separator();
                                } else {
                                    let info_rows = [
                                        (
                                            lang.settings_version(),
                                            format!("VitaForge {}", env!("CARGO_PKG_VERSION")),
                                        ),
                                        (lang.settings_catalog(), format!("{catalog_count}")),
                                        (
                                            lang.settings_icon_cache(),
                                            crate::data::cache_manager::format_bytes(
                                                stats.icons_bytes,
                                            ),
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
                                            lang.settings_free_space(),
                                            crate::app::sysinfo::storage("ux0:")
                                                .map(|(free, _)| {
                                                    crate::data::cache_manager::format_bytes(free)
                                                })
                                                .unwrap_or_else(|| "-".to_owned()),
                                        ),
                                        (
                                            lang.settings_total(),
                                            crate::data::cache_manager::format_bytes(
                                                stats.total_bytes,
                                            ),
                                        ),
                                    ];
                                    for (label, value) in info_rows {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(label)
                                                    .size(FONT_MICRO)
                                                    .color(TEXT_FAINT),
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
                                    ui.add_space(4.0);
                                    ui.separator();
                                    let actions = [
                                        (
                                            5,
                                            lang.settings_clear_icons(),
                                            AppCommand::ClearIconCache,
                                        ),
                                        (
                                            6,
                                            lang.settings_clear_catalog(),
                                            AppCommand::ClearCatalogCache,
                                        ),
                                        (7, lang.settings_purge_all(), AppCommand::PurgeAllCache),
                                    ];
                                    for (index, label, command) in actions {
                                        let armed = index == 7 && purge_armed;
                                        if setting_row(ui, label, selected == index, |ui| {
                                            if armed {
                                                confirm_pill(ui, lang.settings_purge_confirm(), DANGER);
                                            } else if let Some(notice) =
                                                cache_notice.filter(|_| selected == index)
                                            {
                                                confirm_pill(ui, notice, GREEN_PLAY);
                                            }
                                        }) {
                                            commands.push(command);
                                        }
                                        ui.separator();
                                    }
                                }
                            });
                    });
            });
        });
    commands
}

fn tab_button(ui: &mut egui::Ui, label: &str, active: bool, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, RADIUS_XS, ACCENT_STEAM.gamma_multiply(0.14));
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 10.0, rect.max.y - 3.0),
                egui::vec2(rect.width() - 20.0, 2.0),
            ),
            1.0,
            ACCENT_STEAM,
        );
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(FONT_SMALL),
        if active { TEXT_WHITE } else { TEXT_FAINT },
    );
    response.clicked()
}

fn shoulder_hint(ui: &mut egui::Ui, label: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_XS, BG_CARD);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        TEXT_DIM,
    );
}

fn confirm_pill(ui: &mut egui::Ui, label: &str, fill: egui::Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(RADIUS_XS)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(FONT_SMALL)
                    .strong()
                    .color(TEXT_WHITE),
            );
        });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(title)
            .size(FONT_MICRO)
            .strong()
            .color(ACCENT_CYAN),
    );
    ui.add_space(4.0);
}

/// Label on the left, control on the right; the whole row is clickable and outlined when focused.
fn setting_row(
    ui: &mut egui::Ui,
    label: &str,
    focused: bool,
    control: impl FnOnce(&mut egui::Ui),
) -> bool {
    ui.add_space(4.0);
    let row = ui.horizontal(|ui| {
        ui.set_min_height(32.0);
        ui.label(
            egui::RichText::new(label)
                .size(FONT_SMALL)
                .color(TEXT_WHITE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    let rect = row.response.rect;
    let response = ui.interact(rect, ui.id().with(label), egui::Sense::click());
    if focused {
        ui.painter().rect_filled(
            rect.expand(3.0),
            RADIUS_XS,
            ACCENT_STEAM.gamma_multiply(0.12),
        );
        ui.painter().rect_stroke(
            rect.expand(3.0),
            RADIUS_XS,
            egui::Stroke::new(1.5_f32, ACCENT_STEAM),
            egui::StrokeKind::Outside,
        );
        response.scroll_to_me(None);
    }
    ui.add_space(4.0);
    response.clicked()
}

fn chip(ui: &mut egui::Ui, label: &str, active: bool, focused: bool) -> bool {
    let text = egui::RichText::new(label)
        .size(FONT_SMALL)
        .color(if active { TEXT_WHITE } else { TEXT_DIM });
    let fill = if active {
        ACCENT_STEAM.gamma_multiply(0.35)
    } else {
        BG_CARD
    };
    let response = ui.add(
        egui::Button::new(text)
            .fill(fill)
            .corner_radius(RADIUS_XS)
            .min_size(egui::vec2(64.0, 32.0)),
    );
    if focused {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            RADIUS_XS,
            egui::Stroke::new(1.5_f32, ACCENT_STEAM),
            egui::StrokeKind::Outside,
        );
    }
    response.clicked()
}

fn toggle(ui: &mut egui::Ui, on: bool) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::hover());
    let t = ui.ctx().animate_bool_with_time(response.id, on, 0.12);
    let radius = rect.height() / 2.0;
    ui.painter()
        .rect_filled(rect, radius, SEPARATOR.lerp_to_gamma(ACCENT_STEAM, t));
    let x = egui::lerp((rect.min.x + radius)..=(rect.max.x - radius), t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, TEXT_WHITE);
}

fn chevron(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(">").size(FONT_SMALL).color(TEXT_FAINT));
}
