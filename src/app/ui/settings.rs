use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::theme::*;
use crate::app::ui::widgets::{button_texture, radio_row, Glyph};
use crate::data::cache_manager::{format_bytes, CacheStats};
use crate::input::AppCommand;
use crate::install::tai::Snapshot;
use crate::install::tai::config::LineStyle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Account,
    General,
    Appearance,
    Plugins,
    Storage,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    AccountStatus,
    AccountUsername,
    AccountPassword,
    AccountEmail,
    AccountSubmit,
    AccountToggleMode,
    AccountLogout,
    Language,
    Notifications,
    SoundEffects,
    Theme,
    ClearIcons,
    ClearCatalog,
    PurgeAll,
    PluginManager,
    ConfigViewer,
    ConfigBackups,
    Profiles,
    SafeMode,
    MigrateConfig,
    Reboot,
}

/// A choice list that replaces the section's rows until confirmed or backed out of.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SubPage {
    Language,
    Theme,
    /// Every config.txt plugin line, grouped by section.
    Plugins,
    /// config.txt as text, read-only.
    Config,
    Backups,
    /// Saved plugin setups: save the current one, apply or delete another.
    Profiles,
}

/// Sections in rail order with the focusable rows each one holds.
pub const SECTIONS: [(Section, &[Row]); 6] = [
    (
        Section::Account,
        &[
            Row::AccountStatus,
            Row::AccountUsername,
            Row::AccountPassword,
            Row::AccountEmail,
            Row::AccountSubmit,
            Row::AccountToggleMode,
            Row::AccountLogout,
        ],
    ),
    (Section::General, &[Row::Language, Row::Notifications, Row::SoundEffects]),
    (Section::Appearance, &[Row::Theme]),
    (
        Section::Plugins,
        &[Row::PluginManager, Row::ConfigViewer, Row::ConfigBackups, Row::Profiles, Row::SafeMode, Row::MigrateConfig, Row::Reboot],
    ),
    (Section::Storage, &[Row::ClearIcons, Row::ClearCatalog, Row::PurgeAll]),
    (Section::About, &[]),
];

/// Where the d-pad is inside Settings.
#[derive(Clone, Copy, Debug)]
pub struct SettingsNav {
    pub section: usize,
    pub row: usize,
    /// Focus is on the section rail instead of the rows.
    pub rail: bool,
    pub sub: Option<SubPage>,
    pub sub_index: usize,
    /// Saved theme, restored when a theme preview is backed out of.
    pub theme_before: ThemeId,
    /// Length of the open plugin list, config view or backup list.
    pub list_len: usize,
    /// □ in the plugin list: ↑/↓ carry the focused line instead of moving focus.
    pub moving: bool,
}

impl SettingsNav {
    pub fn new() -> Self {
        Self {
            section: 0,
            row: 0,
            rail: false,
            sub: None,
            sub_index: 0,
            theme_before: active_theme(),
            list_len: 0,
            moving: false,
        }
    }

    pub fn rows(&self) -> &'static [Row] {
        SECTIONS[self.section].1
    }

    pub fn current_row(&self) -> Option<Row> {
        if self.rail { None } else { self.rows().get(self.row).copied() }
    }

    /// Jumps to a section; a section without rows keeps focus on the rail.
    pub fn go_section(&mut self, section: usize) {
        self.section = section % SECTIONS.len();
        self.row = 0;
        self.sub = None;
        if self.rows().is_empty() {
            self.rail = true;
        }
    }

    pub fn sub_len(&self) -> usize {
        match self.sub {
            Some(SubPage::Language) => Language::ALL.len(),
            Some(SubPage::Theme) => ThemeId::ALL.len(),
            Some(SubPage::Plugins | SubPage::Config | SubPage::Backups | SubPage::Profiles) => self.list_len,
            None => 0,
        }
    }
}

fn section_label(section: Section, lang: Language) -> &'static str {
    match section {
        Section::Account => lang.settings_account(),
        Section::General => lang.settings_general(),
        Section::Appearance => lang.settings_appearance(),
        Section::Plugins => lang.settings_plugins(),
        Section::Storage => lang.settings_storage(),
        Section::About => lang.settings_about(),
    }
}

fn section_icon(section: Section) -> ChromeIcon {
    match section {
        Section::Account => ChromeIcon::User,
        Section::General => ChromeIcon::Sliders,
        Section::Appearance => ChromeIcon::Palette,
        Section::Plugins => ChromeIcon::Plugins,
        Section::Storage => ChromeIcon::Storage,
        Section::About => ChromeIcon::Info,
    }
}

pub fn theme_label(theme: ThemeId, lang: Language) -> &'static str {
    match theme {
        ThemeId::Midnight => lang.theme_midnight(),
        ThemeId::Obsidian => lang.theme_obsidian(),
        ThemeId::OledBlack => lang.theme_oled_black(),
        ThemeId::Aurora => lang.theme_aurora(),
        ThemeId::Sunset => lang.theme_sunset(),
        ThemeId::HighContrast => lang.theme_high_contrast(),
        ThemeId::VitaClassic => lang.theme_vita_classic(),
        ThemeId::Graphite => lang.theme_graphite(),
        ThemeId::Forest => lang.theme_forest(),
        ThemeId::Nebula => lang.theme_nebula(),
        ThemeId::Sakura => lang.theme_sakura(),
        ThemeId::Mocha => lang.theme_mocha(),
        ThemeId::TokyoNight => lang.theme_tokyo_night(),
        ThemeId::Catppuccin => lang.theme_catppuccin(),
        ThemeId::Nord => lang.theme_nord(),
        ThemeId::Gruvbox => lang.theme_gruvbox(),
        ThemeId::Dracula => lang.theme_dracula(),
        ThemeId::RosePine => lang.theme_rose_pine(),
        ThemeId::Everforest => lang.theme_everforest(),
        ThemeId::Kanagawa => lang.theme_kanagawa(),
    }
}

/// "AJUSTES" -> "Ajustes": the stored title is the old all-caps modal label.
fn title_case(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub(crate) struct SettingsView<'a> {
    pub lang: Language,
    pub nav: &'a SettingsNav,
    pub catalog_count: usize,
    pub stats: &'a CacheStats,
    pub cache_notice: Option<&'a str>,
    pub install_notifications: bool,
    pub sound_effects: bool,
    pub purge_armed: bool,
    pub plugins: PluginsView<'a>,
    pub auth_form: &'a crate::app::AuthForm,
    pub auth_busy: bool,
    pub auth_status: Option<&'a (String, bool)>,
    pub appear: f32,
}

/// What Settings › Plugins shows; `tai` is `None` until config.txt was read.
pub(crate) struct PluginsView<'a> {
    pub tai: Option<&'a Snapshot>,
    /// Catalog ids with a newer version than the installed one.
    pub outdated: &'a [String],
    pub pending_reboot: bool,
    pub reboot_armed: bool,
    /// △ in the plugin list or ✕ in the backups waits for a second press.
    pub armed: bool,
    pub notice: Option<&'a str>,
}

