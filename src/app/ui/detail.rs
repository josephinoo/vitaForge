use crate::app::detail_actions::{self, DetailAction};
use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::tile_art_url;
use crate::app::ui::footer::{button_hints, status_note};
use crate::app::ui::grid::cover_fit_uv;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::*;
use crate::data::{Category, Platform};
use crate::input::{AppCommand, DetailFocus};
use crate::install::installed::{InstallState, InstalledIndex};

pub(crate) fn detail_screen(
    ctx: &egui::Context,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    entry: &crate::data::AppEntry,
    install: Option<&crate::install::Progress>,
    busy: bool,
    cancel_armed: bool,
    scroll_delta: f32,
    comments: &[crate::data::api::Comment],
    comments_loaded: bool,
    comment_entry_requested: bool,
    lightbox: Option<usize>,
    data_prompt: bool,
    data_language: usize,
    data_step: crate::input::DataStep,
    uninstall_prompt: bool,
    report_notice: Option<&str>,
    apps: &[crate::data::AppEntry],
    dep_job: Option<(&str, &crate::install::Progress)>,
    detail_focus: DetailFocus,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let state = installed.state(entry);
    egui::TopBottomPanel::top("detail_header")
        .exact_height(46.0)
        .frame(
            egui::Frame::NONE
                .fill(glass(pal().bg_header))
                .inner_margin(egui::vec2(SCREEN_MARGIN, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if busy {
                    ui.label(
                        egui::RichText::new(lang.install_in_progress())
                            .size(FONT_BODY)
                            .color(pal().warning),
                    );
                } else if back_button(ui, lang.back()) {
                    commands.push(AppCommand::BackToCatalog);
                }
            });
        });
    let cancellable = install.is_some_and(|job| job.is_cancellable());
    let primary = detail_actions::primary(entry, state);
    let failed = install.is_some_and(|job| matches!(job, crate::install::Progress::Failed(_)));
    if busy {
        let hints: &[(Glyph, &str)] = if cancellable {
            &[(Glyph::Circle, if cancel_armed { lang.cancel_confirm() } else { lang.cancel_btn() })]
        } else {
            &[]
        };
        commands.extend(button_hints(ctx, lang, hints, None));
    } else if install.is_some() {
        let mut hints = vec![(Glyph::Circle, lang.btn_back())];
        if failed {
            hints.push((Glyph::Square, lang.report_btn()));
        }
        commands.extend(button_hints(
            ctx,
            lang,
            &hints,
            Some(status_note(installed, icons)),
        ));
    } else {
        let mut hints = vec![(Glyph::Circle, lang.btn_back())];
        match detail_focus {
            DetailFocus::Author => {
                hints.push((Glyph::Cross, lang.see_all()));
            }
            DetailFocus::Screenshots => {
                hints.push((Glyph::Cross, lang.btn_open()));
            }
            DetailFocus::Requirements => {
                hints.push((Glyph::Cross, lang.get_btn()));
            }
            DetailFocus::Primary => {
                hints.push((Glyph::Cross, action_label(lang, primary)));
                if let Some(action) = detail_actions::secondary(entry, state) {
                    hints.push((Glyph::Triangle, action_label(lang, action)));
                }
                if let Some(action) = detail_actions::tertiary(entry, state) {
                    hints.push((Glyph::Square, action_label(lang, action)));
                }
            }
        }
        commands.extend(button_hints(
            ctx,
            lang,
            &hints,
            Some(status_note(installed, icons)),
        ));
    }

    if data_prompt {
        data_prompt_modal(ctx, entry, data_language, data_step, lang, &mut commands);
    }

    if uninstall_prompt {
        paint_modal_scrim(ctx);
        egui::Area::new(egui::Id::new("uninstall_prompt"))
            .anchor(egui::Align2::CENTER_CENTER, modal_center_offset(ctx))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::window(&ctx.style())
                    .fill(pal().bg_card)
                    .stroke(egui::Stroke::new(
                        1.5_f32,
                        pal().danger,
                    ))
                    .corner_radius(RADIUS_LG)
                    .inner_margin(egui::vec2(24.0, 18.0))
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(lang.uninstall_title(&entry.name))
                                    .size(FONT_LARGE)
                                    .strong()
                                    .color(pal().text),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(lang.uninstall_body())
                                    .size(FONT_BODY)
                                    .color(pal().text_dim),
                            );
                            ui.add_space(18.0);
                            ui.horizontal(|ui| {
                                if pill_button(ui, lang.uninstall_btn(), true) {
                                    commands.push(AppCommand::ConfirmUninstall);
                                }
                                ui.add_space(10.0);
                                if pill_button(ui, lang.cancel_btn(), false) {
                                    commands.push(AppCommand::CancelUninstall);
                                }
                            });
                        });
                    });
            });
    }

    if let Some(shot_idx) = lightbox {
        if let Some(url) = entry.screenshot_urls.get(shot_idx) {
            egui::Area::new(egui::Id::new("screenshot_lightbox"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let screen = ui.ctx().screen_rect();
                    let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                    ui.painter()
                        .rect_filled(bg, 0.0, egui::Color32::from_black_alpha(200));
                    if bg_resp.clicked() {
                        commands.push(AppCommand::CloseScreenshot);
                    }
                    let frame = egui::Rect::from_center_size(
                        detail_content_center(ui.ctx()),
                        egui::vec2(720.0, 405.0),
                    );
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(frame), |ui| {
                        let (rect, _) = ui.allocate_exact_size(frame.size(), egui::Sense::hover());
                        draw_screenshot(ui, icons, rect, url, entry.category, true, lang);
                    });
                    let total = entry.screenshot_urls.len();
                    if total > 1 {
                        ui.painter().text(
                            egui::pos2(screen.center().x, frame.bottom() + 16.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{} / {total}", shot_idx + 1),
                            font(FONT_BODY),
                            pal().text_dim,
                        );
                        for (step, label, x) in [
                            (-1, "<", screen.center().x - 400.0),
                            (1, ">", screen.center().x + 400.0),
                        ] {
                            let rect = egui::Rect::from_center_size(
                                egui::pos2(x, screen.center().y),
                                egui::vec2(40.0, 60.0),
                            );
                            let response = ui.interact(
                                rect,
                                egui::Id::new(("shot_step", step)),
                                egui::Sense::click(),
                            );
                            ui.painter().rect_filled(
                                rect,
                                RADIUS_MD,
                                if response.hovered() {
                                    pal().bg_card_hover
                                } else {
                                    pal().bg_card
                                },
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                label,
                                font(FONT_HEADLINE),
                                pal().text,
                            );
                            if response.clicked() {
                                commands.push(AppCommand::ScreenshotStep(step));
                            }
                        }
                    }
                    ui.allocate_new_ui(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::pos2(screen.right() - 120.0, screen.top() + 16.0),
                            egui::vec2(100.0, 36.0),
                        )),
                        |ui| {
                            if back_button(ui, lang.back()) {
                                commands.push(AppCommand::CloseScreenshot);
                            }
                        },
                    );
                });
        }
    }

    if let Some(job) = install {
        install_progress_modal(ctx, lang, entry, job, cancellable, cancel_armed, &mut commands);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            if paint_hero(ui, icons, entry) {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
            // Stop short of the hint bar so the last visible card is not cut flush against it.
            let scroll_h = (ui.available_height() - 12.0).max(80.0);
            let viewport_w = ui.available_width();
            let enter_t = detail_enter_t(ui.ctx(), egui::Id::new(("detail_enter", entry.id.as_str())));
            let scroll_output = egui::ScrollArea::vertical()
                .id_salt(&entry.id)
                .auto_shrink([false, false])
                .max_height(scroll_h)
                .show(ui, |ui| {
                    ui.set_max_width(viewport_w);
                    ui.multiply_opacity(enter_t);
                    let gap = 18.0;
                    let col_w = ((viewport_w - gap) / 2.0).floor();
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_max_width(col_w);
                                left_column(
                                    ui,
                                    icons,
                                    installed,
                                    lang,
                                    entry,
                                    state,
                                    install,
                                    report_notice,
                                    detail_focus,
                                    &mut commands,
                                );
                            },
                        );
                        ui.add_space(gap);
                        ui.allocate_ui_with_layout(
                            egui::vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_max_width(col_w);
                                right_column(
                                    ui,
                                    icons,
                                    lang,
                                    entry,
                                    state,
                                    apps,
                                    dep_job,
                                    detail_focus,
                                    &mut commands,
                                );
                            },
                        );
                    });
                    ui.add_space(20.0);
                    if !entry.changelog.trim().is_empty() {
                        ui.add_space(16.0);
                        egui::Frame::NONE
                            .fill(glass(pal().bg_card))
                            .corner_radius(CARD_RADIUS)
                            .stroke(egui::Stroke::new(1.0_f32, pal().accent_soft.gamma_multiply(0.45)))
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                section_label(ui, lang.changelog());
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(entry.changelog.trim())
                                        .size(FONT_BODY)
                                        .color(pal().text),
                                );
                            });
                        ui.add_space(16.0);
                    }
                    if entry.platform == Platform::Plugin {
                        text_panel(ui, lang.plugin_manual_note(), pal().warning);
                        ui.add_space(22.0);
                    }
                    if !entry.overview.is_empty() {
                        section_label(ui, lang.overview());
                        ui.add_space(8.0);
                        for (key, value) in &entry.overview {
                            info_row(ui, key, value);
                        }
                        ui.add_space(22.0);
                    }
                    section_label(ui, lang.technical_info());
                    ui.add_space(8.0);
                    info_row(
                        ui,
                        lang.installed_version(),
                        match state {
                            InstallState::Absent => "-",
                            InstallState::Installed => lang.installed(),
                            InstallState::Outdated => lang.update_available(),
                        },
                    );
                    info_row(ui, lang.version(), &entry.version);
                    info_row(ui, lang.size(), &entry.size_label());
                    if entry.data_size_bytes > 0 {
                        info_row(ui, lang.needs_game_data(), &entry.data_size_label());
                    }
                    info_row(ui, lang.downloads(), &entry.downloads.to_string());
                    info_row(ui, lang.rating(), &format!("{:.1} / 5", entry.rating));
                    info_row(ui, lang.updated(), &entry.updated_at);
                    if !entry.titleid.is_empty() {
                        info_row(ui, "Title ID", &entry.titleid);
                    }
                    if let Some(page) = &entry.release_page {
                        info_row(ui, lang.release_page(), page);
                    }
                    ui.add_space(22.0);
                    section_label(ui, lang.community());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(lang.your_rating())
                                .size(FONT_BODY)
                                .color(pal().text_dim),
                        );
                        ui.add_space(8.0);
                        if let Some(score) = tappable_stars(ui, entry.user_rating) {
                            commands.push(AppCommand::RateCurrent(score));
                        }
                    });
                    info_row(ui, lang.ratings_count(), &entry.ratings_count.to_string());
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} ({})",
                                lang.comments(),
                                entry.comments_count
                            ))
                            .size(FONT_BODY)
                            .strong()
                            .color(pal().text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !comment_entry_requested
                                && ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(lang.add_comment())
                                                .size(FONT_SMALL)
                                                .color(pal().accent_soft),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                            {
                                commands.push(AppCommand::RequestCommentEntry);
                            }
                        });
                    });
                    ui.add_space(8.0);
                    if !comments_loaded {
                        ui.label(
                            egui::RichText::new(lang.loading_comments())
                                .size(FONT_SMALL)
                                .color(pal().text_faint),
                        );
                    } else if comments.is_empty() {
                        ui.label(
                            egui::RichText::new(lang.no_comments_yet())
                                .size(FONT_SMALL)
                                .color(pal().text_faint),
                        );
                    } else {
                        for comment in comments {
                            egui::Frame::NONE
                                .fill(glass(pal().bg_header))
                                .corner_radius(RADIUS_MD)
                                .stroke(egui::Stroke::new(1.0_f32, pal().glass_edge))
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(&comment.author_name)
                                                .size(FONT_SMALL)
                                                .strong()
                                                .color(pal().accent_soft),
                                        );
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(&comment.content)
                                                .size(FONT_BODY)
                                                .color(pal().text),
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    }
                    ui.add_space(20.0);
                    if scroll_delta != 0.0 {
                        ui.scroll_with_delta(egui::vec2(0.0, -scroll_delta));
                    }
                });
            fade_into_footer(ui, scroll_output.inner_rect);
        });
    commands
}

