use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::ChromeIcon;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::{
    search_chip_sized, search_field, self_update_pill, source_dropdown, top_nav_button,
};
use crate::input::{AppCommand, ContentTypeGroup, StoreTab};

pub(crate) fn catalog_header(
    ui: &mut egui::Ui,
    lang: Language,
    tab: StoreTab,
    content_type_group: ContentTypeGroup,
    search_query: &str,
    search_active: bool,
    source_counts: &[(crate::data::SourceCatalog, usize)],
    total_unique_count: usize,
    source_filter: Option<crate::data::SourceCatalog>,
    self_update: Option<&crate::app::SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
    commands: &mut Vec<AppCommand>,
) {
    let show_search_field = search_active || !search_query.trim().is_empty();
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), HEADER_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

            for group in ContentTypeGroup::ALL {
                let active = tab == StoreTab::Categories && content_type_group == group;
                if top_nav_button(ui, ChromeIcon::for_group(group), lang.group_label(group), active)
                {
                    commands.push(AppCommand::SetContentTypeGroup(group));
                }
            }

            ui.add_space(6.0);

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

            if let Some(progress) = self_update_progress {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(progress.label())
                        .size(FONT_SMALL)
                        .strong()
                        .color(STAR_GOLD),
                );
            } else if let Some(info) = self_update {
                ui.add_space(6.0);
                if self_update_pill(ui, lang, &info.tag) {
                    commands.push(AppCommand::SelfUpdate);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tab != StoreTab::Categories
                    && let Some(picked) = source_dropdown(
                        ui,
                        lang,
                        total_unique_count,
                        source_counts,
                        source_filter,
                    )
                {
                    commands.push(AppCommand::SetSourceFilter(picked));
                }

                let fill = ui.available_width().clamp(110.0, 200.0);
                if show_search_field {
                    let field = search_field(
                        ui,
                        search_query,
                        lang.search_placeholder(),
                        search_active,
                    );
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
