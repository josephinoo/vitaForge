use crate::app::i18n::Language;
use crate::app::ui::chrome_icons::{self, ChromeIcon};
use crate::app::ui::theme::*;
use crate::data::{Category, SortOrder};
use crate::input::AppCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarSlot {
    All,
    Category(Category),
    Featured,
    Downloads,
    Recent,
    Favorites,
}

pub fn sidebar_slots(category_counts: &[(Category, usize)]) -> Vec<SidebarSlot> {
    let mut slots = Vec::with_capacity(1 + category_counts.len() + 4);
    slots.push(SidebarSlot::All);
    for &(category, _) in category_counts {
        slots.push(SidebarSlot::Category(category));
    }
    slots.push(SidebarSlot::Featured);
    slots.push(SidebarSlot::Downloads);
    slots.push(SidebarSlot::Recent);
    slots.push(SidebarSlot::Favorites);
    slots
}

pub fn active_sidebar_index(
    slots: &[SidebarSlot],
    selected: Option<Category>,
    favorites_filter: bool,
    sort_order: SortOrder,
) -> usize {
    let target = if favorites_filter {
        SidebarSlot::Favorites
    } else if let Some(cat) = selected {
        SidebarSlot::Category(cat)
    } else {
        match sort_order {
            SortOrder::Rating => SidebarSlot::Featured,
            SortOrder::Downloads => SidebarSlot::Downloads,
            _ => SidebarSlot::All,
        }
    };
    slots.iter().position(|&s| s == target).unwrap_or(0)
}

pub fn apply_sidebar_slot(slot: SidebarSlot, commands: &mut Vec<AppCommand>) {
    match slot {
        SidebarSlot::All => {
            commands.push(AppCommand::SetFavoritesFilter(false));
            commands.push(AppCommand::SetCategoryFilter(None));
        }
        SidebarSlot::Category(category) => {
            commands.push(AppCommand::SetFavoritesFilter(false));
            commands.push(AppCommand::SetCategoryFilter(Some(category)));
        }
        SidebarSlot::Featured => {
            commands.push(AppCommand::SetFavoritesFilter(false));
            commands.push(AppCommand::SetSortOrder(SortOrder::Rating));
        }
        SidebarSlot::Downloads => {
            commands.push(AppCommand::SetFavoritesFilter(false));
            commands.push(AppCommand::SetSortOrder(SortOrder::Downloads));
        }
        SidebarSlot::Recent => {
            commands.push(AppCommand::SetFavoritesFilter(false));
            commands.push(AppCommand::SetSortOrder(SortOrder::Recent));
        }
        SidebarSlot::Favorites => {
            commands.push(AppCommand::SetFavoritesFilter(true));
        }
    }
}

pub(crate) fn catalog_sidebar(
    ctx: &egui::Context,
    left: f32,
    top: f32,
    lang: Language,
    total: usize,
    category_counts: &[(Category, usize)],
    selected: Option<Category>,
    favorites_filter: bool,
    sort_order: SortOrder,
    focus_index: Option<usize>,
    commands: &mut Vec<AppCommand>,
) {
    let slots = sidebar_slots(category_counts);
    let platform_end = 1 + category_counts.len();

    egui::Area::new(egui::Id::new("catalog_sidebar"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(left, top))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(BG_HEADER)
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.set_width(SIDEBAR_WIDTH - 8.0);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    for (index, &slot) in slots.iter().enumerate() {
                        if index == platform_end {
                            ui.add_space(8.0);
                            let sep = ui
                                .allocate_exact_size(
                                    egui::vec2(ui.available_width(), 1.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.painter().hline(
                                sep.x_range(),
                                sep.center().y,
                                egui::Stroke::new(1.0_f32, SEPARATOR),
                            );
                            ui.add_space(6.0);
                        }

                        let focused = focus_index == Some(index);
                        let clicked = match slot {
                            SidebarSlot::All => sidebar_row(
                                ui,
                                ChromeIcon::All,
                                lang.category_label(None),
                                total,
                                selected.is_none() && !favorites_filter,
                                focused,
                            ),
                            SidebarSlot::Category(category) => {
                                let count = category_counts
                                    .iter()
                                    .find(|(c, _)| *c == category)
                                    .map(|(_, n)| *n)
                                    .unwrap_or(0);
                                sidebar_row(
                                    ui,
                                    ChromeIcon::for_category(category),
                                    lang.sidebar_platform_label(category),
                                    count,
                                    selected == Some(category) && !favorites_filter,
                                    focused,
                                )
                            }
                            SidebarSlot::Featured => sidebar_action_row(
                                ui,
                                ChromeIcon::Featured,
                                lang.sidebar_featured(),
                                sort_order == SortOrder::Rating && !favorites_filter,
                                focused,
                            ),
                            SidebarSlot::Downloads => sidebar_action_row(
                                ui,
                                ChromeIcon::Downloads,
                                lang.sidebar_downloads(),
                                sort_order == SortOrder::Downloads && !favorites_filter,
                                focused,
                            ),
                            SidebarSlot::Recent => sidebar_action_row(
                                ui,
                                ChromeIcon::Recent,
                                lang.sidebar_recent(),
                                sort_order == SortOrder::Recent && !favorites_filter,
                                focused,
                            ),
                            SidebarSlot::Favorites => sidebar_action_row(
                                ui,
                                ChromeIcon::Favorites,
                                lang.sidebar_favorites(),
                                favorites_filter,
                                focused,
                            ),
                        };

                        if clicked {
                            apply_sidebar_slot(slot, commands);
                        }

                        if matches!(slot, SidebarSlot::All) {
                            ui.add_space(2.0);
                        }
                    }
                });
        });
}