/// Lets the scrolling content dissolve into the hint bar instead of being cut
/// flush against it: a short gradient over the last rows, then solid bar colour
/// down to the bar itself.
fn fade_into_footer(ui: &egui::Ui, viewport: egui::Rect) {
    const FADE_H: f32 = 26.0;
    let bar = pal().bg_header;
    let clear = egui::Color32::from_rgba_unmultiplied(bar.r(), bar.g(), bar.b(), 0);
    let left = viewport.left() - SCREEN_MARGIN;
    let right = viewport.right() + SCREEN_MARGIN;
    let top = viewport.bottom() - FADE_H;
    let mut mesh = egui::Mesh::default();
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(egui::pos2(left, top), clear);
    mesh.colored_vertex(egui::pos2(right, top), clear);
    mesh.colored_vertex(egui::pos2(right, viewport.bottom()), bar);
    mesh.colored_vertex(egui::pos2(left, viewport.bottom()), bar);
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base, base + 2, base + 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    let below = egui::Rect::from_min_max(
        egui::pos2(left, viewport.bottom()),
        egui::pos2(right, ui.max_rect().bottom() + SCREEN_MARGIN),
    );
    ui.painter().rect_filled(below, 0.0, bar);
}

fn detail_content_center(ctx: &egui::Context) -> egui::Pos2 {
    let screen = ctx.screen_rect();
    egui::Rect::from_min_max(
        egui::pos2(screen.left(), screen.top() + 46.0),
        egui::pos2(screen.right(), screen.bottom() - HINT_BAR_HEIGHT),
    )
    .center()
}

/// Offset so `Align2::CENTER_CENTER` lands on the content area (not under header/hint bar).
fn modal_center_offset(ctx: &egui::Context) -> egui::Vec2 {
    detail_content_center(ctx) - ctx.screen_rect().center()
}

