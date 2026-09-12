use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::tile_art_url;
use crate::app::ui::footer::{button_hints, status_note};
use crate::app::ui::grid::cover_fit_uv;
use crate::app::ui::theme::*;
use crate::app::ui::widgets::*;
use crate::data::{Category, Platform};
use crate::input::AppCommand;
use crate::install::installed::{InstallState, InstalledIndex};

pub(crate) fn detail_screen(
    ctx: &egui::Context,
    icons: &IconCache,
    installed: &InstalledIndex,
    lang: Language,
    entry: &crate::data::AppEntry,
    install: Option<&crate::install::Progress>,
    busy: bool,
    scroll_delta: f32,
    comments: &[crate::data::api::Comment],
    comments_loaded: bool,
    comment_entry_requested: bool,
    lightbox: Option<usize>,
    data_prompt: bool,
) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let state = installed.state(entry);
    egui::TopBottomPanel::top("detail_header")
        .frame(
            egui::Frame::NONE
                .fill(glass(BG_HEADER))
                .inner_margin(egui::vec2(SCREEN_MARGIN, 8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if busy {
                    ui.label(
                        egui::RichText::new(lang.install_in_progress())
                            .size(FONT_BODY)
                            .color(STAR_GOLD),
                    );
                } else if back_button(ui, lang.back()) {
                    commands.push(AppCommand::BackToCatalog);
                }
            });
        });
    if busy {
        commands.extend(button_hints(ctx, lang, &[], None));
    } else {
        commands.extend(button_hints(
            ctx,
            lang,
            &[
                (Glyph::Circle, lang.btn_back()),
                (Glyph::Cross, lang.btn_open()),
            ],
            Some(status_note(installed, icons)),
        ));
    }

    if data_prompt {
        egui::Area::new(egui::Id::new("data_prompt"))
            .fixed_pos(egui::Pos2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let screen = ui.ctx().screen_rect();
                let (bg, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(bg, 0.0, egui::Color32::from_black_alpha(200));
                let panel = egui::Rect::from_center_size(screen.center(), egui::vec2(360.0, 210.0));
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel), |ui| {
                    egui::Frame::window(&ctx.style())
                        .fill(BG_CARD)
                        .stroke(egui::Stroke::new(1.5_f32, STAR_GOLD))
                        .corner_radius(RADIUS_LG)
                        .inner_margin(egui::vec2(24.0, 18.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(lang.data_prompt_title())
                                        .size(FONT_LARGE)
                                        .strong()
                                        .color(TEXT_WHITE),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new(lang.data_prompt_body())
                                        .size(FONT_BODY)
                                        .color(TEXT_DIM),
                                );
                                if entry.data_size_bytes > 0 {
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new(entry.data_size_label())
                                            .size(FONT_BODY)
                                            .strong()
                                            .color(STAR_GOLD),
                                    );
                                }
                                ui.add_space(18.0);
                                ui.horizontal(|ui| {
                                    if pill_button(ui, lang.data_prompt_accept(), true) {
                                        commands.push(AppCommand::InstallCurrent);
                                    }
                                    ui.add_space(10.0);
                                    if pill_button(ui, lang.cancel_btn(), false) {
                                        commands.push(AppCommand::CancelDataPrompt);
                                    }
                                });
                            });
                        });
                });
            });
    }

    if let Some(job) = install {
        if !job.is_finished() {
            egui::Area::new(egui::Id::new("install_overlay"))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::window(&ctx.style())
                        .fill(BG_CARD)
                        .stroke(egui::Stroke::new(1.5_f32, ACCENT_STEAM))
                        .corner_radius(RADIUS_LG)
                        .inner_margin(egui::vec2(28.0, 20.0))
                        .show(ui, |ui| {
                            ui.set_width(300.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(entry.name.as_str())
                                        .size(16.0)
                                        .strong()
                                        .color(TEXT_WHITE),
                                );
                                ui.add_space(16.0);
                                install_stepper(ui, job);
                                ui.add_space(16.0);
                                ui.add(egui::Spinner::new().size(20.0).color(ACCENT_STEAM));
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(job.label()).size(13.0).color(TEXT_DIM),
                                );
                                if job.is_cancellable() {
                                    ui.add_space(14.0);
                                    if cancel_button(ui, lang.cancel_btn()) {
                                        commands.push(AppCommand::CancelInstall);
                                    }
                                }
                            });
                        });
                });
        }
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
                    let frame =
                        egui::Rect::from_center_size(screen.center(), egui::vec2(720.0, 405.0));
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(frame), |ui| {
                        let (rect, _) = ui.allocate_exact_size(frame.size(), egui::Sense::hover());
                        draw_screenshot(ui, icons, rect, url, entry.category, true);
                    });
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

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(SCREEN_MARGIN))
        .show(ctx, |ui| {
            if paint_hero(ui, icons, entry) {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
            egui::ScrollArea::vertical()
                .id_salt(&entry.id)
                .show(ui, |ui| {
                    egui::Frame::NONE
                        .fill(glass(BG_CARD))
                        .corner_radius(CARD_RADIUS)
                        .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                        .inner_margin(16.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let icon_rect = ui
                                    .allocate_exact_size(
                                        egui::vec2(84.0, 84.0),
                                        egui::Sense::hover(),
                                    )
                                    .0;
                                draw_icon(ui, icons, icon_rect, entry);
                                ui.add_space(14.0);
                                ui.vertical(|ui| {
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(&entry.name)
                                            .size(FONT_TITLE)
                                            .strong()
                                            .color(TEXT_WHITE),
                                    );
                                    ui.add_space(2.0);
                                    ui.horizontal_wrapped(|ui| {
                                        let author_label = lang.by_author(&entry.author);
                                        let author_btn = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new(author_label)
                                                    .color(ACCENT_CYAN)
                                                    .size(FONT_BODY),
                                            )
                                            .frame(false),
                                        );
                                        if author_btn.clicked() {
                                            commands.push(AppCommand::MoreByAuthor(
                                                entry.author.clone(),
                                            ));
                                        }
                                        ui.add_space(6.0);
                                        category_badge(ui, entry.category);
                                        if crate::data::SourceCatalog::Nps
                                            .matches(&entry.source_catalog)
                                        {
                                            for genre in entry.genres.iter().filter(|genre| {
                                                !genre.eq_ignore_ascii_case("other")
                                            }) {
                                                ui.add_space(4.0);
                                                genre_badge(ui, lang, genre);
                                            }
                                        }
                                        if entry.platform != Platform::Vita {
                                            ui.add_space(4.0);
                                            platform_badge(ui, entry.platform);
                                        }
                                        if let Some(region) = &entry.region {
                                            ui.add_space(4.0);
                                            region_badge(ui, region);
                                        }
                                        for label in &entry.source_labels {
                                            ui.add_space(4.0);
                                            source_badge(ui, label);
                                        }
                                        if entry.source_labels.is_empty() {
                                            if let Some(source) =
                                                crate::data::SourceCatalog::from_api(
                                                    &entry.source_catalog,
                                                )
                                            {
                                                ui.add_space(4.0);
                                                source_chip(ui, source);
                                            }
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
                                        ui.add_space(5.0);
                                        install_pill(ui, lang, state);
                                    }
                                    if entry.platform.is_nps() {
                                        ui.add_space(5.0);
                                        warning_pill(ui, lang.needs_nonpdrm());
                                    }
                                    if entry.data_url.is_some() || !entry.requirements.is_empty() {
                                        ui.add_space(5.0);
                                        let text = if entry.requirements.is_empty() {
                                            lang.needs_game_data()
                                        } else {
                                            entry.requirements.as_str()
                                        };
                                        warning_pill(ui, text);
                                    }
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        version_info_block(ui, lang, installed, entry, state);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| match install {
                                                None => {
                                                    let label = match (entry.platform, state) {
                                                        (Platform::Plugin, _) => lang.download(),
                                                        (_, InstallState::Absent) => lang.install(),
                                                        (_, InstallState::Installed) => {
                                                            lang.reinstall()
                                                        }
                                                        (_, InstallState::Outdated) => {
                                                            lang.update()
                                                        }
                                                    };
                                                    if play_install_button(ui, label, state) {
                                                        commands.push(AppCommand::InstallCurrent);
                                                    }
                                                }
                                                Some(progress) => {
                                                    if install_status(ui, progress) {
                                                        commands.push(AppCommand::DismissInstall);
                                                    }
                                                }
                                            },
                                        );
                                    });
                                });
                            });
                        });
                    ui.add_space(20.0);
                    screenshots_row(ui, icons, entry, &mut commands);
                    if !entry.changelog.trim().is_empty() {
                        ui.add_space(16.0);
                        egui::Frame::NONE
                            .fill(glass(BG_CARD))
                            .corner_radius(CARD_RADIUS)
                            .stroke(egui::Stroke::new(1.0_f32, ACCENT_CYAN.gamma_multiply(0.45)))
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                section_label(ui, lang.changelog());
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(entry.changelog.trim())
                                        .size(FONT_BODY)
                                        .color(TEXT_WHITE),
                                );
                            });
                        ui.add_space(16.0);
                    }
                    section_label(ui, lang.description());
                    ui.add_space(8.0);
                    let body = if entry.long_description.trim().is_empty() {
                        &entry.description
                    } else {
                        &entry.long_description
                    };
                    text_panel(ui, body, TEXT_WHITE);
                    ui.add_space(22.0);
                    if entry.platform == Platform::Plugin {
                        text_panel(ui, lang.plugin_manual_note(), STAR_GOLD);
                        ui.add_space(22.0);
                    }
                    if !entry.requirements.trim().is_empty() {
                        section_label(ui, lang.requirements());
                        ui.add_space(8.0);
                        text_panel(ui, entry.requirements.trim(), STAR_GOLD);
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
                                .color(TEXT_DIM),
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
                            .color(TEXT_WHITE),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !comment_entry_requested
                                && ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(lang.add_comment())
                                                .size(FONT_SMALL)
                                                .color(ACCENT_CYAN),
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
                                .color(TEXT_FAINT),
                        );
                    } else if comments.is_empty() {
                        ui.label(
                            egui::RichText::new(lang.no_comments_yet())
                                .size(FONT_SMALL)
                                .color(TEXT_FAINT),
                        );
                    } else {
                        for comment in comments {
                            egui::Frame::NONE
                                .fill(glass(BG_HEADER))
                                .corner_radius(RADIUS_MD)
                                .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(&comment.author_name)
                                                .size(FONT_SMALL)
                                                .strong()
                                                .color(ACCENT_CYAN),
                                        );
                                        ui.add_space(2.0);
                                        ui.label(
                                            egui::RichText::new(&comment.content)
                                                .size(FONT_BODY)
                                                .color(TEXT_WHITE),
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
        });
    commands
}

