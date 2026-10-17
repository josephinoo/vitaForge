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
use self::grid::{catalog_grid, featured_card};
use self::header::catalog_header;
use self::loading::loading_screen;
use self::settings::settings_modal;
use self::sidebar::catalog_sidebar;
use self::theme::*;
use self::widgets::*;

pub(crate) enum UpdateAllUi {
    Hidden,
    Available(usize),
    Running(String),
}

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
            uninstall_prompt,
            previous,
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
                app.navigation_locked(),
                *scroll_delta,
                comments,
                *comments_loaded,
                *comment_entry_requested,
                *lightbox,
                *data_prompt,
                *uninstall_prompt,
                app.report_notice.as_deref(),
                &previous.apps,
                app.install
                    .as_ref()
                    .filter(|job| job.app_id != entry.id)
                    .map(|job| (job.app_id.as_str(), &job.progress)),
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
    let update_all = match app.update_all_label() {
        Some(label) => UpdateAllUi::Running(label),
        None => match app.updatable_count(&catalog.apps) {
            0 => UpdateAllUi::Hidden,
            updatable => UpdateAllUi::Available(updatable),
        },
    };
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
        catalog.sort_order,
        catalog.sort_direction,
        catalog
            .selection_active
            .then_some(catalog.selected)
            .filter(|_| catalog.focus_pane == crate::app::FocusPane::Grid),
        catalog.scroll_to_selected,
        catalog.scroll_reset,
        &catalog.group_category_counts,
        catalog.group_total,
        &catalog.genre_counts,
        catalog.tab,
        catalog.shows_sidebar(),
        catalog.focus_pane == crate::app::FocusPane::Sidebar,
        catalog.sidebar_cursor,
        self_update,
        self_update_progress,
        &update_all,
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
    sort_order: SortOrder,
    sort_direction: crate::data::SortDirection,
    selected: Option<usize>,
    scroll_to_selected: bool,
    scroll_reset: bool,
    category_counts: &[(Category, usize)],
    group_total: usize,
    genre_counts: &[(String, usize)],
    tab: StoreTab,
    show_sidebar: bool,
    sidebar_focus: bool,
    sidebar_cursor: usize,
    self_update: Option<&SelfUpdateInfo>,
    self_update_progress: Option<&crate::install::Progress>,
    update_all: &UpdateAllUi,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let update_all_hint = match update_all {
        UpdateAllUi::Available(count) if tab == StoreTab::Updates => lang.update_all(*count),
        _ => String::new(),
    };
    let hints = vec![
        (Glyph::Cross, lang.btn_open()),
        (Glyph::Circle, lang.btn_back()),
        (
            Glyph::Triangle,
            if update_all_hint.is_empty() {
                lang.btn_filter()
            } else {
                update_all_hint.as_str()
            },
        ),
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
                self_update,
                self_update_progress,
                update_all,
                &mut commands,
            );
            ui.add_space(6.0);

            ui.horizontal_top(|ui| {
                if show_sidebar {
                    let focus_index = sidebar_focus.then_some(sidebar_cursor);
                    ui.allocate_ui_with_layout(
                        egui::vec2(SIDEBAR_WIDTH, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            catalog_sidebar(
                                ui,
                                lang,
                                group_total,
                                category_counts,
                                category_filter,
                                favorites_filter,
                                sort_order,
                                focus_index,
                                &mut commands,
                            );
                        },
                    );
                    ui.add_space(SIDEBAR_GAP);
                }

                ui.vertical(|ui| {
                    if tab != StoreTab::Categories
                        && content_type_group == ContentTypeGroup::Games
                    {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
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
                                },
                            );
                        });
                        ui.add_space(6.0);
                    }

                    if filtered_indices.is_empty() {
                        ui.add_space(18.0);
                        let empty = match tab {
                            StoreTab::Library => lang.library_empty(),
                            StoreTab::Updates => lang.updates_empty(),
                            _ => lang.no_results(),
                        };
                        ui.label(egui::RichText::new(empty).size(FONT_LARGE).color(TEXT_DIM));
                        if !matches!(tab, StoreTab::Library | StoreTab::Updates) {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(lang.no_results_sub())
                                    .size(FONT_BODY)
                                    .color(TEXT_FAINT),
                            );
                        }
                    } else {
                        let banner_label = if favorites_filter {
                            lang.sidebar_favorites()
                        } else if let Some(category) = category_filter {
                            lang.sidebar_platform_label(category)
                        } else {
                            match sort_order {
                                SortOrder::Downloads => lang.sidebar_downloads(),
                                SortOrder::Rating => lang.sidebar_featured(),
                                SortOrder::Recent => lang.sidebar_recent(),
                                SortOrder::Size | SortOrder::Name => lang.category_label(None),
                            }
                        };
                        if tab == StoreTab::Categories
                            && content_type_group == ContentTypeGroup::Home
                            && let Some((&real_index, _)) = filtered_indices.split_first()
                            && let Some(entry) = apps.get(real_index)
                        {
                            if featured_card(ui, icons, entry, banner_label) {
                                commands.push(AppCommand::SelectApp { index: 0 });
                            }
                            ui.add_space(10.0);
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
                            &mut commands,
                        );
                    }
                });
            });
        });
    commands
}