fn paint_modal_scrim(ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("modal_scrim"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            let (bg, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(bg, 0.0, egui::Color32::from_black_alpha(200));
        });
}

/// Installation owns its own surface.  Keeping progress here prevents the
/// primary action from jumping between an action button and a progress bar.
fn install_progress_modal(
    ctx: &egui::Context,
    lang: Language,
    entry: &crate::data::AppEntry,
    job: &crate::install::Progress,
    cancellable: bool,
    cancel_armed: bool,
    commands: &mut Vec<AppCommand>,
) {
    let finished = job.is_finished();
    egui::Area::new(egui::Id::new("install_progress_modal"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::window(&ctx.style())
                .fill(pal().bg_card)
                .stroke(egui::Stroke::new(
                    if cancel_armed { 2.5_f32 } else { 1.5_f32 },
                    if cancel_armed { pal().danger } else { pal().accent },
                ))
                .corner_radius(RADIUS_LG)
                .inner_margin(egui::vec2(28.0, 20.0))
                .show(ui, |ui| {
                    ui.set_width(400.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(entry.name.as_str())
                                .size(FONT_LARGE)
                                .strong()
                                .color(pal().text),
                        );
                        ui.add_space(16.0);
                        if !matches!(job, crate::install::Progress::Failed(_)) {
                            install_stepper(ui, job, lang);
                        }
                        ui.add_space(16.0);
                        if !finished {
                            if let Some(fraction) = job.fraction() {
                                ui.add(egui::ProgressBar::new(fraction).desired_width(360.0).fill(pal().accent));
                            } else {
                                ui.add(egui::Spinner::new().size(20.0).color(pal().accent));
                            }
                            ui.add_space(8.0);
                        }
                        ui.label(
                            egui::RichText::new(job.label_for(lang))
                                .size(FONT_SMALL)
                                .color(pal().text_dim),
                        );
                        if cancellable {
                            ui.add_space(14.0);
                            if cancel_armed {
                                ui.label(
                                    egui::RichText::new(lang.cancel_confirm())
                                        .size(FONT_SMALL)
                                        .strong()
                                        .color(pal().danger),
                                );
                                ui.add_space(6.0);
                            }
                            if cancel_button(ui, lang.cancel_btn()) {
                                commands.push(AppCommand::CancelInstall);
                            }
                        } else if finished {
                            ui.add_space(14.0);
                            if pill_button(ui, lang.back(), true) {
                                commands.push(AppCommand::DismissInstall);
                            }
                        }
                    });
                });
        });
    if !finished {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn install_stepper(ui: &mut egui::Ui, progress: &crate::install::Progress, lang: Language) {
    let steps = ["step-download", "step-verify", "step-extract", "step-install"].map(|id| lang.tr(id));
    use crate::install::Progress;
    let failed = matches!(progress, Progress::Failed(_));
    let current = match progress {
        Progress::Resolving
        | Progress::DownloadingData { .. }
        | Progress::Downloading { .. } => 1,
        Progress::Finishing { .. } => 2,
        Progress::Decrypting | Progress::Extracting { .. } => 3,
        Progress::VerificationComplete
        | Progress::Installing { .. }
        | Progress::Queued
        | Progress::Done
        | Progress::RebootRequired
        | Progress::Restarting
        | Progress::Failed(_) => 4,
    };
    let circle_r = 14.0;
    let spacing = 90.0;
    let total_width = spacing * (steps.len() as f32 - 1.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(total_width, circle_r * 2.0 + 22.0),
        egui::Sense::hover(),
    );
    let center_y = rect.top() + circle_r;
    let start_x = rect.center().x - total_width / 2.0;
    for (index, label) in steps.iter().enumerate() {
        let step = index + 1;
        let center = egui::pos2(start_x + index as f32 * spacing, center_y);
        if index > 0 {
            let previous = egui::pos2(start_x + (index - 1) as f32 * spacing, center_y);
            let color = if step <= current && !(failed && step == current) {
                pal().accent
            } else {
                pal().separator
            };
            ui.painter().line_segment(
                [
                    egui::pos2(previous.x + circle_r, center_y),
                    egui::pos2(center.x - circle_r, center_y),
                ],
                egui::Stroke::new(2.0_f32, color),
            );
        }
        let is_current = step == current;
        let is_done = step < current || (is_current && !failed && progress.is_finished());
        let (fill, text) = if failed && is_current {
            (
                pal().danger_bg,
                pal().danger,
            )
        } else if is_done || is_current {
            (pal().accent, pal().text)
        } else {
            (pal().bg_card_hover, pal().text_dim)
        };
        ui.painter().circle_filled(center, circle_r, fill);
        if !is_done && !is_current {
            ui.painter()
                .circle_stroke(center, circle_r, egui::Stroke::new(1.0_f32, pal().separator));
        }
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            step.to_string(),
            font(FONT_BODY),
            text,
        );
        ui.painter().text(
            egui::pos2(center.x, center.y + circle_r + 14.0),
            egui::Align2::CENTER_CENTER,
            label,
            font(FONT_SMALL),
            if is_current { pal().text } else { pal().text_dim },
        );
    }
}

fn text_panel(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(glass(pal().bg_card))
        .corner_radius(RADIUS_MD)
        .stroke(egui::Stroke::new(1.0_f32, pal().glass_edge))
        .inner_margin(egui::vec2(14.0, 12.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(FONT_BODY).color(color));
        });
}

/// Fade-in when opening a detail page (new id resets the clock).
fn detail_enter_t(ctx: &egui::Context, id: egui::Id) -> f32 {
    const SECS: f64 = 0.18;
    let now = ctx.input(|i| i.time);
    let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || now));
    let t = ((now - start) / SECS).clamp(0.0, 1.0) as f32;
    let t = t * t * (3.0 - 2.0 * t);
    if t < 1.0 {
        ctx.request_repaint();
    }
    t
}

fn entry_needs_game_data(entry: &crate::data::AppEntry) -> bool {
    if entry.data_url.is_some() || entry.data_size_bytes > 0 {
        return true;
    }
    is_game_data_req_text(&entry.requirements)
}

fn is_game_data_req_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("data files") || lower.contains("game files") || lower.contains("game data")
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .color(pal().accent_soft)
            .size(FONT_SMALL)
            .strong(),
    );
    ui.add_space(4.0);
}

fn version_info_block(
    ui: &mut egui::Ui,
    lang: Language,
    installed: &InstalledIndex,
    entry: &crate::data::AppEntry,
    state: InstallState,
) {
    let catalog_date = entry
        .updated_at
        .get(..10)
        .unwrap_or(entry.updated_at.as_str());
    ui.vertical(|ui| {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(format!(
                "{}: {} · {}",
                lang.version(),
                entry.version,
                catalog_date
            ))
            .size(FONT_SMALL)
            .color(pal().text_dim),
        );
        if state != InstallState::Absent {
            let info = installed
                .installed_info(ui.ctx(), entry)
                .unwrap_or_default();
            let installed_line = match (&info.app_ver, &info.installed_at) {
                (Some(ver), _) => format!("{}: {ver}", lang.installed_version_value()),
                (None, Some(date)) => format!("{}: {date}", lang.installed_version_value()),
                (None, None) => format!("{}: —", lang.installed_version_value()),
            };
            ui.label(
                egui::RichText::new(installed_line)
                    .size(FONT_SMALL)
                    .color(pal().text_faint),
            );
        }
    });
}