fn text_panel(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(glass(BG_CARD))
        .corner_radius(RADIUS_MD)
        .stroke(egui::Stroke::new(1.0_f32, GLASS_EDGE))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(FONT_LARGE).color(color));
        });
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .color(ACCENT_CYAN)
            .size(FONT_SMALL)
            .strong(),
    );
    ui.add_space(4.0);
}

const INSTALL_STEPS: [&str; 3] = ["Download", "Extract", "Install"];

fn install_stepper(ui: &mut egui::Ui, progress: &crate::install::Progress) {
    let failed = matches!(progress, crate::install::Progress::Failed(_));
    let current = progress.step();
    let circle_r = 14.0;
    let spacing = 90.0;
    let total_width = spacing * (INSTALL_STEPS.len() as f32 - 1.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(total_width, circle_r * 2.0 + 22.0),
        egui::Sense::hover(),
    );
    let center_y = rect.top() + circle_r;
    let start_x = rect.center().x - total_width / 2.0;
    for (index, label) in INSTALL_STEPS.iter().enumerate() {
        let step = index + 1;
        let center = egui::pos2(start_x + index as f32 * spacing, center_y);
        if index > 0 {
            let prev_center = egui::pos2(start_x + (index - 1) as f32 * spacing, center_y);
            let line_color = if step <= current && !(failed && step == current) {
                ACCENT_STEAM
            } else {
                SEPARATOR
            };
            ui.painter().line_segment(
                [
                    egui::pos2(prev_center.x + circle_r, center_y),
                    egui::pos2(center.x - circle_r, center_y),
                ],
                egui::Stroke::new(2.0_f32, line_color),
            );
        }
        let is_current = step == current;
        let is_done = step < current || (step == current && !failed && progress.is_finished());
        let (fill, text_color) = if failed && is_current {
            (
                egui::Color32::from_rgb(0x3a, 0x1c, 0x1c),
                egui::Color32::from_rgb(0xff, 0x6b, 0x6b),
            )
        } else if is_done || is_current {
            (ACCENT_STEAM, TEXT_WHITE)
        } else {
            (BG_CARD_HOVER, TEXT_DIM)
        };
        ui.painter().circle_filled(center, circle_r, fill);
        if !is_done && !is_current {
            ui.painter()
                .circle_stroke(center, circle_r, egui::Stroke::new(1.0_f32, SEPARATOR));
        }
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            step.to_string(),
            font(FONT_BODY),
            text_color,
        );
        ui.painter().text(
            egui::pos2(center.x, center.y + circle_r + 14.0),
            egui::Align2::CENTER_CENTER,
            *label,
            font(FONT_SMALL),
            if is_current { TEXT_WHITE } else { TEXT_DIM },
        );
    }
}

