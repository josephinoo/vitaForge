use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::theme::*;
use crate::app::ui::widgets::radio_row;
use crate::data::cache_manager::{format_bytes, CacheStats};
use crate::input::AppCommand;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    General,
    Appearance,
    Storage,
    Plugins,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    Language,
    Notifications,
    Theme,
    ClearIcons,
    ClearCatalog,
    PurgeAll,
}

/// A choice list that replaces the section's rows until confirmed or backed out of.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SubPage {
    Language,
    Theme,
}

/// Sections in rail order with the focusable rows each one holds.
pub const SECTIONS: [(Section, &[Row]); 5] = [
    (Section::General, &[Row::Language, Row::Notifications]),
    (Section::Appearance, &[Row::Theme]),
    (Section::Storage, &[Row::ClearIcons, Row::ClearCatalog, Row::PurgeAll]),
    (Section::Plugins, &[]),
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
}

impl SettingsNav {
    pub fn new() -> Self {
        Self { section: 0, row: 0, rail: false, sub: None, sub_index: 0, theme_before: active_theme() }
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
            None => 0,
        }
    }
}

fn section_label(section: Section, lang: Language) -> &'static str {
    match section {
        Section::General => lang.settings_general(),
        Section::Appearance => lang.settings_appearance(),
        Section::Storage => lang.settings_storage(),
        Section::Plugins => lang.plugins_manager_label(),
        Section::About => lang.settings_about(),
    }
}

fn section_icon(section: Section) -> ChromeIcon {
    match section {
        Section::General => ChromeIcon::Sliders,
        Section::Appearance => ChromeIcon::Palette,
        Section::Storage => ChromeIcon::Storage,
        Section::Plugins => ChromeIcon::Plugins,
        Section::About => ChromeIcon::Info,
    }
}

pub fn theme_label(theme: ThemeId, lang: Language) -> &'static str {
    match theme {
        ThemeId::Midnight => lang.theme_midnight(),
        ThemeId::Obsidian => lang.theme_obsidian(),
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
    pub purge_armed: bool,
    pub appear: f32,
}

const RAIL_WIDTH: f32 = 196.0;
const ROW_HEIGHT: f32 = 56.0;

/// Full-screen settings: section rail on the left, the section's rows (or an
/// open choice list) on the right. The footer hints stay visible below it.
pub(crate) fn settings_page(ctx: &egui::Context, view: SettingsView) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let screen = ctx.screen_rect();
    // The footer panel can end up taller than HINT_BAR_HEIGHT; stop at its real top edge.
    let footer_top = egui::containers::panel::PanelState::load(ctx, egui::Id::new("hints"))
        .map_or(screen.max.y - HINT_BAR_HEIGHT, |panel| panel.rect.top());
    let page = egui::Rect::from_min_max(screen.min, egui::pos2(screen.max.x, footer_top));
    egui::Area::new(egui::Id::new("settings_page"))
        .order(egui::Order::Foreground)
        .fixed_pos(page.min)
        .show(ctx, |ui| {
            ui.set_clip_rect(page);
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
                            egui::ScrollArea::vertical()
                                .max_height(page.bottom() - ui.cursor().top())
                                .id_salt(("settings_content", view.nav.section, view.nav.sub))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width() - SCROLLBAR_RESERVE);
                                    content(ui, &view, &mut commands);
                                    ui.add_space(12.0);
                                });
                        },
                    );
                });
            });
        });
    commands
}

fn header(ui: &mut egui::Ui, lang: Language, commands: &mut Vec<AppCommand>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HEADER_HEIGHT), egui::Sense::hover());
    let back = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 15.0), egui::vec2(30.0, 30.0));
    let response = ui.interact(back, ui.id().with("settings_back"), egui::Sense::click());
    ui.painter().rect_filled(back, 15.0, pal().text.gamma_multiply(if response.hovered() { 0.14 } else { 0.08 }));
    ui.painter().text(back.center(), egui::Align2::CENTER_CENTER, "<", font(FONT_BODY), pal().text);
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
            return;
        }
        Some(SubPage::Theme) => {
            sub_title(ui, lang.settings_theme(), Some(lang.settings_theme_preview()));
            for (index, theme) in ThemeId::ALL.into_iter().enumerate() {
                if theme_row(ui, theme, theme_label(theme, lang), theme == nav.theme_before, index == nav.sub_index) {
                    commands.push(AppCommand::SetTheme(theme));
                }
            }
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
    if section == Section::About {
        about_card(ui, lang, view.catalog_count);
        return;
    }
    if section == Section::Plugins {
        super::plugins::render_plugins_manager(ui, lang, commands);
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
        };
        if clicked {
            commands.push(AppCommand::SettingsRow(index));
        }
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).size(FONT_TITLE).strong().color(pal().text));
    ui.add_space(2.0);
}

fn sub_title(ui: &mut egui::Ui, title: &str, hint: Option<&str>) {
    section_title(ui, title);
    if let Some(hint) = hint {
        ui.label(egui::RichText::new(hint).size(FONT_SMALL).color(pal().text_faint));
        ui.add_space(2.0);
    }
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
    }
    let title_color = if destructive { pal().danger } else { pal().text };
    let icon_color = if destructive { pal().danger } else if focused { pal().text } else { pal().text_dim };
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Vec2::splat(18.0));
    chrome_icons::paint(ui, icon, icon_rect, icon_color);
    let text_x = rect.left() + 50.0;
    ui.painter().text(egui::pos2(text_x, rect.center().y - 9.0), egui::Align2::LEFT_CENTER, title, font(FONT_BODY), title_color);
    ui.painter().text(egui::pos2(text_x, rect.center().y + 10.0), egui::Align2::LEFT_CENTER, hint, font(FONT_MICRO), pal().text_faint);
    let control_rect = egui::Rect::from_min_max(egui::pos2(rect.center().x, rect.top()), egui::pos2(rect.right() - 14.0, rect.bottom()));
    // A child ui draws inside the row without taking space in the list layout.
    control(&mut ui.new_child(
        egui::UiBuilder::new().max_rect(control_rect).layout(egui::Layout::right_to_left(egui::Align::Center)),
    ));
    response.clicked()
}

/// Scroll to the focused row only when focus moves, so a manual scroll isn't yanked back every frame.
fn scroll_on_focus_change(ui: &egui::Ui, response: &egui::Response) {
    let key = egui::Id::new("settings_last_focus");
    let changed = ui.ctx().data_mut(|d| {
        let changed = d.get_temp::<egui::Id>(key) != Some(response.id);
        d.insert_temp(key, response.id);
        changed
    });
    if changed {
        response.scroll_to_me(None);
    }
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
                ui.label(egui::RichText::new(label).size(FONT_MICRO).strong().color(color));
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
    fn every_row_lives_in_exactly_one_section() {
        for row in [Row::Language, Row::Notifications, Row::Theme, Row::ClearIcons, Row::ClearCatalog, Row::PurgeAll] {
            let homes = SECTIONS.iter().filter(|(_, rows)| rows.contains(&row)).count();
            assert_eq!(homes, 1, "{row:?}");
        }
    }
}