const RAIL_WIDTH: f32 = 196.0;
const ROW_HEIGHT: f32 = 56.0;

/// Full-screen settings: section rail on the left, the section's rows (or an
/// open choice list) on the right. The footer hints stay visible below it.
pub(crate) fn settings_page(ctx: &egui::Context, view: SettingsView) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    // A CentralPanel is whatever the footer panel leaves free, so nothing can reach under it.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let page = ui.max_rect();
            ui.multiply_opacity(view.appear);
            // Swallow clicks so they never reach the catalog underneath.
            let _ = ui.interact(page, ui.id().with("blocker"), egui::Sense::click());
            paint_background(ui.painter(), page);
            // Slides in from the right as it fades.
            let shift = 24.0 * (1.0 - view.appear);
            let inner = page.shrink2(egui::vec2(SCREEN_MARGIN, 0.0)).translate(egui::vec2(shift, 0.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner), |ui| {
                header(ui, view.lang, &mut commands);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 18.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(RAIL_WIDTH, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| rail(ui, &view, &mut commands),
                    );
                    ui.allocate_ui_with_layout(
                        ui.available_size(),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            // Back on the first row: the title and hint above it scroll into view too.
                            let focus_key = egui::Id::new("settings_scroll_focus");
                            let focus = (view.nav.section, view.nav.sub, view.nav.sub_index, view.nav.row);
                            let went_to_first = ctx.data_mut(|d| {
                                let previous = d.get_temp::<(usize, Option<SubPage>, usize, usize)>(focus_key);
                                d.insert_temp(focus_key, focus);
                                let first = if focus.1.is_some() { focus.2 == 0 } else { focus.3 == 0 };
                                previous.is_some_and(|p| p != focus) && first
                            });
                            let pending = ctx.data_mut(|d| d.remove_temp::<f32>(egui::Id::new("settings_pending_offset")));
                            ctx.data_mut(|d| d.insert_temp::<Option<egui::Rect>>(egui::Id::new("settings_focus_rect"), None));
                            let mut area = egui::ScrollArea::vertical()
                                .max_height(page.bottom() - ui.cursor().top() - 10.0)
                                .id_salt(("settings_content", view.nav.section, view.nav.sub))
                                .auto_shrink([false, false]);
                            if went_to_first {
                                ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("settings_follow")));
                                area = area.vertical_scroll_offset(0.0);
                            } else if let Some(offset) = pending {
                                area = area.vertical_scroll_offset(offset);
                            }
                            let output = area.show(ui, |ui| {
                                    ui.set_width(ui.available_width() - SCROLLBAR_RESERVE);
                                    content(ui, &view, &mut commands);
                                    ui.add_space(14.0);
                                });
                            follow_focus(ctx, &output);
                        },
                    );
                });
            });
        });
    commands
}

/// Moves the scroll straight to the focused row (no animation, so holding ↑/↓ never lags behind
/// and ↑ can't get stuck): the offset is set for the next frame until the row is on screen.
fn follow_focus<T>(ctx: &egui::Context, output: &egui::containers::scroll_area::ScrollAreaOutput<T>) {
    let follow = egui::Id::new("settings_follow");
    if !ctx.data(|d| d.get_temp::<bool>(follow)).unwrap_or(false) {
        return;
    }
    let Some(row) = ctx.data(|d| d.get_temp::<Option<egui::Rect>>(egui::Id::new("settings_focus_rect"))).flatten() else { return };
    let view = output.inner_rect;
    let margin = 10.0;
    let delta = if row.top() < view.top() + margin {
        row.top() - (view.top() + margin)
    } else if row.bottom() > view.bottom() - margin {
        row.bottom() - (view.bottom() - margin)
    } else {
        0.0
    };
    ctx.data_mut(|d| {
        if delta.abs() < 0.5 {
            d.remove_temp::<bool>(follow);
        } else {
            d.insert_temp(egui::Id::new("settings_pending_offset"), (output.state.offset.y + delta).max(0.0));
        }
    });
    ctx.request_repaint();
}

fn header(ui: &mut egui::Ui, lang: Language, commands: &mut Vec<AppCommand>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEADER_HEIGHT), egui::Sense::hover());
    let back = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 15.0), egui::vec2(30.0, 30.0));
    let response = ui.interact(back, ui.id().with("settings_back"), egui::Sense::click());
    ui.painter().rect_filled(back, 15.0, pal().text.gamma_multiply(if response.hovered() { 0.14 } else { 0.08 }));
    let icon_rect = egui::Rect::from_center_size(back.center(), egui::Vec2::splat(16.0));
    chrome_icons::paint(ui, ChromeIcon::ChevronLeft, icon_rect, if response.hovered() { pal().text } else { pal().text_dim });
    if response.clicked() {
        commands.push(AppCommand::CloseSettings);
    }
    let icon = egui::Rect::from_center_size(egui::pos2(back.right() + 22.0, rect.center().y), egui::Vec2::splat(20.0));
    chrome_icons::paint(ui, ChromeIcon::Settings, icon, pal().accent);
    ui.painter().text(
        egui::pos2(icon.right() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title_case(lang.settings_title()),
        font(FONT_HEADLINE),
        pal().text,
    );
}

fn rail(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    ui.spacing_mut().item_spacing.y = 6.0;
    for (index, (section, _)) in SECTIONS.iter().enumerate() {
        let active = index == view.nav.section;
        let focused = active && view.nav.rail;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(RAIL_WIDTH, 42.0), egui::Sense::click());
        let radius = rect.height() / 2.0;
        // Active section = solid pill; the ring only shows while the rail has focus.
        let (fill, text) = if active {
            (pal().primary, egui::Color32::WHITE)
        } else if response.hovered() {
            (pal().text.gamma_multiply(0.08), pal().text)
        } else {
            (egui::Color32::TRANSPARENT, pal().text_dim)
        };
        ui.painter().rect_filled(rect, radius, fill);
        if focused {
            ui.painter().rect_stroke(rect.expand(3.0), radius + 3.0, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
        }
        let icon = egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Vec2::splat(18.0));
        chrome_icons::paint(ui, section_icon(*section), icon, text);
        ui.painter().text(
            egui::pos2(rect.left() + 46.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            section_label(*section, view.lang),
            font(FONT_BODY),
            text,
        );
        if response.clicked() {
            commands.push(AppCommand::SettingsSection(index));
        }
    }
}