fn cancel_button(ui: &mut egui::Ui, label: &str) -> bool {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), pal().text));
    let desired = egui::vec2(galley.size().x + 24.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let (fill, stroke) = if response.hovered() {
        (
            pal().danger_bg,
            pal().danger,
        )
    } else {
        (pal().bg_card_hover, pal().separator)
    };
    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter().rect_stroke(
        rect,
        RADIUS_SM,
        egui::Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font(FONT_SMALL),
        pal().text,
    );
    response.clicked()
}

const THUMB_SIZE: egui::Vec2 = egui::vec2(132.0, 74.0);

fn screenshots_row(
    ui: &mut egui::Ui,
    icons: &IconCache,
    entry: &crate::data::AppEntry,
    commands: &mut Vec<AppCommand>,
    skip: usize,
    lang: Language,
) {
    egui::ScrollArea::horizontal()
        .id_salt("screenshots")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, url) in entry.screenshot_urls.iter().enumerate().skip(skip) {
                    let (rect, response) = ui.allocate_exact_size(THUMB_SIZE, egui::Sense::click());
                    let nearby = rect.right() > ui.clip_rect().left()
                        && rect.left() < ui.clip_rect().right();
                    draw_screenshot(ui, icons, rect, url, entry.category, nearby, lang);
                    if response.clicked() {
                        commands.push(AppCommand::OpenScreenshot(index));
                    }
                    ui.add_space(8.0);
                }
            });
        });
}

fn draw_screenshot(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    url: &str,
    category: Category,
    fetch: bool,
    lang: Language,
) {
    ui.painter().rect_filled(rect, RADIUS_MD, pal().bg_deep);
    ui.painter().rect_stroke(
        rect,
        RADIUS_MD,
        egui::Stroke::new(1.0_f32, pal().separator),
        egui::StrokeKind::Inside,
    );
    if !fetch {
        return;
    }
    if let Some(texture) = icons.get_sized(ui.ctx(), url, crate::app::icons::MAX_SCREENSHOT_SIDE) {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            pal().text,
        );
        ui.painter().rect_stroke(
            rect,
            RADIUS_MD,
            egui::Stroke::new(1.0_f32, pal().separator),
            egui::StrokeKind::Inside,
        );
        return;
    }
    if icons.is_loading(url, crate::app::icons::MAX_SCREENSHOT_SIDE) {
        let time = ui.input(|i| i.time);
        let angle = time * 4.0;
        let center = rect.center();
        let radius = 12.0;
        let color = category_color(category);
        let n_dots = 8;
        for i in 0..n_dots {
            let dot_angle = angle + (i as f64 * std::f64::consts::TAU / n_dots as f64);
            let pos = center + egui::vec2(dot_angle.cos() as f32, dot_angle.sin() as f32) * radius;
            let alpha = (i as f32 / n_dots as f32).powf(1.5);
            ui.painter()
                .circle_filled(pos, 2.5, color.gamma_multiply(0.2 + 0.8 * alpha));
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            lang.tr("no-image"),
            font(FONT_SMALL),
            pal().text_dim,
        );
    }
}

fn draw_icon(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    entry: &crate::data::AppEntry,
) {
    use crate::app::ui::chrome_icons::{self, ChromeIcon};

    let color = category_color(entry.category);
    let corner_r = rect.width() * 0.22;
    let plate = color.lerp_to_gamma(pal().bg_deep, 0.62);
    ui.painter().rect_filled(rect, corner_r, plate);
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, corner_r, pal().text)
                    .with_texture(texture.id(), cover_fit_uv(texture.size_vec2(), rect)),
            );
            return;
        }
    }
    let is_plugin = matches!(entry.category, Category::Plugin)
        || matches!(entry.platform, Platform::Plugin);
    let loading = art.is_some_and(|url| icons.is_loading(url, crate::app::icons::MAX_ICON_SIDE));
    let pulse = if loading {
        let t = ui.input(|i| i.time);
        0.55 + 0.35 * ((t * 3.0).sin() as f32 * 0.5 + 0.5)
    } else {
        1.0
    };
    if is_plugin {
        // Match grid: teal adaptive plate + puzzle glyph.
        let plate = egui::Color32::from_rgb(0x00, 0x96, 0x88);
        ui.painter().rect_filled(rect, corner_r, plate);
        let side = rect.width().min(rect.height()) * 0.48;
        let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(side, side));
        chrome_icons::paint(
            ui,
            ChromeIcon::Plugins,
            icon_rect,
            pal().text.gamma_multiply(pulse),
        );
    } else {
        let letter = entry
            .name
            .chars()
            .find(|c| !c.is_whitespace())
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            letter,
            font((rect.width() * 0.42).round()),
            pal().text.gamma_multiply(pulse),
        );
    }
    if loading {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(80));
    }
}

fn action_label(lang: Language, action: DetailAction) -> &'static str {
    match action {
        DetailAction::Install => lang.install(),
        DetailAction::Update => lang.update(),
        DetailAction::Reinstall => lang.reinstall(),
        DetailAction::Open => lang.open_btn(),
        DetailAction::Download => lang.download(),
        DetailAction::Uninstall => lang.uninstall_btn(),
    }
}

fn short_count(n: u64) -> String {
    match n {
        1_000_000.. => format!("{:.1}M", n as f64 / 1_000_000.0),
        10_000.. => format!("{}K", n / 1000),
        1_000.. => format!("{:.1}K", n as f64 / 1000.0),
        _ => n.to_string(),
    }
}

fn stats_row(ui: &mut egui::Ui, lang: Language, entry: &crate::data::AppEntry) {
    let version = if entry.version.trim().is_empty() {
        "-".to_owned()
    } else {
        entry.version.clone()
    };
    let dl_count = short_count(entry.downloads);
    let size_label = entry.size_label();
    let (size_cap, size_color) = if let Some((used, total)) = crate::app::sysinfo::storage("ux0:") {
        let available = total.saturating_sub(used);
        let free_gb = available as f64 / (1024.0 * 1024.0 * 1024.0);
        let is_short = crate::install::space_shortfall(entry).is_some();
        (
            format!("{free_gb:.1} GB ux0:"),
            if is_short { pal().danger } else { pal().text },
        )
    } else {
        (lang.size().to_owned(), pal().text)
    };
    let cells: [(&str, String, egui::Color32); 3] = [
        (&dl_count, lang.downloads().to_owned(), pal().text),
        (&size_label, size_cap, size_color),
        (&version, lang.version().to_owned(), pal().text),
    ];
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 34.0), egui::Sense::hover());
    let cell_w = width / cells.len() as f32;
    for (index, (value, caption, val_color)) in cells.iter().enumerate() {
        let center_x = rect.left() + cell_w * (index as f32 + 0.5);
        if index > 0 {
            let x = rect.left() + cell_w * index as f32;
            ui.painter().line_segment(
                [
                    egui::pos2(x, rect.top() + 4.0),
                    egui::pos2(x, rect.bottom() - 4.0),
                ],
                egui::Stroke::new(1.0_f32, pal().separator),
            );
        }
        ui.painter().text(
            egui::pos2(center_x, rect.top() + 9.0),
            egui::Align2::CENTER_CENTER,
            *value,
            font(FONT_TITLE),
            *val_color,
        );
        ui.painter().text(
            egui::pos2(center_x, rect.top() + 27.0),
            egui::Align2::CENTER_CENTER,
            caption,
            font(FONT_SMALL),
            pal().text_faint,
        );
    }
}

