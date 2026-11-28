pub mod chat;
pub mod chrome_icons;
pub mod detail;
pub mod footer;
pub mod grid;
pub mod header;
pub mod loading;
pub mod roms;
pub mod settings;
pub mod sidebar;
pub mod theme;
pub mod widgets;

pub use theme::{apply_theme, GRID_COLUMNS};

use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::{App, AppState, CatalogState, SELF_UPDATE_ID};
use crate::data::{Category, SortOrder};
use crate::input::{AppCommand, ContentTypeGroup, StoreTab};
use crate::install::installed::InstalledIndex;

use self::detail::detail_screen;
use self::footer::{button_hints, status_note};
use self::grid::{catalog_grid, featured_card};
use self::header::catalog_header;
use self::loading::loading_screen;
use self::settings::{settings_page, SettingsView};
use self::sidebar::catalog_sidebar;
use self::theme::*;
use self::widgets::*;

pub(crate) enum UpdateAllUi {
    Hidden,
    Available(usize),
    Running(String),
}

pub fn build_ui(ctx: &egui::Context, app: &App) -> Vec<AppCommand> {
    theme::sync_visuals(ctx);
    if let (Some(screen), Some(client)) = (&app.chat, &app.chat_client)
        && !app.self_update_prompt
    {
        return chat::chat_page(ctx, app.lang, screen, client);
    }
    let self_update = app.self_update.as_ref();
    // Fades and slides the settings modal in; snaps back to 0 while it is closed.
    let appear = ctx.animate_bool_with_time(
        egui::Id::new("settings_appear"),
        matches!(app.state, AppState::Settings { .. }),
        if matches!(app.state, AppState::Settings { .. }) { 0.15 } else { 0.0 },
    );
    let mut commands = match &app.state {
        AppState::Loading => loading_screen(
            ctx,
            app.lang,
            app.install.as_ref().map(|j| &j.progress),
            self_update,
        ),
        AppState::Catalog(catalog) => {
            catalog_view(ctx, app, catalog)
        }
        AppState::Detail {
            app: entry,
            scroll_delta,
            comments,
            comments_loaded,
            comment_entry_requested,
            lightbox,
            data_prompt,
            data_language,
            data_step,
            uninstall_prompt,
            detail_focus,
            verified_tab,
            previous,
        } => {
            let progress = app
                .install
                .as_ref()
                .filter(|job| job.app_id == entry.id)
                .map(shown_progress);
            detail_screen(
                ctx,
                &app.icons,
                &app.installed,
                app.lang,
                entry,
                progress,
                app.navigation_locked(),
                app.cancel_armed(),
                *scroll_delta,
                comments,
                *comments_loaded,
                app.verification.as_ref().filter(|(id, _)| *id == entry.id).map(|(_, v)| v),
                *comment_entry_requested,
                *lightbox,
                *data_prompt,
                *data_language,
                *data_step,
                *uninstall_prompt,
                app.report_notice.as_deref(),
                &previous.apps,
                app.install
                    .as_ref()
                    .filter(|job| job.app_id != entry.id)
                    .map(|job| (job.app_id.as_str(), &job.progress)),
                *detail_focus,
                *verified_tab,
                app.report_modal,
                app.report_choice,
                app.verify_modal.as_ref(),
                app.is_dimmed(),
            )
        }
        AppState::Settings { nav, previous } => {
            // The catalog still draws the footer hints; the page covers the rest.
            let mut commands = catalog_view(ctx, app, previous);
            commands.extend(settings_page(
                ctx,
                SettingsView {
                    lang: app.lang,
                    nav,
                    catalog_count: previous.apps.len(),
                    stats: &app.cache_stats,
                    cache_notice: app.cache_notice.as_deref(),
                    install_notifications: app.install_notifications,
                    sound_effects: app.audio.is_enabled(),
                    psp_install_target: app.psp_install_target,
                    psp_storage: app.psp_storage,
                    roms_dirs: &app.roms_dirs,
                    purge_armed: app.purge_armed,
                    plugins: settings::PluginsView {
                        tai: app.tai.as_ref(),
                        outdated: &app.tai_outdated,
                        pending_reboot: app.pending_reboot,
                        reboot_armed: app.reboot_armed,
                        armed: app.tai_armed,
                        notice: app.tai_notice.as_deref(),
                    },
                    auth_form: &app.auth_form,
                    auth_busy: app.auth_busy,
                    auth_status: app.auth_status_message.as_ref(),
                    verifier_status: app.verifier_status.as_deref(),
                    appear,
                },
            ));
            commands
        }
    };
    if let Some(picker) = app.filter_picker {
        filter_picker_modal(ctx, app, picker, &mut commands);
    }
    app.icons.maintain(ctx);
    if app.icons.rate_limited_for().is_some() {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
    if app.self_update_prompt && let Some(info) = &app.self_update {
        self_update_modal(ctx, app.lang, &info.tag, &mut commands);
    } else if let Some(job) = app.install.as_ref().filter(|job| job.app_id == SELF_UPDATE_ID) {
        crate::app::ui::detail::install_progress_modal_opt(
            ctx,
            app.lang,
            &job.title,
            shown_progress(job),
            shown_progress(job).is_cancellable(),
            app.cancel_armed(),
            app.is_dimmed(),
            &mut commands,
        );
    }
    commands
}

/// The job's progress, or "cancelling" while the worker is still winding down.
fn shown_progress(job: &crate::app::InstallJob) -> &crate::install::Progress {
    if job.progress.is_cancellable() && job.cancel.load(std::sync::atomic::Ordering::Relaxed) {
        &crate::install::Progress::Cancelling
    } else {
        &job.progress
    }
}

fn self_update_modal(
    ctx: &egui::Context,
    lang: Language,
    tag: &str,
    commands: &mut Vec<AppCommand>,
) {
    let tag = if tag.starts_with('v') || tag.starts_with('V') {
        tag.to_owned()
    } else {
        format!("v{tag}")
    };
    egui::Area::new(egui::Id::new("self_update_modal_scrim"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            let (bg, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(bg, 0.0, egui::Color32::from_black_alpha(175));
            if response.clicked() {
                commands.push(AppCommand::DismissSelfUpdate);
            }
        });
    egui::Area::new(egui::Id::new("self_update_modal"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::window(&ctx.style())
                .fill(theme::glass_modal(theme::pal().bg_card))
                .stroke(egui::Stroke::new(1.5_f32, theme::pal().accent))
                .corner_radius(theme::RADIUS_LG)
                .inner_margin(egui::vec2(28.0, 20.0))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("VitaForge")
                                .size(theme::FONT_LARGE)
                                .strong()
                                .color(theme::pal().text),
                        );
                        ui.add_space(8.0);
                        let title = format!("{} {tag}", lang.update_available());
                        ui.label(
                            egui::RichText::new(title)
                                .size(theme::FONT_BODY)
                                .strong()
                                .color(theme::pal().text),
                        );
                        ui.add_space(20.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.horizontal(|ui| {
                                if crate::app::ui::detail::prompt_chip(ui, Glyph::Cross, lang.update(), true) {
                                    commands.push(AppCommand::SelfUpdate);
                                }
                                ui.add_space(14.0);
                                if crate::app::ui::detail::prompt_chip(ui, Glyph::Circle, lang.cancel_btn(), false) {
                                    commands.push(AppCommand::DismissSelfUpdate);
                                }
                            });
                        });
                    });
                });
        });
}