fn install_status(ui: &mut egui::Ui, progress: &crate::install::Progress) -> bool {
    use crate::install::Progress;
    let finished = progress.is_finished();
    let text = progress.label();
    let galley = ui.fonts(|f| f.layout_no_wrap(text.clone(), font(FONT_SMALL), TEXT_WHITE));
    let width = (galley.size().x + 28.0).min(240.0);
    let sense = if finished {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 38.0), sense);
    let (fill, text_color) = match progress {
        Progress::Done => (GREEN_PLAY.gamma_multiply(0.25), GREEN_PLAY_HOVER),
        Progress::Queued => (
            egui::Color32::from_rgb(0x3a, 0x30, 0x14),
            egui::Color32::from_rgb(0xf5, 0xb8, 0x42),
        ),
        Progress::Failed(_) => (
            egui::Color32::from_rgb(0x3a, 0x1c, 0x1c),
            egui::Color32::from_rgb(0xff, 0x6b, 0x6b),
        ),
        _ => (BG_CARD_HOVER, TEXT_DIM),
    };
    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter().rect_stroke(
        rect,
        RADIUS_SM,
        egui::Stroke::new(1.0_f32, ACCENT_STEAM),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &text,
        font(FONT_BODY),
        text_color,
    );
    if !finished {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
    response.clicked()
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
            .color(TEXT_DIM),
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
                    .color(TEXT_FAINT),
            );
        }
    });
}