/// The one big action under the stats: Install / Update / Reinstall, which turns
/// into the progress bar while a job runs and into its result once it ends.
fn install_action(
    ui: &mut egui::Ui,
    lang: Language,
    entry: &crate::data::AppEntry,
    state: InstallState,
    install: Option<&crate::install::Progress>,
    commands: &mut Vec<AppCommand>,
) {
    use crate::install::Progress;
    let size = egui::vec2(ui.available_width(), 40.0);
    let sense = match install {
        Some(job) if !job.is_finished() => egui::Sense::hover(),
        _ => egui::Sense::click(),
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let radius = size.y / 2.0;
    let painter = ui.painter();
    match install {
        None => {
            let action = detail_actions::primary(entry, state);
            let short = matches!(
                action,
                DetailAction::Install | DetailAction::Update | DetailAction::Reinstall
            )
            .then(|| crate::install::space_shortfall(entry))
            .flatten();
            let (base, hover) = if short.is_some() {
                let red = pal().danger_bg;
                (red, red)
            } else {
                match action {
                    DetailAction::Update => (pal().warning, pal().warning_hover),
                    DetailAction::Install | DetailAction::Open => (pal().success, pal().success_hover),
                    DetailAction::Download => (pal().primary, pal().primary_hover),
                    _ => (pal().bg_card_hover, pal().bg_card_hover.gamma_multiply(1.3)),
                }
            };
            let enabled = short.is_none();
            let press_t = ui.ctx().animate_bool_with_time(
                ui.id().with("install_press"),
                enabled && response.is_pointer_button_down_on(),
                0.12,
            );
            let hover_t = ui.ctx().animate_bool_with_time(
                ui.id().with("install_hover"),
                enabled && response.hovered(),
                0.14,
            );
            let rect = rect.shrink(PRESS_SHRINK * press_t);
            painter.rect_filled(rect, radius, base.lerp_to_gamma(hover, hover_t));
            let label = if let Some((required, _)) = short {
                lang.no_space(&crate::data::cache_manager::format_bytes(required))
            } else {
                match action {
                    DetailAction::Update => lang.update_to(&entry.version),
                    DetailAction::Reinstall => lang.installed_reinstall().to_owned(),
                    other => action_label(lang, other).to_owned(),
                }
            };
            let text_color = match action {
                DetailAction::Install | DetailAction::Open if enabled => pal().bg_deep,
                _ => pal().text,
            };
            let galley = ui.fonts(|f| f.layout_no_wrap(label, font(FONT_LARGE), text_color));
            let glyph = enabled
                .then(|| button_texture(ui.ctx(), Glyph::Cross))
                .flatten();
            let icon = 22.0;
            let total = galley.size().x + if glyph.is_some() { icon + 10.0 } else { 0.0 };
            let mut x = rect.center().x - total / 2.0;
            if let Some(tex) = glyph {
                ui.painter().image(
                    tex.id(),
                    egui::Rect::from_center_size(
                        egui::pos2(x + icon / 2.0, rect.center().y),
                        egui::vec2(icon, icon),
                    ),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                x += icon + 10.0;
            }
            ui.painter().galley(
                egui::pos2(x, rect.center().y - galley.size().y / 2.0),
                galley,
                text_color,
            );
            if enabled {
                // Soft always-on ring for gamepad focus; brightens on hover.
                let glow_t = 0.55 + 0.45 * hover_t;
                ui.painter().rect_stroke(
                    rect.expand(1.5 + 0.5 * hover_t),
                    radius + 2.0,
                    egui::Stroke::new(
                        FOCUS_STROKE * glow_t,
                        pal().accent_soft
                            .gamma_multiply(0.65)
                            .lerp_to_gamma(pal().focus_glow, hover_t),
                    ),
                    egui::StrokeKind::Outside,
                );
            }
            if enabled && response.clicked() {
                commands.push(if action == DetailAction::Open {
                    AppCommand::LaunchCurrent
                } else {
                    AppCommand::InstallCurrent
                });
            }
        }
        Some(job) => {
            let finished = job.is_finished();
            let (fill, text_color, track) = match job {
                Progress::Done | Progress::RebootRequired | Progress::Restarting => (
                    pal().success.gamma_multiply(0.30),
                    pal().success_hover,
                    pal().success.gamma_multiply(0.30),
                ),
                Progress::Queued => (
                    pal().warning.gamma_multiply(0.2),
                    pal().warning,
                    pal().warning.gamma_multiply(0.2),
                ),
                Progress::Failed(_) => (
                    pal().danger_bg,
                    pal().danger,
                    pal().danger_bg,
                ),
                _ => (pal().accent, pal().text, pal().bg_card_hover),
            };
            painter.rect_filled(rect, radius, track);
            if !finished {
                match job.fraction() {
                    Some(frac) => {
                        let fill_w = (rect.width() * frac).max(size.y);
                        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, size.y));
                        painter.rect_filled(bar, radius, fill);
                    }
                    None => {
                        let t = ui.input(|i| i.time);
                        let pulse = 0.45 + 0.35 * ((t * 3.0).sin() as f32 * 0.5 + 0.5);
                        painter.rect_filled(rect, radius, fill.gamma_multiply(pulse));
                    }
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if !finished {
                    let label = job.label_for(lang);
                    let phase = label.split('·').next().unwrap_or(&label).trim();
                    match job.fraction() {
                        Some(fraction) => format!("{phase} · {:.0}%", fraction * 100.0),
                        None => phase.to_owned(),
                    }
                } else {
                    job.label_for(lang)
                },
                font(FONT_BODY),
                text_color,
            );
            if finished && response.clicked() {
                commands.push(AppCommand::DismissInstall);
            }
        }
    }
}

fn left_column(
    ui: &mut egui::Ui,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    entry: &crate::data::AppEntry,
    state: InstallState,
    install: Option<&crate::install::Progress>,
    report_notice: Option<&str>,
    detail_focus: DetailFocus,
    commands: &mut Vec<AppCommand>,
) {
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let icon_rect = ui
                .allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover())
                .0;
            draw_icon(ui, icons, icon_rect, entry);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(&entry.name)
                        .size(FONT_HEADLINE)
                        .strong()
                        .color(pal().text),
                );
                let is_author_focused = detail_focus == DetailFocus::Author;
                let author_btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new(&entry.author)
                            .color(if is_author_focused { pal().focus } else { pal().accent_soft })
                            .size(FONT_BODY)
                            .strong(),
                    )
                    .frame(false),
                );
                if is_author_focused {
                    ui.painter().rect_stroke(
                        author_btn.rect.expand(2.0),
                        RADIUS_XS,
                        egui::Stroke::new(FOCUS_STROKE, pal().focus),
                        egui::StrokeKind::Outside,
                    );
                }
                if author_btn.clicked() {
                    commands.push(AppCommand::SetDetailFocus(DetailFocus::Author));
                    commands.push(AppCommand::MoreByAuthor(entry.author.clone()));
                }
                if !entry.version.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(format!("v.{}", entry.version.trim()))
                            .size(FONT_SMALL)
                            .color(pal().text_faint),
                    );
                }
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    category_badge(ui, entry.category);
                    if entry_needs_game_data(entry) {
                        game_data_badge(ui, lang.game_data_badge());
                    }
                    if entry.platform.is_commercial() {
                        for genre in entry
                            .genres
                            .iter()
                            .filter(|genre| !genre.eq_ignore_ascii_case("other"))
                        {
                            genre_badge(ui, lang, genre);
                        }
                    }
                    if entry.platform != Platform::Vita {
                        platform_badge(ui, entry.platform);
                    }
                    if let Some(region) = &entry.region {
                        region_badge(ui, region);
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    rating_stars(ui, entry.rating);
                    ui.add_space(8.0);
                    if like_button(ui, entry.user_liked, entry.likes_count) {
                        commands.push(AppCommand::ToggleLike);
                    }
                });
                if state != InstallState::Absent {
                    install_pill(ui, lang, state);
                }
            });
        });
    });
    ui.add_space(12.0);
    stats_row(ui, lang, entry);
    ui.add_space(12.0);
    // A running install is represented by the modal above the detail view.
    // Do not replace this action with a progress bar in the background.
    if install.is_none() {
        install_action(ui, lang, entry, state, None, commands);
        ui.add_space(8.0);
    }
    match install {
        None => {
            let secondary = detail_actions::secondary(entry, state);
            let tertiary = detail_actions::tertiary(entry, state);
            if secondary.is_some() || tertiary.is_some() {
                ui.horizontal(|ui| {
                    if let Some(action) = secondary
                        && action_chip(ui, Glyph::Triangle, action_label(lang, action))
                    {
                        commands.push(if action == DetailAction::Open {
                            AppCommand::LaunchCurrent
                        } else {
                            AppCommand::InstallCurrent
                        });
                    }
                    if let Some(action) = tertiary
                        && action_chip(ui, Glyph::Square, action_label(lang, action))
                    {
                        commands.push(AppCommand::UninstallCurrent);
                    }
                });
            }
        }
        Some(crate::install::Progress::Failed(_)) => {
            if action_chip(ui, Glyph::Square, lang.report_btn()) {
                commands.push(AppCommand::ReportError);
            }
            if let Some(notice) = report_notice {
                ui.label(
                    egui::RichText::new(notice)
                        .size(FONT_SMALL)
                        .color(pal().success),
                );
            }
        }
        Some(_) => {}
    }
    version_info_block(ui, lang, installed, entry, state);
    ui.add_space(10.0);
    section_label(ui, lang.about_app());
    let body = if entry.long_description.trim().is_empty() {
        &entry.description
    } else {
        &entry.long_description
    };
    text_panel(ui, body, pal().text);
}