fn content(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    ui.spacing_mut().item_spacing.y = 8.0;
    let lang = view.lang;
    let nav = view.nav;
    match nav.sub {
        Some(SubPage::Language) => {
            sub_title(ui, lang.language_label(), None);
            for (index, language) in Language::ALL.into_iter().enumerate() {
                if radio_row(ui, language.label(), None, language == lang, index == nav.sub_index) {
                    commands.push(AppCommand::SetLanguage(language));
                }
            }
            ui.add_space(24.0);
            return;
        }
        Some(SubPage::Theme) => {
            sub_title(ui, lang.settings_theme(), Some(lang.settings_theme_preview()));
            for (index, theme) in ThemeId::ALL.into_iter().enumerate() {
                if theme_row(ui, theme, theme_label(theme, lang), theme == nav.theme_before, index == nav.sub_index) {
                    commands.push(AppCommand::SetTheme(theme));
                }
            }
            ui.add_space(24.0);
            return;
        }
        Some(SubPage::Plugins) => {
            plugin_list(ui, view, commands);
            return;
        }
        Some(SubPage::Config) => {
            config_view(ui, view);
            return;
        }
        Some(SubPage::Backups) => {
            backup_list(ui, view, commands);
            return;
        }
        Some(SubPage::Profiles) => {
            profile_list(ui, view, commands);
            return;
        }
        None => {}
    }
    let section = SECTIONS[nav.section].0;
    section_title(ui, section_label(section, lang));
    if section == Section::Storage {
        storage_card(ui, lang, view.stats);
        ui.add_space(4.0);
    }
    if section == Section::Plugins {
        tai_card(ui, lang, &view.plugins);
        ui.add_space(4.0);
    }
    if section == Section::About {
        about_card(ui, lang, view.catalog_count);
        return;
    }
    if section == Section::Account {
        account_section(ui, view, commands);
        return;
    }
    for (index, row) in nav.rows().iter().enumerate() {
        let focused = !nav.rail && index == nav.row;
        let clicked = match row {
            Row::Language => setting_row(ui, ChromeIcon::Languages, lang.language_label(), lang.settings_language_hint(), focused, false, |ui| {
                chevron(ui);
                value(ui, lang.label());
            }),
            Row::Notifications => setting_row(ui, ChromeIcon::Bell, lang.settings_install_notifications(), lang.settings_notifications_hint(), focused, false, |ui| {
                toggle(ui, view.install_notifications);
            }),
            Row::SoundEffects => setting_row(ui, ChromeIcon::Sliders, lang.settings_sound_effects(), lang.settings_sound_effects_hint(), focused, false, |ui| {
                toggle(ui, view.sound_effects);
            }),
            Row::Theme => setting_row(ui, ChromeIcon::Palette, lang.settings_theme(), lang.settings_theme_hint(), focused, false, |ui| {
                chevron(ui);
                value(ui, theme_label(active_theme(), lang));
                swatch(ui, active_theme().palette(), egui::vec2(34.0, 22.0));
            }),
            Row::ClearIcons | Row::ClearCatalog | Row::PurgeAll => {
                let (icon, label, hint) = match row {
                    Row::ClearIcons => (ChromeIcon::Image, lang.settings_clear_icons(), lang.settings_clear_icons_hint()),
                    Row::ClearCatalog => (ChromeIcon::Database, lang.settings_clear_catalog(), lang.settings_clear_catalog_hint()),
                    _ => (ChromeIcon::Trash, lang.settings_purge_all(), lang.settings_purge_hint()),
                };
                let armed = *row == Row::PurgeAll && view.purge_armed;
                let notice = view.cache_notice.filter(|_| focused && !armed);
                setting_row(ui, icon, label, hint, focused, *row == Row::PurgeAll, |ui| {
                    if armed {
                        pill(ui, lang.settings_purge_confirm(), pal().danger, None);
                    } else if let Some(notice) = notice {
                        pill(ui, notice, pal().success, Some(ChromeIcon::Check));
                    }
                })
            }
            Row::PluginManager | Row::ConfigViewer | Row::ConfigBackups | Row::Profiles => {
                let tai = view.plugins.tai;
                let (icon, label, hint, count) = match row {
                    Row::PluginManager => (
                        ChromeIcon::Plugins,
                        lang.tr("settings-plugin-manager"),
                        lang.tr("settings-plugin-manager-hint"),
                        tai.map(|t| t.plugin_count().to_string()),
                    ),
                    Row::ConfigViewer => (
                        ChromeIcon::Info,
                        lang.tr("settings-config-viewer"),
                        lang.tr("settings-config-viewer-hint"),
                        tai.map(|t| t.path.display().to_string()),
                    ),
                    Row::Profiles => (
                        ChromeIcon::Sliders,
                        lang.tr("settings-profiles"),
                        lang.tr("settings-profiles-hint"),
                        tai.map(|t| t.profiles.len().to_string()),
                    ),
                    _ => (
                        ChromeIcon::Database,
                        lang.tr("settings-config-backups"),
                        lang.tr("settings-config-backups-hint"),
                        tai.map(|t| t.backups.len().to_string()),
                    ),
                };
                setting_row(ui, icon, &label, &hint, focused, false, |ui| {
                    chevron(ui);
                    if let Some(count) = count {
                        value(ui, &count);
                    }
                })
            }
            Row::SafeMode => {
                let on = view.plugins.tai.is_some_and(Snapshot::safe_mode);
                setting_row(ui, ChromeIcon::Alert, &lang.tr("settings-safe-mode"), &lang.tr("settings-safe-mode-hint"), focused, false, |ui| {
                    toggle(ui, on);
                })
            }
            Row::MigrateConfig => {
                let both = view.plugins.tai.is_some_and(|t| t.both_exist);
                setting_row(ui, ChromeIcon::Storage, &lang.tr("settings-migrate-config"), &lang.tr("settings-migrate-config-hint"), focused, false, |ui| {
                    if both {
                        pill(ui, &lang.tr("tai-both-configs-short"), pal().warning, Some(ChromeIcon::Alert));
                    } else {
                        value(ui, &lang.tr("tai-migrate-not-needed"));
                    }
                })
            }
            Row::Reboot => {
                let plugins = &view.plugins;
                setting_row(ui, ChromeIcon::Recent, &lang.tr("settings-reboot"), &lang.tr("settings-reboot-hint"), focused, true, |ui| {
                    if plugins.reboot_armed {
                        pill(ui, &lang.tr("settings-reboot-confirm"), pal().danger, None);
                    } else if let Some(notice) = plugins.notice.filter(|_| focused) {
                        pill(ui, notice, pal().success, Some(ChromeIcon::Check));
                    } else if plugins.pending_reboot {
                        pill(ui, &lang.tr("tai-pending-reboot-short"), pal().warning, None);
                    }
                })
            }
            _ => false,
        };
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
}

fn account_section(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    let lang = view.lang;
    let nav = view.nav;
    let auth = crate::data::auth::current();

    if let Some(status) = view.auth_status {
        let (msg, is_err) = status;
        let color = if *is_err { pal().danger } else { pal().success };
        let icon = if *is_err { ChromeIcon::Alert } else { ChromeIcon::Check };
        ui.horizontal(|ui| {
            pill(ui, msg, color, Some(icon));
        });
        ui.add_space(4.0);
    }

    if let Some(session) = &auth {
        // Logged in view card
        let card_frame = egui::Frame::NONE
            .fill(glass(pal().bg_header))
            .corner_radius(RADIUS_MD)
            .stroke(egui::Stroke::new(1.0_f32, pal().glass_edge))
            .inner_margin(16.0);

        card_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Connected Account").size(FONT_BODY).color(pal().text_dim));
                ui.add_space(8.0);

                ui.label(egui::RichText::new(&session.username).size(FONT_TITLE + 4.0).strong().color(pal().text));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Online green dot
                    let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 3.5, egui::Color32::from_rgb(34, 197, 94));
                    ui.label(egui::RichText::new("Online").size(FONT_SMALL).color(egui::Color32::from_rgb(34, 197, 94)).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Connected to VitaForge").size(FONT_SMALL).color(pal().text_dim));
                });

                ui.add_space(3.0);
                ui.label(egui::RichText::new(format!("User ID: {}", session.user_id)).size(FONT_SMALL).color(pal().text_faint));
            });
        });

        ui.add_space(12.0);

        // Red bordered "Log out" button
        let focused = !nav.rail && nav.row == 6; // Logout row index
        let logout_height = 54.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), logout_height), egui::Sense::click());
        let bg_fill = if response.hovered() {
            pal().danger.gamma_multiply(0.18)
        } else {
            pal().danger.gamma_multiply(0.10)
        };
        let stroke_color = if focused {
            pal().focus
        } else {
            pal().danger.gamma_multiply(0.45)
        };
        ui.painter().rect_filled(rect, RADIUS_MD, bg_fill);
        ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(1.0_f32, stroke_color), egui::StrokeKind::Inside);
        if focused {
            ui.painter().rect_stroke(rect.expand(2.0), RADIUS_MD + 2.0, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
        }

        // Center content: "Log out" + icon, subtitle "Disconnect your VitaForge account from this console"
        ui.painter().text(
            egui::pos2(rect.center().x, rect.center().y - 8.0),
            egui::Align2::CENTER_CENTER,
            "Log out",
            font(FONT_BODY),
            pal().danger,
        );
        let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x + 36.0, rect.center().y - 8.0), egui::Vec2::splat(15.0));
        chrome_icons::paint(ui, ChromeIcon::Trash, icon_rect, pal().danger);

        ui.painter().text(
            egui::pos2(rect.center().x, rect.center().y + 10.0),
            egui::Align2::CENTER_CENTER,
            &lang.tr("auth-logout-hint"),
            font(FONT_SMALL),
            pal().text_dim,
        );

        if response.clicked() {
            commands.push(AppCommand::AuthLogout);
        }
    } else {
        // Form: username, password, email (if register), submit button, toggle mode
        let form = view.auth_form;
        let is_reg = form.is_register;

        // Mode header / status
        let mode_desc = if is_reg {
            lang.tr("auth-register-desc")
        } else {
            lang.tr("auth-login-desc")
        };
        ui.label(egui::RichText::new(mode_desc).size(FONT_SMALL).color(pal().text_dim));
        ui.add_space(4.0);

        // Row 1: Username
        let focused_u = !nav.rail && nav.row == 1;
        let u_val = if form.username.is_empty() { lang.tr("auth-tap-to-enter") } else { form.username.clone() };
        if auth_input_row(ui, ChromeIcon::User, &lang.tr("auth-username"), &lang.tr("auth-username-hint"), &u_val, focused_u) {
            commands.push(AppCommand::RequestAuthInput(crate::input::TextTarget::AuthUsername));
        }

        ui.add_space(2.0);

        // Row 2: Password
        let focused_p = !nav.rail && nav.row == 2;
        let p_val = if form.password.is_empty() {
            lang.tr("auth-tap-to-enter")
        } else {
            "•".repeat(form.password.len().min(16))
        };
        if auth_input_row(ui, ChromeIcon::Alert, &lang.tr("auth-password"), &lang.tr("auth-password-hint"), &p_val, focused_p) {
            commands.push(AppCommand::RequestAuthInput(crate::input::TextTarget::AuthPassword));
        }

        // Row 3: Email (only if register)
        if is_reg {
            ui.add_space(2.0);
            let focused_e = !nav.rail && nav.row == 3;
            let e_val = if form.email.is_empty() { lang.tr("auth-optional") } else { form.email.clone() };
            if auth_input_row(ui, ChromeIcon::Info, &lang.tr("auth-email"), &lang.tr("auth-email-hint"), &e_val, focused_e) {
                commands.push(AppCommand::RequestAuthInput(crate::input::TextTarget::AuthEmail));
            }
        }

        ui.add_space(6.0);

        // Row 4: Submit action (blue full-width button)
        let focused_s = !nav.rail && nav.row == 4;
        let submit_title = if view.auth_busy {
            lang.tr("auth-connecting")
        } else if is_reg {
            lang.tr("auth-register-action")
        } else {
            lang.tr("auth-login-action")
        };
        if auth_button_row(ui, &submit_title, Some(ChromeIcon::Check), view.auth_busy, focused_s) {
            commands.push(AppCommand::AuthSubmit);
        }

        ui.add_space(6.0);

        // Row 5: Toggle login / register (footer link)
        let focused_t = !nav.rail && nav.row == 5;
        let toggle_title = if is_reg {
            lang.tr("auth-switch-to-login")
        } else {
            lang.tr("auth-switch-to-register")
        };
        if auth_link_row(ui, ChromeIcon::User, &toggle_title, focused_t) {
            commands.push(AppCommand::AuthToggleMode);
        }
    }
}