fn cancel_button(ui: &mut egui::Ui, label: &str) -> bool {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font(FONT_SMALL), TEXT_WHITE));
    let desired = egui::vec2(galley.size().x + 24.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let (fill, stroke) = if response.hovered() {
        (
            egui::Color32::from_rgb(0x3a, 0x1c, 0x1c),
            egui::Color32::from_rgb(0xff, 0x6b, 0x6b),
        )
    } else {
        (BG_CARD_HOVER, SEPARATOR)
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
        TEXT_WHITE,
    );
    response.clicked()
}

fn play_install_button(ui: &mut egui::Ui, label: &str, state: InstallState) -> bool {
    let desired = egui::vec2(130.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let press_t = if response.is_pointer_button_down_on() {
        1.0_f32
    } else {
        0.0_f32
    };
    let hover_t = if response.hovered() { 1.0_f32 } else { 0.0_f32 };
    let rect = rect.shrink(press_t * (PRESS_SHRINK * 0.6));
    let (base, hover) = match state {
        InstallState::Outdated => (STAR_GOLD, STAR_GOLD_HOVER),
        InstallState::Absent | InstallState::Installed => (BLUE_PLAY, BLUE_PLAY_HOVER),
    };
    let bg = base.lerp_to_gamma(hover, hover_t);
    ui.painter().rect_filled(rect, RADIUS_SM, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font(FONT_LARGE),
        TEXT_WHITE,
    );
    response.clicked()
}

const SCREENSHOT_SIZE: egui::Vec2 = egui::vec2(240.0, 135.0);

fn screenshots_row(
    ui: &mut egui::Ui,
    icons: &IconCache,
    entry: &crate::data::AppEntry,
    commands: &mut Vec<AppCommand>,
) {
    if entry.screenshot_urls.is_empty() {
        return;
    }
    egui::ScrollArea::horizontal()
        .id_salt("screenshots")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, url) in entry.screenshot_urls.iter().enumerate() {
                    let (rect, response) =
                        ui.allocate_exact_size(SCREENSHOT_SIZE, egui::Sense::click());
                    let nearby = rect.right() > ui.clip_rect().left()
                        && rect.left() < ui.clip_rect().right();
                    draw_screenshot(ui, icons, rect, url, entry.category, nearby);
                    if response.clicked() {
                        commands.push(AppCommand::OpenScreenshot(index));
                    }
                    ui.add_space(10.0);
                }
            });
        });
    ui.add_space(20.0);
}

