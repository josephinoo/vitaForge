use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::ChromeIcon;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::{search_chip_sized, search_field, top_nav_button};
use crate::input::{AppCommand, ContentTypeGroup, StoreTab};

pub(crate) fn catalog_header(
    ui: &mut egui::Ui,
    lang: Language,
    tab: StoreTab,
    content_type_group: ContentTypeGroup,
    search_query: &str,
    search_active: bool,
    update_all: &crate::app::ui::UpdateAllUi,
    commands: &mut Vec<AppCommand>,
) {
    let show_search_field = search_active || !search_query.trim().is_empty();
    let auth = crate::data::auth::current();

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), HEADER_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);

            for group in ContentTypeGroup::ALL {
                let active = tab == StoreTab::Categories && content_type_group == group;
                if top_nav_button(
                    ui,
                    ChromeIcon::for_group(group),
                    lang.group_label(group),
                    active,
                ) {
                    commands.push(AppCommand::SetContentTypeGroup(group));
                }
            }

            ui.add_space(3.0);

            if top_nav_button(
                ui,
                ChromeIcon::for_store_tab(StoreTab::Library),
                lang.tab_library(),
                tab == StoreTab::Library,
            ) {
                commands.push(AppCommand::SetStoreTab(StoreTab::Library));
            }
            if top_nav_button(
                ui,
                ChromeIcon::for_store_tab(StoreTab::Updates),
                lang.tab_updates(),
                tab == StoreTab::Updates,
            ) {
                commands.push(AppCommand::SetStoreTab(StoreTab::Updates));
            }

            if let crate::app::ui::UpdateAllUi::Running(label) = update_all {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("{} {label}", lang.update()))
                        .size(FONT_SMALL)
                        .strong()
                        .color(pal().warning),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                // User Avatar button (circular, rightmost)
                let avatar_size = egui::vec2(32.0, 32.0);
                let (avatar_rect, avatar_resp) = ui.allocate_exact_size(avatar_size, egui::Sense::click());
                let avatar_hover = avatar_resp.hovered();

                if let Some(session) = &auth {
                    let initial = session.username.chars().next().unwrap_or('U').to_ascii_uppercase().to_string();
                    let bg_color = if avatar_hover {
                        pal().primary_hover
                    } else {
                        pal().primary
                    };
                    ui.painter().circle_filled(avatar_rect.center(), 15.0, bg_color);
                    ui.painter().circle_stroke(
                        avatar_rect.center(),
                        15.0,
                        egui::Stroke::new(1.5_f32, pal().glass_edge),
                    );
                    ui.painter().text(
                        avatar_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        initial,
                        font(FONT_BODY),
                        egui::Color32::WHITE,
                    );

                    // Green online indicator dot at bottom right
                    let dot_pos = egui::pos2(avatar_rect.right() - 3.0, avatar_rect.bottom() - 3.0);
                    ui.painter().circle_filled(dot_pos, 4.5, pal().bg_header);
                    ui.painter().circle_filled(dot_pos, 3.5, pal().success);
                } else {
                    let bg_color = if avatar_hover {
                        pal().bg_card_hover
                    } else {
                        pal().bg_card
                    };
                    ui.painter().circle_filled(avatar_rect.center(), 15.0, bg_color);
                    ui.painter().circle_stroke(
                        avatar_rect.center(),
                        15.0,
                        egui::Stroke::new(1.0_f32, pal().separator),
                    );
                    let ic_rect = egui::Rect::from_center_size(avatar_rect.center(), egui::Vec2::splat(16.0));
                    crate::app::ui::chrome_icons::paint(ui, ChromeIcon::User, ic_rect, pal().text_dim);
                }

                if avatar_resp.clicked() {
                    commands.push(AppCommand::OpenSettings);
                }

                // Search field / button
                let fill = ui.available_width().clamp(90.0, 150.0);
                if show_search_field {
                    let field =
                        search_field(ui, search_query, lang.search_placeholder(), search_active);
                    if field.cleared {
                        commands.push(AppCommand::SetSearchQuery(String::new()));
                    }
                    if field.open_requested {
                        commands.push(AppCommand::RequestSearch);
                    }
                } else if search_chip_sized(ui, lang.tab_search(), Some(fill)) {
                    commands.push(AppCommand::RequestSearch);
                }
            });
        },
    );
}