fn auth_input_row(
    ui: &mut egui::Ui,
    icon: ChromeIcon,
    title: &str,
    hint: &str,
    value_text: &str,
    focused: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::click());
    let lift = if focused { 0.12 } else if response.hovered() { 0.09 } else { 0.06 };
    ui.painter().rect_filled(rect, RADIUS_MD, pal().text.gamma_multiply(lift));
    if focused {
        ui.painter().rect_stroke(rect.expand(2.0), RADIUS_MD + 2.0, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 28.0, rect.center().y), egui::Vec2::splat(18.0));
    chrome_icons::paint(ui, icon, icon_rect, pal().text);

    let text_x = rect.left() + 52.0;
    ui.painter().text(
        egui::pos2(text_x, rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        title,
        font(FONT_BODY),
        pal().text,
    );
    ui.painter().text(
        egui::pos2(text_x, rect.center().y + 11.0),
        egui::Align2::LEFT_CENTER,
        hint,
        font(FONT_SMALL),
        pal().text_dim,
    );

    // Right-hand side: Tap to enter + rounded box with Chevron >
    let chevron_box_size = egui::vec2(28.0, 28.0);
    let chevron_box_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 14.0 - chevron_box_size.x, rect.center().y - chevron_box_size.y * 0.5),
        chevron_box_size,
    );
    ui.painter().rect_filled(chevron_box_rect, 6.0, pal().text.gamma_multiply(0.08));
    chrome_icons::paint(
        ui,
        ChromeIcon::ChevronRight,
        egui::Rect::from_center_size(chevron_box_rect.center(), egui::Vec2::splat(14.0)),
        pal().text_dim,
    );

    ui.painter().text(
        egui::pos2(chevron_box_rect.left() - 12.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        value_text,
        font(FONT_SMALL),
        pal().text_dim,
    );

    response.clicked()
}