fn filter_picker_modal(
    ctx: &egui::Context,
    app: &App,
    picker: crate::app::FilterPicker,
    commands: &mut Vec<AppCommand>,
) {
    use crate::app::PickerTarget;
    let lang = app.lang;
    let (options, _) = app.picker_options(picker.target);
    let region = picker.target == PickerTarget::RomRegion;
    let title = lang.tr(if region { "filter-region" } else { "filter-genre" });
    egui::Area::new(egui::Id::new("filter_picker_scrim"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            let (bg, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(bg, 0.0, egui::Color32::from_black_alpha(175));
            if response.clicked() {
                commands.push(AppCommand::CloseFilterPicker);
            }
        });
    egui::Area::new(egui::Id::new("filter_picker"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::window(&ctx.style())
                .fill(theme::glass_modal(pal().bg_card))
                .stroke(egui::Stroke::new(1.5_f32, pal().accent_soft))
                .corner_radius(RADIUS_LG)
                .inner_margin(egui::vec2(16.0, 12.0))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    ui.label(egui::RichText::new(title).size(FONT_LARGE).strong().color(pal().text));
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(344.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for row in 0..=options.len() {
                            let label = match row.checked_sub(1).map(|i| &options[i]) {
                                None if region => lang.tr("filter-all-regions"),
                                None => lang.all_genres_label(),
                                Some((name, count)) if region => format!("{name} ({count})"),
                                Some((name, count)) => format!("{} ({count})", lang.genre_label(name)),
                            };
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 41.0),
                                egui::Sense::click(),
                            );
                            let focused = row == picker.cursor;
                            if focused {
                                ui.painter().rect_filled(rect, RADIUS_SM, pal().primary);
                                crate::app::ui::widgets::scroll_on_focus_arrival(ui, &response);
                            }
                            ui.painter().text(
                                rect.left_center() + egui::vec2(12.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                label,
                                font(FONT_BODY),
                                if focused { egui::Color32::WHITE } else { pal().text },
                            );
                            if response.clicked() {
                                commands.push(AppCommand::PickFilter(row));
                            }
                        }
                    });
                });
        });
}