fn sidebar_action_row(
    ui: &mut egui::Ui,
    icon: ChromeIcon,
    label: &str,
    active: bool,
    focused: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SIDEBAR_ROW_HEIGHT),
        egui::Sense::click(),
    );
    let fill = if active {
        BLUE_PLAY.gamma_multiply(0.55)
    } else if response.hovered() || focused {
        BG_CARD_HOVER
    } else {
        egui::Color32::TRANSPARENT
    };
    let active_t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(("side_action", label)), active, 0.12);
    if active_t > 0.01 {
        ui.painter().rect_filled(
            rect,
            RADIUS_SM,
            BLUE_PLAY.gamma_multiply(0.35 + 0.45 * active_t),
        );
    } else if !matches!(fill, egui::Color32::TRANSPARENT) {
        ui.painter().rect_filled(rect, RADIUS_SM, fill);
    }
    if focused && !active {
        ui.painter().rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(FOCUS_STROKE, ACCENT_CYAN),
            egui::StrokeKind::Outside,
        );
    }
    let color = if active_t > 0.5 || focused {
        TEXT_WHITE
    } else {
        TEXT_DIM
    };
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.left() + 4.0 + chrome_icons::SIDEBAR_ICON * 0.5,
            rect.center().y,
        ),
        egui::vec2(chrome_icons::SIDEBAR_ICON, chrome_icons::SIDEBAR_ICON),
    );
    chrome_icons::paint(ui, icon, icon_rect, color);
    ui.painter().text(
        rect.left_center() + egui::vec2(4.0 + chrome_icons::SIDEBAR_ICON + 6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_SMALL),
        color,
    );
    response.clicked()
}

fn sidebar_row(
    ui: &mut egui::Ui,
    icon: ChromeIcon,
    label: &str,
    count: usize,
    active: bool,
    focused: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SIDEBAR_ROW_HEIGHT),
        egui::Sense::click(),
    );
    let fill = if active {
        BLUE_PLAY
    } else if response.hovered() || focused {
        BG_CARD_HOVER
    } else {
        egui::Color32::TRANSPARENT
    };
    let active_t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(("side", label)), active, 0.12);
    if active_t > 0.01 {
        ui.painter()
            .rect_filled(rect, RADIUS_SM, BLUE_PLAY.gamma_multiply(active_t));
    } else if !matches!(fill, egui::Color32::TRANSPARENT) {
        ui.painter().rect_filled(rect, RADIUS_SM, fill);
    }
    if focused && !active {
        ui.painter().rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(FOCUS_STROKE, ACCENT_CYAN),
            egui::StrokeKind::Outside,
        );
    }
    let color = if active_t > 0.5 || focused {
        TEXT_WHITE
    } else {
        TEXT_DIM
    };
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.left() + 4.0 + chrome_icons::SIDEBAR_ICON * 0.5,
            rect.center().y,
        ),
        egui::vec2(chrome_icons::SIDEBAR_ICON, chrome_icons::SIDEBAR_ICON),
    );
    chrome_icons::paint(ui, icon, icon_rect, color);
    ui.painter().text(
        rect.left_center() + egui::vec2(4.0 + chrome_icons::SIDEBAR_ICON + 6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        font(FONT_BODY),
        color,
    );
    ui.painter().text(
        rect.right_center() - egui::vec2(4.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        font(FONT_SMALL),
        if active || focused {
            TEXT_WHITE
        } else {
            TEXT_FAINT
        },
    );
    response.clicked()
}