fn auth_button_row(
    ui: &mut egui::Ui,
    label: &str,
    icon: Option<ChromeIcon>,
    busy: bool,
    focused: bool,
) -> bool {
    let height = 48.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    let fill = if busy {
        pal().primary.gamma_multiply(0.7)
    } else if response.hovered() {
        pal().primary.gamma_multiply(0.9)
    } else {
        pal().primary
    };
    let radius = height / 2.0;
    ui.painter().rect_filled(rect, radius, fill);
    ui.painter().rect_stroke(rect, radius, egui::Stroke::new(1.5_f32, pal().accent_soft), egui::StrokeKind::Inside);
    if focused {
        ui.painter().rect_stroke(rect.expand(2.5), radius + 2.5, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
    }

    let mut center_x = rect.center().x;
    if let Some(ic) = icon {
        let ic_size = 16.0;
        let ic_rect = egui::Rect::from_center_size(egui::pos2(center_x - 38.0, rect.center().y), egui::Vec2::splat(ic_size));
        chrome_icons::paint(ui, ic, ic_rect, egui::Color32::WHITE);
        center_x += 8.0;
    }
    ui.painter().text(
        egui::pos2(center_x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        label,
        font(FONT_BODY),
        egui::Color32::WHITE,
    );

    response.clicked()
}

fn auth_link_row(
    ui: &mut egui::Ui,
    icon: ChromeIcon,
    label: &str,
    focused: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
    if focused {
        ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(1.5_f32, pal().focus), egui::StrokeKind::Inside);
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 24.0, rect.center().y), egui::Vec2::splat(18.0));
    let color = if response.hovered() || focused { pal().text } else { pal().text_dim };
    chrome_icons::paint(ui, icon, icon_rect, color);

    ui.painter().text(
        egui::pos2(rect.left() + 48.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_BODY),
        color,
    );

    let arrow_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - 24.0, rect.center().y), egui::Vec2::splat(16.0));
    chrome_icons::paint(ui, ChromeIcon::ChevronRight, arrow_rect, color);

    response.clicked()
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).size(FONT_TITLE).strong().color(pal().text));
    ui.add_space(2.0);
}

fn sub_title(ui: &mut egui::Ui, title: &str, hint: Option<&str>) {
    section_title(ui, title);
    if let Some(hint) = hint {
        glyph_text(ui, hint, FONT_SMALL, pal().text_faint, false);
        ui.add_space(2.0);
    }
}

/// The PlayStation button the translations spell as ✕ ○ △ □ (the UI font has no such glyphs).
fn button_glyph(c: char) -> Option<Glyph> {
    match c {
        '✕' => Some(Glyph::Cross),
        '○' => Some(Glyph::Circle),
        '△' => Some(Glyph::Triangle),
        '□' => Some(Glyph::Square),
        _ => None,
    }
}

/// One line of text where ✕ ○ △ □ are drawn with the bundled button icons.
fn glyph_text(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32, strong: bool) {
    // Translations name the buttons as [cross] [circle] [triangle] [square], never as symbols.
    let text = text.replace("[cross]", "✕").replace("[circle]", "○").replace("[triangle]", "△").replace("[square]", "□");
    let text = text.as_str();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let mut run = String::new();
        let flush = |ui: &mut egui::Ui, run: &mut String| {
            if !run.is_empty() {
                let mut label = egui::RichText::new(std::mem::take(run)).size(size).color(color);
                if strong {
                    label = label.strong();
                }
                ui.label(label);
            }
        };
        for c in text.chars() {
            match button_glyph(c) {
                Some(glyph) => {
                    flush(ui, &mut run);
                    let side = size + 2.0;
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
                    if let Some(texture) = button_texture(ui.ctx(), glyph) {
                        ui.painter().image(
                            texture.id(),
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
                None => run.push(c),
            }
        }
        flush(ui, &mut run);
    });
}

/// Icon, title and one-line hint on the left; `control` lays out right-to-left.
fn setting_row(
    ui: &mut egui::Ui,
    icon: ChromeIcon,
    title: &str,
    hint: &str,
    focused: bool,
    destructive: bool,
    control: impl FnOnce(&mut egui::Ui),
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::click());
    let lift = if focused { 0.12 } else if response.hovered() { 0.09 } else { 0.06 };
    ui.painter().rect_filled(rect, RADIUS_MD, pal().text.gamma_multiply(lift));
    if focused {
        ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
        scroll_on_focus_change(ui, &response);
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("test_focus_rect"), (rect, ui.clip_rect())));
    }
    let title_color = if destructive { pal().danger } else { pal().text };
    let icon_color = if destructive { pal().danger } else if focused { pal().text } else { pal().text_dim };
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Vec2::splat(18.0));
    chrome_icons::paint(ui, icon, icon_rect, icon_color);
    let text_x = rect.left() + 50.0;
    let control_rect = egui::Rect::from_min_max(egui::pos2(rect.center().x, rect.top()), egui::pos2(rect.right() - 14.0, rect.bottom()));
    // A child ui draws inside the row without taking space in the list layout.
    let mut control_ui = ui.new_child(
        egui::UiBuilder::new().max_rect(control_rect).layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    control(&mut control_ui);
    // The hint stops short of whatever the control drew, ending in "…" when it doesn't fit.
    let used = control_ui.min_rect();
    let hint_right = if used.width() > 0.0 { used.left() - 12.0 } else { rect.right() - 14.0 };
    let mut title_job = egui::text::LayoutJob::simple_singleline(title.to_owned(), font(FONT_BODY), title_color);
    title_job.wrap = egui::text::TextWrapping { max_width: (hint_right - text_x).max(0.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let title_galley = ui.painter().layout_job(title_job);
    ui.painter().galley(egui::pos2(text_x, rect.center().y - 9.0 - title_galley.size().y / 2.0), title_galley, title_color);
    let mut job = egui::text::LayoutJob::simple_singleline(hint.to_owned(), font(FONT_MICRO), pal().text_faint);
    job.wrap = egui::text::TextWrapping { max_width: (hint_right - text_x).max(0.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(egui::pos2(text_x, rect.center().y + 10.0 - galley.size().y / 2.0), galley, pal().text_faint);
    response.clicked()
}

/// Remembers where the focused row is and asks `settings_page` to bring it into view, but only
/// when focus moved, so a manual scroll isn't yanked back every frame.
fn scroll_on_focus_change(ui: &egui::Ui, response: &egui::Response) {
    let key = egui::Id::new("settings_last_focus");
    ui.ctx().data_mut(|d| {
        d.insert_temp(egui::Id::new("settings_focus_rect"), Some(response.rect));
        if d.get_temp::<egui::Id>(key) != Some(response.id) {
            d.insert_temp(egui::Id::new("settings_follow"), true);
        }
        d.insert_temp(key, response.id);
    });
}

fn value(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(FONT_SMALL).color(pal().text_dim));
}

fn chevron(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
    chrome_icons::paint(ui, ChromeIcon::ChevronRight, rect, pal().text_faint);
}

/// Tinted status pill: colored text on a faint fill of the same color.
fn pill(ui: &mut egui::Ui, label: &str, color: egui::Color32, icon: Option<ChromeIcon>) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.4)))
        .corner_radius(11.0)
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if let Some(icon) = icon {
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                    chrome_icons::paint(ui, icon, rect, color);
                }
                glyph_text(ui, label, FONT_MICRO, color, true);
            });
        });
}