fn right_column(
    ui: &mut egui::Ui,
    icons: &IconCache,
    lang: Language,
    entry: &crate::data::AppEntry,
    state: InstallState,
    apps: &[crate::data::AppEntry],
    dep_job: Option<(&str, &crate::install::Progress)>,
    detail_focus: DetailFocus,
    commands: &mut Vec<AppCommand>,
) {
    if let Some(first) = entry.screenshot_urls.first() {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, (width * 9.0 / 16.0).round()),
            egui::Sense::click(),
        );
        let is_shot_focused = detail_focus == DetailFocus::Screenshots;
        draw_screenshot(ui, icons, rect, first, entry.category, true, lang);
        if is_shot_focused {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                RADIUS_MD,
                egui::Stroke::new(FOCUS_STROKE, pal().focus),
                egui::StrokeKind::Outside,
            );
        }
        if response.clicked() {
            commands.push(AppCommand::SetDetailFocus(DetailFocus::Screenshots));
            commands.push(AppCommand::OpenScreenshot(0));
        }
        if entry.screenshot_urls.len() > 1 {
            ui.add_space(8.0);
            screenshots_row(ui, icons, entry, commands, 1, lang);
        }
        ui.add_space(14.0);
    }
    requirements_block(ui, lang, entry, state, apps, dep_job, detail_focus, commands);
}

fn requirements_block(
    ui: &mut egui::Ui,
    lang: Language,
    entry: &crate::data::AppEntry,
    state: InstallState,
    apps: &[crate::data::AppEntry],
    dep_job: Option<(&str, &crate::install::Progress)>,
    detail_focus: DetailFocus,
    commands: &mut Vec<AppCommand>,
) {
    use crate::install::requirements::{ReqStatus, Verdict};
    let installed = state != InstallState::Absent;
    let items = crate::install::requirements::items(
        ui.ctx(),
        &entry.id,
        &entry.titleid,
        &entry.requirements,
    );
    let nps = entry.platform.is_nps();
    let show_data = entry_needs_game_data(entry);
    let data_hint = items
        .iter()
        .find(|item| is_game_data_req_text(&item.text))
        .map(|item| item.text.as_str());
    let plugin_items: Vec<_> = items
        .iter()
        .filter(|item| !is_game_data_req_text(&item.text))
        .collect();
    if plugin_items.is_empty() && !nps && !show_data {
        return;
    }
    let verdict = crate::install::requirements::verdict(&items, installed);
    let (heading, color) = match verdict {
        Verdict::Ready => (lang.ready_to_play(), pal().success),
        Verdict::NeedsFiles => (lang.needs_your_files(), pal().warning),
        Verdict::Plain => (lang.also_needs(), pal().warning),
    };
    egui::Frame::NONE
        .fill(glass(pal().bg_card))
        .corner_radius(CARD_RADIUS)
        .stroke(egui::Stroke::new(1.0_f32, pal().glass_edge))
        .inner_margin(egui::vec2(12.0, 10.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(heading)
                    .size(FONT_LARGE)
                    .strong()
                    .color(color),
            );
            ui.add_space(8.0);
            if show_data {
                game_data_callout(ui, lang, entry, data_hint);
                if !plugin_items.is_empty() || nps {
                    ui.add_space(8.0);
                }
            }
            for item in &plugin_items {
                ui.horizontal_wrapped(|ui| {
                    if installed {
                        let dot = match item.status {
                            ReqStatus::Found => pal().success,
                            ReqStatus::Missing => pal().warning,
                            ReqStatus::Unknown => pal().text.gamma_multiply(0.25),
                        };
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, dot);
                    }
                    ui.label(
                        egui::RichText::new(&item.text)
                            .size(FONT_BODY)
                            .color(pal().text),
                    );
                    if installed
                        && item.status == ReqStatus::Missing
                        && let Some(dep) =
                            crate::install::requirements::dependency(apps, &item.text)
                    {
                        ui.add_space(6.0);
                        match dep_job {
                            Some((running_id, progress)) if running_id == dep.id => {
                                ui.label(
                                    egui::RichText::new(progress.label_for(lang))
                                        .size(FONT_SMALL)
                                        .color(pal().warning),
                                );
                            }
                            _ => {
                                let is_req_focused = detail_focus == DetailFocus::Requirements;
                                if pill_button_focused(ui, lang.get_btn(), false, is_req_focused) {
                                    commands.push(AppCommand::SetDetailFocus(DetailFocus::Requirements));
                                    commands.push(AppCommand::InstallDependency {
                                        id: dep.id.clone(),
                                        dir: dep.dir.clone(),
                                        file: dep.file.clone(),
                                    });
                                }
                            }
                        }
                    }
                });
            }
            if nps {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(lang.needs_nonpdrm())
                        .size(FONT_SMALL)
                        .color(pal().warning),
                );
            }
        });
}

