pub mod chrome_icons;
pub mod detail;
pub mod footer;
pub mod grid;
pub mod header;
pub mod loading;
pub mod settings;
pub mod sidebar;
pub mod theme;
pub mod widgets;

pub use theme::{apply_theme, GRID_COLUMNS};

use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::{App, AppState, CatalogState, SelfUpdateInfo, SELF_UPDATE_ID};
use crate::data::{Category, SortOrder};
use crate::input::{AppCommand, ContentTypeGroup, StoreTab};
use crate::install::installed::InstalledIndex;

use self::detail::detail_screen;
use self::footer::{button_hints, status_note};
use self::grid::catalog_grid;
use self::header::catalog_header;
use self::loading::loading_screen;
use self::settings::settings_modal;
use self::sidebar::catalog_sidebar;
use self::theme::*;
use self::widgets::*;

pub fn build_ui(ctx: &egui::Context, app: &App) -> Vec<AppCommand> {
    let self_update = app.self_update.as_ref();
    let self_update_progress = app
        .install
        .as_ref()
        .filter(|job| job.app_id == SELF_UPDATE_ID)
        .map(|job| &job.progress);
    let commands = match &app.state {
        AppState::Loading => loading_screen(
            ctx,
            app.lang,
            app.install.as_ref().map(|j| &j.progress),
            self_update,
        ),
        AppState::Catalog(catalog) => {
            catalog_view(ctx, app, catalog, self_update, self_update_progress)
        }
        AppState::Detail {
            app: entry,
            scroll_delta,
            comments,
            comments_loaded,
            comment_entry_requested,
            lightbox,
            data_prompt,
            ..
        } => {
            let progress = app
                .install
                .as_ref()
                .filter(|job| job.app_id == entry.id)
                .map(|job| &job.progress);
            detail_screen(
                ctx,
                &app.icons,
                &app.installed,
                app.lang,
                entry,
                progress,
                app.install_busy(),
                *scroll_delta,
                comments,
                *comments_loaded,
                *comment_entry_requested,
                *lightbox,
                *data_prompt,
            )
        }
        AppState::Settings { selected, previous } => {
            let mut commands = catalog_view(ctx, app, previous, self_update, self_update_progress);
            commands.extend(settings_modal(
                ctx,
                app.lang,
                *selected,
                previous.apps.len(),
                &app.cache_stats,
                app.cache_notice.as_deref(),
                app.install_notifications,
            ));
            commands
        }
    };
    app.icons.maintain(ctx);
    if app.icons.rate_limited_for().is_some() {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
    commands
}

fn catalog_view(
    ctx: &egui::Context,
    app: &App,
    catalog: &CatalogState,
    self_update: Option<&SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
) -> Vec<AppCommand> {
    if catalog.scroll_to_selected {
        app.icons.note_scrolling();
    }
    catalog_screen(
        ctx,
        &app.icons,
        &app.installed,
        app.lang,
        &catalog.apps,
        &catalog.filtered_indices,
        &catalog.search_query,
        catalog.search_requested,
        catalog.category_filter,
        catalog.content_type_group,
        catalog.favorites_filter,
        catalog.genre_filter.as_deref(),
        catalog.source_filter,
        catalog.sort_order,
        catalog.sort_direction,
        catalog.selection_active.then_some(catalog.selected).filter(|_| {
            catalog.focus_pane == crate::app::FocusPane::Grid
        }),
        catalog.scroll_to_selected,
        catalog.scroll_reset,
        &catalog.source_counts,
        catalog.total_unique_count,
        &catalog.group_category_counts,
        catalog.group_total,
        &catalog.genre_counts,
        catalog.tab,
        catalog.shows_sidebar(),
        catalog.focus_pane == crate::app::FocusPane::Sidebar,
        catalog.sidebar_cursor,
        self_update,
        self_update_progress,
    )
}

fn catalog_screen(
    ctx: &egui::Context,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    apps: &[crate::data::AppEntry],
    filtered_indices: &[usize],
    search_query: &str,
    search_active: bool,
    category_filter: Option<Category>,
    content_type_group: ContentTypeGroup,
    favorites_filter: bool,
    genre_filter: Option<&str>,
    source_filter: Option<crate::data::SourceCatalog>,
    sort_order: SortOrder,
    sort_direction: crate::data::SortDirection,
    selected: Option<usize>,
    scroll_to_selected: bool,
    scroll_reset: bool,
    source_counts: &[(crate::data::SourceCatalog, usize)],
    total_unique_count: usize,
    category_counts: &[(Category, usize)],
    group_total: usize,
    genre_counts: &[(String, usize)],
    tab: StoreTab,
    show_sidebar: bool,
    sidebar_focus: bool,
    sidebar_cursor: usize,
    self_update: Option<&SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let hints = vec![
        (Glyph::Cross, lang.btn_open()),
        (Glyph::Circle, lang.btn_back()),
        (Glyph::Triangle, lang.btn_filter()),
        (Glyph::Square, lang.btn_sort()),
        (Glyph::Shoulders, lang.btn_tabs()),
    ];
    commands.extend(button_hints(
        ctx,
        lang,
        &hints,
        Some(status_note(installed, icons)),
    ));
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::vec2(SCREEN_MARGIN, 6.0)))
        .show(ctx, |ui| {
            paint_background(ui.painter(), ui.clip_rect());

            catalog_header(
                ui,
                lang,
                tab,
                content_type_group,
                search_query,
                search_active,
                source_counts,
                total_unique_count,
                source_filter,
                self_update,
                self_update_progress,
                &mut commands,
            );
            ui.add_space(6.0);

            let sidebar_top = ui.cursor().top();
            if show_sidebar {
                let focus_index = sidebar_focus.then_some(sidebar_cursor);
                catalog_sidebar(
                    ui.ctx(),
                    ui.max_rect().left(),
                    sidebar_top,
                    lang,
                    group_total,
                    category_counts,
                    category_filter,
                    favorites_filter,
                    sort_order,
                    focus_index,
                    &mut commands,
                );
            }

            if tab == StoreTab::Categories {
                ui.add_space(2.0);
            } else {
                ui.add_space(4.0);
                if source_filter == Some(crate::data::SourceCatalog::Nps) {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(picked) =
                                genre_dropdown(ui, lang, genre_counts, genre_filter)
                            {
                                commands.push(AppCommand::SetGenreFilter(picked));
                            }
                            if let Some(picked) =
                                sort_dropdown(ui, lang, sort_order, sort_direction)
                            {
                                commands.push(AppCommand::SetSortOrder(picked));
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            }

            if filtered_indices.is_empty() {
                let content_left = if show_sidebar {
                    SIDEBAR_WIDTH + SIDEBAR_GAP
                } else {
                    0.0
                };
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(content_left);
                    ui.vertical(|ui| {
                        let empty = match tab {
                            StoreTab::Library => lang.library_empty(),
                            StoreTab::Updates => lang.updates_empty(),
                            _ => lang.no_results(),
                        };
                        ui.label(egui::RichText::new(empty).size(FONT_LARGE).color(TEXT_DIM));
                        ui.add_space(4.0);
                        if !matches!(tab, StoreTab::Library | StoreTab::Updates) {
                            ui.label(
                                egui::RichText::new(lang.no_results_sub())
                                    .size(FONT_BODY)
                                    .color(TEXT_FAINT),
                            );
                        }
                    });
                });
                return;
            }

            catalog_grid(
                ui,
                icons,
                installed,
                apps,
                filtered_indices,
                selected,
                scroll_to_selected,
                scroll_reset,
                show_sidebar,
                &mut commands,
            );
        });
    commands
}