fn toggle(ui: &mut egui::Ui, on: bool) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 24.0), egui::Sense::hover());
    let t = ui.ctx().animate_bool_with_time(response.id, on, 0.12);
    let radius = rect.height() / 2.0;
    ui.painter().rect_filled(rect, radius, pal().separator.lerp_to_gamma(pal().primary, t));
    let x = egui::lerp((rect.min.x + radius)..=(rect.max.x - radius), t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, egui::Color32::WHITE);
}

/// Miniature of a palette: page, a card, the accent and the primary button.
fn swatch(ui: &mut egui::Ui, p: &Palette, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS_XS, p.bg_deep);
    painter.rect_stroke(rect, RADIUS_XS, egui::Stroke::new(1.0_f32, p.separator), egui::StrokeKind::Inside);
    let card = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(rect.width() * 0.55, rect.height() - 8.0));
    painter.rect_filled(card, 3.0, p.bg_card);
    painter.circle_filled(egui::pos2(card.left() + 5.0, card.top() + 5.0), 2.5, p.accent);
    let button = egui::Rect::from_min_max(egui::pos2(card.right() + 3.0, rect.bottom() - 9.0), rect.right_bottom() - egui::vec2(4.0, 4.0));
    painter.rect_filled(button, 2.5, p.primary);
}

fn theme_row(ui: &mut egui::Ui, theme: ThemeId, name: &str, saved: bool, focused: bool) -> bool {
    const THEME_ROW_HEIGHT: f32 = 40.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), THEME_ROW_HEIGHT), egui::Sense::click());
    let lift = if focused { 0.12 } else if response.hovered() { 0.09 } else { 0.06 };
    ui.painter().rect_filled(rect, RADIUS_MD, pal().text.gamma_multiply(lift));
    if focused {
        ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
        scroll_on_focus_change(ui, &response);
    }
    let preview = egui::Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.top() + 7.0), egui::vec2(42.0, THEME_ROW_HEIGHT - 14.0));
    swatch(&mut ui.new_child(egui::UiBuilder::new().max_rect(preview)), theme.palette(), preview.size());
    ui.painter().text(
        egui::pos2(preview.right() + 16.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        font(FONT_BODY),
        if focused || saved { pal().text } else { pal().text_dim },
    );
    if saved {
        let check = egui::Rect::from_center_size(egui::pos2(rect.right() - 22.0, rect.center().y), egui::Vec2::splat(16.0));
        chrome_icons::paint(ui, ChromeIcon::Check, check, pal().success);
    }
    response.clicked()
}

/// Cache breakdown as one stacked bar with a legend, plus free space on ux0.
fn storage_card(ui: &mut egui::Ui, lang: Language, stats: &CacheStats) {
    egui::Frame::new()
        .fill(pal().text.gamma_multiply(0.06))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(lang.settings_cache()).size(FONT_SMALL).color(pal().text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format_bytes(stats.total_bytes)).size(FONT_TITLE).strong().color(pal().text));
                });
            });
            ui.add_space(6.0);
            let parts = [
                (lang.settings_icon_cache(), stats.icons_bytes, pal().primary),
                (lang.settings_catalog_cache(), stats.catalog_bytes, pal().accent),
                (lang.settings_hash_cache(), stats.hashes_bytes, pal().text_faint),
            ];
            let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 10.0), egui::Sense::hover());
            ui.painter().rect_filled(bar, 5.0, pal().text.gamma_multiply(0.08));
            let total = parts.iter().map(|p| p.1).sum::<u64>().max(1) as f32;
            let mut x = bar.left();
            for (_, bytes, color) in parts {
                let w = bar.width() * bytes as f32 / total;
                if w >= 1.0 {
                    let seg = egui::Rect::from_min_max(egui::pos2(x, bar.top()), egui::pos2(x + w, bar.bottom()));
                    ui.painter().rect_filled(seg, 5.0, color);
                }
                x += w;
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (label, bytes, color) in parts {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (dot, _) = ui.allocate_exact_size(egui::Vec2::splat(8.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, color);
                        ui.label(egui::RichText::new(label).size(FONT_MICRO).color(pal().text_dim));
                        ui.label(egui::RichText::new(format_bytes(bytes)).size(FONT_MICRO).strong().color(pal().text));
                    });
                }
            });
            // sysinfo::storage returns (used, total).
            if let Some((used, total)) = crate::app::sysinfo::storage("ux0:") {
                let free = total.saturating_sub(used);
                ui.add_space(6.0);
                ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top() - 3.0, egui::Stroke::new(1.0_f32, pal().separator));
                ui.label(
                    egui::RichText::new(format!("{}: {}", lang.settings_free_space(), format_bytes(free)))
                        .size(FONT_MICRO)
                        .color(pal().text_faint),
                );
            }
        });
}

/// Which config.txt is live, how many plugins it loads, and what needs attention.
fn tai_card(ui: &mut egui::Ui, lang: Language, plugins: &PluginsView) {
    egui::Frame::new()
        .fill(pal().text.gamma_multiply(0.06))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(tai) = plugins.tai else {
                ui.label(egui::RichText::new(lang.tr("tai-reading")).size(FONT_SMALL).color(pal().text_dim));
                return;
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(lang.tr("tai-active-config")).size(FONT_SMALL).color(pal().text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let count = tai.plugin_count().to_string();
                    ui.label(
                        egui::RichText::new(lang.tr_args("tai-plugins-count", &[("count", &count)]))
                            .size(FONT_SMALL)
                            .strong()
                            .color(pal().text),
                    );
                });
            });
            ui.label(egui::RichText::new(tai.path.display().to_string()).size(FONT_TITLE).strong().color(pal().text));
            let mut warn = |text: String, color: egui::Color32| {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                    chrome_icons::paint(ui, ChromeIcon::Alert, rect, color);
                    ui.label(egui::RichText::new(text).size(FONT_MICRO).color(color));
                });
            };
            warn(lang.tr("tai-beta"), pal().warning);
            if let Some(error) = &tai.error {
                warn(error.clone(), pal().danger);
            }
            if tai.both_exist {
                warn(lang.tr("tai-both-configs"), pal().warning);
            }
            if tai.safe_mode() {
                warn(lang.tr("tai-safe-on"), pal().warning);
            }
            if plugins.pending_reboot {
                warn(lang.tr("tai-pending-reboot"), pal().accent);
            }
        });
}