fn game_data_callout(
    ui: &mut egui::Ui,
    lang: Language,
    entry: &crate::data::AppEntry,
    req_hint: Option<&str>,
) {
    let appear = detail_enter_t(ui.ctx(), egui::Id::new(("gdata_enter", entry.id.as_str())));
    let prev = ui.opacity();
    ui.set_opacity(prev * appear);
    ui.add_space((1.0 - appear) * 6.0);
    egui::Frame::NONE
        .fill(pal().warning.gamma_multiply(0.12))
        .corner_radius(RADIUS_MD)
        .stroke(egui::Stroke::new(1.0_f32, pal().warning.gamma_multiply(0.55)))
        .inner_margin(egui::vec2(10.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                game_data_badge(ui, lang.game_data_badge());
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(lang.needs_game_data())
                            .size(FONT_SMALL)
                            .strong()
                            .color(pal().warning),
                    );
                    let mut detail = String::new();
                    if entry.data_size_bytes > 0 {
                        detail.push_str(&entry.data_size_label());
                    }
                    if let Some(path) = entry.data_extract_path.as_deref() {
                        if !detail.is_empty() {
                            detail.push_str(" · ");
                        }
                        detail.push_str(path);
                    } else if let Some(hint) = req_hint {
                        if !detail.is_empty() {
                            detail.push_str(" · ");
                        }
                        detail.push_str(hint);
                    }
                    if !detail.is_empty() {
                        ui.label(
                            egui::RichText::new(detail)
                                .size(FONT_MICRO)
                                .color(pal().text_dim),
                        );
                    }
                    if entry.data_url.is_none() {
                        ui.label(
                            egui::RichText::new(lang.data_not_auto())
                                .size(FONT_MICRO)
                                .color(pal().text_faint),
                        );
                    }
                });
            });
        });
    ui.set_opacity(prev);
}

fn action_chip(ui: &mut egui::Ui, glyph: Glyph, label: &str) -> bool {
    prompt_chip(ui, glyph, label, false)
}

/// Chip showing which pad button triggers it; `primary` is the solid blue action.
fn prompt_chip(ui: &mut egui::Ui, glyph: Glyph, label: &str, primary: bool) -> bool {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), pal().text));
    let icon = 16.0;
    // 14 left + icon + 6 gap + text + 14 right, so the content sits centred.
    let size = egui::vec2(galley.size().x + icon + 34.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let (fill, stroke) = if primary {
        (pal().primary.lerp_to_gamma(pal().primary_hover, t), egui::Stroke::NONE)
    } else {
        (pal().text.gamma_multiply(0.08 + 0.06 * t), egui::Stroke::NONE)
    };
    ui.painter().rect_filled(rect, size.y / 2.0, fill);
    ui.painter().rect_stroke(rect, size.y / 2.0, stroke, egui::StrokeKind::Inside);
    if let Some(tex) = button_texture(ui.ctx(), glyph) {
        ui.painter().image(
            tex.id(),
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 14.0 + icon / 2.0, rect.center().y),
                egui::vec2(icon, icon),
            ),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    ui.painter().galley(
        egui::pos2(
            rect.left() + 14.0 + icon + 6.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        pal().text,
    );
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::is_game_data_req_text;

    #[test]
    fn detects_game_data_requirement_lines() {
        assert!(is_game_data_req_text(
            "Game Data Files: Android in ux0:data/gtasa"
        ));
        assert!(is_game_data_req_text("- data files required"));
        assert!(!is_game_data_req_text("kubridge.skprx in ur0:tai"));
        assert!(!is_game_data_req_text("libshacccg.suprx"));
    }

    #[test]
    fn data_prompt_modal_renders_all_steps() {
        let ctx = egui::Context::default();
        let mut entry = crate::app::sort_tests::entry("dragon-ball-z", "vitadb", 0, 350 * 1024 * 1024, 0.0, "");
        entry.data_size_bytes = 350 * 1024 * 1024;
        entry.game_data_languages = vec![
            crate::data::GameDataLanguage {
                language: "es".into(),
                size_bytes: 350 * 1024 * 1024,
                url: "https://example.com/data_es.zip".into(),
                extract_path: None,
            },
        ];

        for step in [
            crate::input::DataStep::Choice,
            crate::input::DataStep::Language,
            crate::input::DataStep::Method,
        ] {
            let mut commands = Vec::new();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                super::data_prompt_modal(
                    ctx,
                    &entry,
                    1,
                    step,
                    crate::app::Language::English,
                    &mut commands,
                );
            });
        }
    }
}

/// Two-step game data modal: pick a language (only when the game has extra
/// languages), then confirm how to get the data for that choice.
fn data_prompt_modal(
    ctx: &egui::Context,
    entry: &crate::data::AppEntry,
    data_language: usize,
    step: crate::input::DataStep,
    lang: Language,
    commands: &mut Vec<AppCommand>,
) {
    use crate::input::DataStep;
    paint_modal_scrim(ctx);
    let chosen = crate::data::with_data_language(entry, data_language);
    egui::Area::new(egui::Id::new("data_prompt").with(step as u8))
        .anchor(egui::Align2::CENTER_CENTER, modal_center_offset(ctx))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::window(&ctx.style())
                .fill(pal().bg_card)
                .stroke(egui::Stroke::new(1.0_f32, pal().separator))
                .corner_radius(RADIUS_LG)
                .inner_margin(egui::vec2(20.0, 14.0))
                .show(ui, |ui| {
                    match step {
                        DataStep::Choice => choice_step(ui, entry, lang, commands),
                        DataStep::Language => language_step(ui, entry, data_language, lang, commands),
                        DataStep::Method => heavy_data_prompt(
                            ui,
                            &chosen,
                            chosen_language_label(entry, data_language, lang),
                            lang,
                            commands,
                        ),
                    }
                });
        });
}

/// Name of the chosen variant, only when the game offers more than one.
fn chosen_language_label(entry: &crate::data::AppEntry, choice: usize, lang: Language) -> Option<String> {
    if entry.game_data_languages.is_empty() {
        return None;
    }
    Some(match choice.checked_sub(1).and_then(|i| entry.game_data_languages.get(i)) {
        Some(l) => language_name(&l.language),
        None => lang.data_language_default(),
    })
}