fn draw_screenshot(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    url: &str,
    category: Category,
    fetch: bool,
) {
    ui.painter().rect_filled(rect, RADIUS_MD, BG_DEEP);
    ui.painter().rect_stroke(
        rect,
        RADIUS_MD,
        egui::Stroke::new(1.0_f32, SEPARATOR),
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
            TEXT_WHITE,
        );
        ui.painter().rect_stroke(
            rect,
            RADIUS_MD,
            egui::Stroke::new(1.0_f32, SEPARATOR),
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
            "No Image",
            font(FONT_SMALL),
            TEXT_DIM,
        );
    }
}

fn draw_icon(
    ui: &mut egui::Ui,
    icons: &IconCache,
    rect: egui::Rect,
    entry: &crate::data::AppEntry,
) {
    let color = category_color(entry.category);
    let corner_r = rect.width() * 0.22;
    let plate = color.lerp_to_gamma(BG_DEEP, 0.62);
    ui.painter().rect_filled(rect, corner_r, plate);
    let art = tile_art_url(entry);
    if let Some(url) = art {
        if let Some(texture) = icons.get(ui.ctx(), url) {
            ui.painter().add(
                egui::epaint::RectShape::filled(rect, corner_r, TEXT_WHITE)
                    .with_texture(texture.id(), cover_fit_uv(texture.size_vec2(), rect)),
            );
            return;
        }
    }
    let letter = entry
        .name
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let loading = art.is_some_and(|url| icons.is_loading(url, crate::app::icons::MAX_ICON_SIDE));
    let pulse = if loading {
        let t = ui.input(|i| i.time);
        0.55 + 0.35 * ((t * 3.0).sin() as f32 * 0.5 + 0.5)
    } else {
        1.0
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        font((rect.width() * 0.42).round()),
        TEXT_WHITE.gamma_multiply(pulse),
    );
    if loading {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(80));
    }
}