fn list_hint(ui: &mut egui::Ui, text: &str) {
    glyph_text(ui, text, FONT_SMALL, pal().text_faint, false);
    ui.add_space(2.0);
}

/// Plugin lines grouped under their `*SECTION` header. ✕ switches one on or
/// off, □ picks it up to reorder, △ removes it (twice).
fn plugin_list(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    let lang = view.lang;
    let nav = view.nav;
    let plugins = &view.plugins;
    section_title(ui, &lang.tr("settings-plugin-manager"));
    let hint = if nav.moving { lang.tr("tai-move-hint") } else { lang.tr("tai-list-hint") };
    list_hint(ui, &hint);
    let Some(tai) = plugins.tai else { return };
    if tai.entries.is_empty() {
        list_hint(ui, &lang.tr("tai-no-plugins"));
        return;
    }
    let mut section: Option<&str> = None;
    for (index, entry) in tai.entries.iter().enumerate() {
        if section != entry.section.as_deref() || index == 0 {
            section = entry.section.as_deref();
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(section.unwrap_or("—"))
                    .size(FONT_SMALL)
                    .strong()
                    .monospace()
                    .color(pal().accent),
            );
        }
        let focused = index == nav.sub_index;
        let record = tai.registry.owner_of(&entry.path);
        let title = tai.label(entry);
        let mut detail = entry.path.clone();
        if let Some(version) = record.map(|r| r.version.as_str()).filter(|v| !v.is_empty()) {
            detail = format!("{detail}  ·  {version}");
        }
        let icon = if entry.protected { ChromeIcon::Check } else { ChromeIcon::Plugins };
        let missing = tai.missing.get(index).copied().unwrap_or(false);
        let outdated = record.is_some_and(|r| plugins.outdated.contains(&r.id));
        let clicked = setting_row(ui, icon, &title, &detail, focused, false, |ui| {
            if entry.protected {
                pill(ui, &lang.tr("tai-protected"), pal().text_faint, None);
            } else if focused && nav.moving {
                pill(ui, &lang.tr("tai-moving"), pal().accent, None);
            } else if focused && plugins.armed {
                pill(ui, &lang.tr("tai-remove-confirm"), pal().danger, Some(ChromeIcon::Trash));
            } else {
                toggle(ui, entry.enabled);
            }
            if missing {
                pill(ui, &lang.tr("tai-missing-file"), pal().danger, Some(ChromeIcon::Alert));
            }
            if outdated {
                pill(ui, &lang.tr("tai-update-available"), pal().warning, Some(ChromeIcon::UpdateAvailable));
            }
            if record.is_none() && !entry.protected {
                value(ui, &lang.tr("tai-manual"));
            }
        });
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
    if let Some(notice) = plugins.notice {
        ui.add_space(6.0);
        list_hint(ui, notice);
    }
    ui.add_space(24.0);
}

/// config.txt exactly as taiHEN reads it, with line numbers.
fn config_view(ui: &mut egui::Ui, view: &SettingsView) {
    let lang = view.lang;
    let Some(tai) = view.plugins.tai else { return };
    sub_title(ui, &tai.path.display().to_string(), Some(&lang.tr("settings-config-viewer-hint")));
    // Each line paints its own slice of the dark panel: a Frame's background came out unclipped
    // at the footer, a row painted with the same painter as its text always clips with it.
    ui.spacing_mut().item_spacing.y = 0.0;
    let total = tai.config.raw_lines().count();
    for (number, (text, style)) in tai.config.raw_lines().enumerate() {
        let color = match style {
            LineStyle::Header => pal().accent,
            LineStyle::Entry => pal().text,
            LineStyle::Disabled => pal().warning,
            LineStyle::Comment => pal().text_faint,
        };
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), FONT_SMALL + 8.0), egui::Sense::hover());
        let r = RADIUS_MD as u8;
        let rounding = egui::CornerRadius {
            nw: if number == 0 { r } else { 0 },
            ne: if number == 0 { r } else { 0 },
            sw: if number + 1 == total { r } else { 0 },
            se: if number + 1 == total { r } else { 0 },
        };
        ui.painter().rect_filled(rect, rounding, pal().bg_deep);
        ui.painter().text(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{:>3}", number + 1),
            egui::FontId::monospace(FONT_MICRO),
            pal().text_faint,
        );
        // One row per line: a long line ends in "…" instead of running off the screen.
        let text_x = rect.left() + 46.0;
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), egui::FontId::monospace(FONT_SMALL), color);
        job.wrap = egui::text::TextWrapping {
            max_width: (rect.right() - 12.0 - text_x).max(0.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(egui::pos2(text_x, rect.center().y - galley.size().y / 2.0), galley, color);
        if number == view.nav.sub_index {
            ui.painter().rect_stroke(rect.shrink(1.0), RADIUS_XS, egui::Stroke::new(1.0_f32, pal().focus), egui::StrokeKind::Inside);
            scroll_on_focus_change(ui, &response);
        }
    }
    ui.add_space(24.0);
}