fn language_step(
    ui: &mut egui::Ui,
    entry: &crate::data::AppEntry,
    selected: usize,
    lang: Language,
    commands: &mut Vec<AppCommand>,
) {
    ui.set_width(380.0);
    ui.label(egui::RichText::new(lang.data_language_title()).size(FONT_LARGE).strong().color(pal().text));
    ui.add_space(12.0);
    let options = std::iter::once((lang.data_language_default(), entry.data_size_bytes)).chain(
        entry.game_data_languages.iter().map(|l| (language_name(&l.language), l.size_bytes)),
    );
    for (i, (name, size)) in options.enumerate() {
        if language_row(ui, &name, size, i == selected) {
            commands.push(AppCommand::SelectDataLanguage(i));
        }
        ui.add_space(6.0);
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if prompt_chip(ui, Glyph::Cross, &lang.data_language_continue(), true) {
            commands.push(AppCommand::ConfirmDataLanguage);
        }
        ui.add_space(10.0);
        if prompt_chip(ui, Glyph::Circle, lang.cancel_btn(), false) {
            commands.push(AppCommand::CancelDataPrompt);
        }
    });
}

/// One selectable line: radio + name, size on the right.
fn language_row(ui: &mut egui::Ui, name: &str, size: u64, selected: bool) -> bool {
    let size = (size > 0).then(|| crate::data::cache_manager::format_bytes(size));
    crate::app::ui::widgets::radio_row(ui, name, size.as_deref(), selected, selected)
}

/// First question: just the app (VPK), or the app plus its game data.
fn choice_step(
    ui: &mut egui::Ui,
    entry: &crate::data::AppEntry,
    lang: Language,
    commands: &mut Vec<AppCommand>,
) {
    ui.set_width(420.0);
    ui.label(egui::RichText::new(lang.data_choice_title()).size(FONT_LARGE).strong().color(pal().text));
    ui.add_space(8.0);
    ui.add(egui::Label::new(egui::RichText::new(lang.data_prompt_body()).size(FONT_BODY).color(pal().text_dim)).wrap());
    if entry.data_size_bytes > 0 {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(entry.data_size_label()).size(FONT_BODY).strong().color(pal().warning));
    }
    ui.add_space(14.0);
    if prompt_chip(ui, Glyph::Cross, &lang.data_choice_with_data(), true) {
        commands.push(AppCommand::WantGameData);
    }
    ui.add_space(8.0);
    if prompt_chip(ui, Glyph::Triangle, &lang.data_choice_app_only(), false) {
        commands.push(AppCommand::InstallAppOnly);
    }
    ui.add(egui::Label::new(egui::RichText::new(lang.data_choice_app_only_hint()).size(FONT_SMALL).color(pal().text_faint)).wrap());
    ui.add_space(12.0);
    if prompt_chip(ui, Glyph::Circle, lang.cancel_btn(), false) {
        commands.push(AppCommand::CancelDataPrompt);
    }
}

/// Big data: two clearly separated ways to get it, each with its own button.
fn heavy_data_prompt(
    ui: &mut egui::Ui,
    entry: &crate::data::AppEntry,
    language: Option<String>,
    lang: Language,
    commands: &mut Vec<AppCommand>,
) {
    let url = format!("https://vitaforge.josephinoo.dev/apps/{}", entry.id);
    let path = entry.data_extract_path.as_deref().unwrap_or("ux0:data/");
    ui.set_width(640.0);
    let mut title = lang.data_heavy_title(&entry.data_size_label());
    if let Some(language) = language {
        title = format!("{title} · {language}");
    }
    ui.label(egui::RichText::new(title).size(FONT_LARGE).strong().color(pal().warning));
    ui.label(egui::RichText::new(lang.data_heavy_choose()).size(FONT_BODY).color(pal().text_dim));
    ui.add_space(12.0);
    ui.horizontal_top(|ui| {
        option_card(ui, 220.0, |ui| {
            ui.label(egui::RichText::new(lang.data_heavy_vita_title()).size(FONT_BODY).strong().color(pal().text));
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new(lang.data_heavy_vita_body()).size(FONT_SMALL).color(pal().text_dim)).wrap());
            ui.add_space(12.0);
            if prompt_chip(ui, Glyph::Triangle, &lang.data_heavy_download_here(), false) {
                commands.push(AppCommand::InstallCurrent);
            }
        });
        ui.add_space(12.0);
        option_card(ui, 360.0, |ui| {
            ui.label(egui::RichText::new(lang.data_heavy_pc_title()).size(FONT_BODY).strong().color(pal().warning));
            ui.add_space(6.0);
            let qr_size = 96.0;
            ui.horizontal_top(|ui| {
                paint_qr(ui, &url, qr_size);
                ui.add_space(10.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(360.0 - qr_size - 10.0, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        for step in lang.data_heavy_pc_steps(path) {
                            ui.add(egui::Label::new(egui::RichText::new(step).size(FONT_SMALL).color(pal().text_dim)).wrap());
                        }
                    },
                );
            });
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new(&url).size(FONT_MICRO).color(pal().text_faint)).truncate());
            ui.add_space(8.0);
            if prompt_chip(ui, Glyph::Cross, &lang.data_heavy_pc_done(), true) {
                commands.push(AppCommand::InstallAppOnly);
            }
        });
    });
    ui.add_space(12.0);
    if prompt_chip(ui, Glyph::Circle, lang.cancel_btn(), false) {
        commands.push(AppCommand::CancelDataPrompt);
    }
}

fn option_card(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(pal().bg_deep)
        .stroke(egui::Stroke::new(1.0_f32, pal().separator))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::vec2(12.0, 10.0))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(width);
                    ui.set_max_width(width);
                    add(ui);
                },
            );
        });
}

/// Native name for common codes; the font has no CJK glyphs, so those stay as codes.
fn language_name(code: &str) -> String {
    match code.split('-').next().unwrap_or(code) {
        "en" => "English".to_owned(),
        "es" => "Español".to_owned(),
        "fr" => "Français".to_owned(),
        "it" => "Italiano".to_owned(),
        "pt" => "Português".to_owned(),
        "de" => "Deutsch".to_owned(),
        _ => code.to_uppercase(),
    }
}

/// Draws `text` as a QR code (black modules on a white quiet zone) in a `side` px square.
fn paint_qr(ui: &mut egui::Ui, text: &str, side: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, egui::Color32::WHITE);
    let Ok(qr) = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium) else {
        return;
    };
    let n = qr.size();
    let module = side / (n + 4) as f32; // 2-module quiet zone each side
    let origin = rect.min + egui::vec2(2.0 * module, 2.0 * module);
    for y in 0..n {
        for x in 0..n {
            if qr.get_module(x, y) {
                let min = origin + egui::vec2(x as f32 * module, y as f32 * module);
                painter.rect_filled(
                    egui::Rect::from_min_size(min, egui::vec2(module + 0.5, module + 0.5)),
                    0.0,
                    egui::Color32::BLACK,
                );
            }
        }
    }
}