fn catalog_view(
    ctx: &egui::Context,
    app: &App,
    catalog: &CatalogState,
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
        &update_all,
        matches!(app.state, crate::app::AppState::Settings { .. }),
        app.self_update_prompt,
        app.filter_picker.is_some(),
        app.roms.view(),
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
    update_all: &UpdateAllUi,
    settings_open: bool,
    self_update_prompt: bool,
    picking: bool,
    roms_view_data: self::roms::RomsView<'_>,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let update_all_hint = match update_all {
        UpdateAllUi::Available(count) if tab == StoreTab::Updates => lang.update_all(*count),
        _ => String::new(),
    };
    // ○ only does something here when there is a sidebar focus, search or filter to leave.
    let can_go_back = sidebar_focus
        || search_active
        || !search_query.is_empty()
        || (tab == StoreTab::Categories && (category_filter.is_some() || favorites_filter));
    let roms_genre = lang.tr("filter-genre");
    let roms_region = lang.tr("filter-region");
    let retry = lang.tr("roms-retry");
    let mut hints = if picking {
        vec![(Glyph::Cross, lang.btn_choose()), (Glyph::Circle, lang.btn_back())]
    } else if self_update_prompt {
        vec![
            (Glyph::Cross, lang.update()),
            (Glyph::Circle, lang.btn_back()),
        ]
    } else if settings_open {
        vec![
            (Glyph::Cross, lang.btn_choose()),
            (Glyph::Circle, lang.btn_back()),
            (Glyph::Shoulders, lang.btn_tabs()),
        ]
    } else {
        vec![
            (
                Glyph::Cross,
                if tab == StoreTab::Roms && roms_view_data.failed { retry.as_str() } else { lang.btn_open() },
            ),
            (Glyph::Circle, lang.btn_back()),
            (
                Glyph::Triangle,
                if update_all_hint.is_empty() {
                    if tab == StoreTab::Roms { roms_genre.as_str() } else { lang.btn_filter() }
                } else {
                    update_all_hint.as_str()
                },
            ),
            (Glyph::Square, if tab == StoreTab::Roms { roms_region.as_str() } else { lang.btn_sort() }),
            (Glyph::Shoulders, lang.btn_tabs()),
            (Glyph::Start, lang.btn_settings()),
        ]
    };
    if !picking && !self_update_prompt && !settings_open && !can_go_back {
        hints.retain(|(glyph, _)| *glyph != Glyph::Circle);
    }
    commands.extend(button_hints(
        ctx,
        lang,
        &hints,
        Some(status_note(installed, icons)),
    ));
    // Settings fills the space above the footer itself; the catalog body would only sit under it.
    if settings_open {
        return commands;
    }
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
                    if tab == StoreTab::Categories
                        && !favorites_filter
                        && let Some(current) = category_filter.filter(|c| c.is_ps())
                    {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for category in std::iter::once(Category::PsGames).chain(Category::PS) {
                                let label = if category == Category::PsGames {
                                    lang.category_label(None)
                                } else {
                                    lang.sidebar_platform_label(category)
                                };
                                if platform_pill(ui, label, current == category) {
                                    commands.push(AppCommand::SetCategoryFilter(Some(category)));
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                    if tab == StoreTab::Categories
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
                        ui.add_space(4.0);
                    }

                    if tab == StoreTab::Roms {
                        self::roms::roms_view(ui, lang, icons, installed, search_query, roms_view_data, &mut commands);
                    } else if filtered_indices.is_empty()
                        && content_type_group == ContentTypeGroup::Themes
                        && crate::data::psvt::searching()
                    {
                        // The site is still answering the search.
                        ui.add_space(18.0);
                        let dots = ".".repeat(1 + (ui.input(|i| i.time) * 3.0) as usize % 3);
                        ui.label(
                            egui::RichText::new(format!("{}{dots}", lang.tr("themes-searching").trim_end_matches('…')))
                                .size(FONT_LARGE)
                                .color(pal().text_dim),
                        );
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
                    } else if filtered_indices.is_empty() {
                        ui.add_space(18.0);
                        let empty = match tab {
                            StoreTab::Library => lang.library_empty(),
                            StoreTab::Updates => lang.updates_empty(),
                            _ => lang.no_results(),
                        };
                        ui.label(egui::RichText::new(empty).size(FONT_LARGE).color(pal().text_dim));
                        if !matches!(tab, StoreTab::Library | StoreTab::Updates) {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(lang.no_results_sub())
                                    .size(FONT_BODY)
                                    .color(pal().text_faint),
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
                            content_type_group.grid_columns(),
                        );
                    }
                });
            });
        });
    commands
}

fn platform_pill(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        font(FONT_SMALL),
        if active { pal().text } else { pal().text_dim },
    );
    let size = egui::vec2(galley.size().x + 18.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if active {
        pal().primary
    } else if response.hovered() {
        pal().bg_card_hover
    } else {
        pal().bg_card
    };
    ui.painter().rect_filled(rect, 13.0, fill);
    if !active {
        ui.painter().rect_stroke(
            rect,
            13.0,
            egui::Stroke::new(1.0, pal().separator),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        if active { pal().text } else { pal().text_dim },
    );
    response.clicked()
}