fn backup_list(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    let lang = view.lang;
    let plugins = &view.plugins;
    sub_title(ui, &lang.tr("settings-config-backups"), Some(&lang.tr("tai-backups-hint")));
    let Some(tai) = plugins.tai else { return };
    if tai.backups.is_empty() {
        list_hint(ui, &lang.tr("tai-no-backups"));
        return;
    }
    for (index, backup) in tai.backups.iter().enumerate() {
        let focused = index == view.nav.sub_index;
        let reason = if backup.reason.is_empty() { "—".to_owned() } else { backup.reason.replace('-', " ") };
        let clicked = setting_row(ui, ChromeIcon::Database, &backup.label(), &reason, focused, false, |ui| {
            if focused && plugins.armed {
                pill(ui, &lang.tr("tai-restore-confirm"), pal().danger, None);
            } else if let Some(notice) = plugins.notice.filter(|_| focused) {
                pill(ui, notice, pal().success, Some(ChromeIcon::Check));
            }
        });
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
    ui.add_space(24.0);
}

/// Row 0 saves the current setup; the rows below are the saved profiles.
fn profile_list(ui: &mut egui::Ui, view: &SettingsView, commands: &mut Vec<AppCommand>) {
    let lang = view.lang;
    let plugins = &view.plugins;
    sub_title(ui, &lang.tr("settings-profiles"), Some(&lang.tr("tai-profiles-hint")));
    let Some(tai) = plugins.tai else { return };
    let focused = view.nav.sub_index == 0;
    let clicked = setting_row(ui, ChromeIcon::Check, &lang.tr("tai-profile-save"), &lang.tr("tai-profile-save-hint"), focused, false, |ui| {
        if let Some(notice) = plugins.notice.filter(|_| focused) {
            pill(ui, notice, pal().success, Some(ChromeIcon::Check));
        }
    });
    if clicked {
        commands.push(AppCommand::SettingsRow(0));
    }
    if tai.profiles.is_empty() {
        list_hint(ui, &lang.tr("tai-no-profiles"));
    }
    for (i, profile) in tai.profiles.iter().enumerate() {
        let index = i + 1;
        let focused = index == view.nav.sub_index;
        let on = profile.entries.iter().filter(|e| e.enabled).count().to_string();
        let clicked = setting_row(ui, ChromeIcon::Plugins, &profile.name, &lang.tr_args("tai-profile-count", &[("count", &on)]), focused, false, |ui| {
            if focused && plugins.armed {
                pill(ui, &lang.tr("tai-profile-confirm"), pal().danger, None);
            } else if let Some(notice) = plugins.notice.filter(|_| focused) {
                pill(ui, notice, pal().success, Some(ChromeIcon::Check));
            }
        });
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
    for (i, (_, profile)) in tai.importable.iter().enumerate() {
        let index = tai.profiles.len() + 1 + i;
        let focused = index == view.nav.sub_index;
        let clicked = setting_row(ui, ChromeIcon::Storage, &profile.name, &lang.tr("tai-profile-import-hint"), focused, false, |ui| {
            if let Some(notice) = plugins.notice.filter(|_| focused) {
                pill(ui, notice, pal().success, Some(ChromeIcon::Check));
            }
        });
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
    ui.add_space(24.0);
}

fn about_card(ui: &mut egui::Ui, lang: Language, catalog_count: usize) {
    egui::Frame::new()
        .fill(pal().text.gamma_multiply(0.06))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let logo = logo_texture(ui.ctx());
                ui.add(egui::Image::new(&logo).fit_to_exact_size(egui::Vec2::splat(48.0)).corner_radius(RADIUS_SM));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("VitaForge").size(FONT_TITLE).strong().color(pal().text));
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}  ·  {} {}",
                            lang.settings_version(),
                            env!("CARGO_PKG_VERSION"),
                            lang.settings_catalog(),
                            catalog_count
                        ))
                        .size(FONT_SMALL)
                        .color(pal().text_dim),
                    );
                });
            });
        });
    ui.add_space(4.0);
    ui.label(egui::RichText::new(lang.settings_credits()).size(FONT_SMALL).strong().color(pal().text_dim));
    ui.label(egui::RichText::new(lang.credits_body()).size(FONT_SMALL).color(pal().text_faint));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_pages_render() {
        let store = crate::install::tai::store::temp_store("settings-render");
        std::fs::write(&store.ur0, "*KERNEL\nur0:tai/a.skprx\n# ur0:tai/b.skprx\n*main\nur0:tai/henkaku.suprx\n").unwrap();
        std::fs::write(&store.ux0, "*KERNEL\n").unwrap();
        let snapshot = Snapshot::read(&store, &[]);
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let stats = CacheStats::default();
        let plugins_section = SECTIONS.iter().position(|(s, _)| *s == Section::Plugins).unwrap();
        for sub in [None, Some(SubPage::Plugins), Some(SubPage::Config), Some(SubPage::Backups), Some(SubPage::Profiles)] {
            for (moving, armed) in [(false, false), (true, false), (false, true)] {
                let mut nav = SettingsNav::new();
                nav.go_section(plugins_section);
                nav.sub = sub;
                nav.moving = moving;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    settings_page(
                        ctx,
                        SettingsView {
                            lang: Language::Spanish,
                            nav: &nav,
                            catalog_count: 0,
                            stats: &stats,
                            cache_notice: None,
                            install_notifications: true,
                            sound_effects: true,
                            purge_armed: false,
                            plugins: PluginsView {
                                tai: Some(&snapshot),
                                outdated: &[],
                                pending_reboot: true,
                                reboot_armed: armed,
                                armed,
                                notice: Some("ok"),
                            },
                            appear: 1.0,
                            auth_form: &crate::app::AuthForm::default(),
                            auth_busy: false,
                            auth_status: None,
                        },
                    );
                });
            }
        }
    }

    /// Walk focus down a long plugin list and back up: the focused row must stay on screen.
    #[test]
    fn focus_stays_visible_going_down_and_back_up() {
        let store = crate::install::tai::store::temp_store("settings-scroll");
        let mut text = String::from("*KERNEL\n");
        for i in 0..30 {
            text.push_str(&format!("ur0:tai/plugin{i}.suprx\n"));
        }
        std::fs::write(&store.ur0, text).unwrap();
        let snapshot = Snapshot::read(&store, &[]);
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let stats = CacheStats::default();
        let plugins_section = SECTIONS.iter().position(|(s, _)| *s == Section::Plugins).unwrap();
        let mut nav = SettingsNav::new();
        nav.go_section(plugins_section);
        nav.sub = Some(SubPage::Plugins);
        let mut clock = 0.0_f64;
        let steps: Vec<usize> = (0..30).chain((0..29).rev()).collect();
        for index in steps {
            nav.sub_index = index;
            for frame in 0..2 {
                clock += 0.016;
                let _ = frame;
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 544.0))),
                    time: Some(clock),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::TopBottomPanel::bottom("hints").exact_height(47.0).show(ctx, |_| {});
                    settings_page(ctx, SettingsView {
                        lang: Language::English, nav: &nav, catalog_count: 0, stats: &stats,
                        cache_notice: None, install_notifications: true, sound_effects: true, purge_armed: false,
                        plugins: PluginsView { tai: Some(&snapshot), outdated: &[], pending_reboot: false,
                            reboot_armed: false, armed: false, notice: None },
                        appear: 1.0,
                        auth_form: &crate::app::AuthForm::default(),
                        auth_busy: false,
                        auth_status: None,
                    });
                });
            }
            let (rect, clip): (egui::Rect, egui::Rect) =
                ctx.data(|d| d.get_temp(egui::Id::new("test_focus_rect"))).expect("focused row drawn");
            assert!(clip.bottom() <= 544.0 - 47.0 + 0.5, "page reaches under the footer: {clip:?}");
            assert!(rect.top() >= clip.top() - 1.0 && rect.bottom() <= clip.bottom() + 1.0,
                "row {index} not visible: row {rect:?} clip {clip:?}");
        }
    }

    #[test]
    fn every_row_lives_in_exactly_one_section() {
        for row in [
            Row::Language,
            Row::Notifications,
            Row::SoundEffects,
            Row::Theme,
            Row::ClearIcons,
            Row::ClearCatalog,
            Row::PurgeAll,
            Row::PluginManager,
            Row::ConfigViewer,
            Row::ConfigBackups,
            Row::Profiles,
            Row::SafeMode,
            Row::MigrateConfig,
            Row::Reboot,
        ] {
            let homes = SECTIONS.iter().filter(|(_, rows)| rows.contains(&row)).count();
            assert_eq!(homes, 1, "{row:?}");
        }
    }
}
