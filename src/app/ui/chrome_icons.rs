use crate::data::Category;
use crate::input::ContentTypeGroup;

/// Small, original line glyphs designed to remain distinct on Vita's display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeIcon {
    Home,
    Games,
    Apps,
    Emulators,
    Plugins,
    All,
    Vita,
    Psp,
    Ps1,
    Ports,
    Homebrew,
    Featured,
    Downloads,
    Recent,
    Favorites,
    Search,
    Library,
    Updates,
}

impl ChromeIcon {
    pub fn for_group(group: ContentTypeGroup) -> Self {
        match group {
            ContentTypeGroup::Home => Self::Home,
            ContentTypeGroup::Games => Self::Games,
            ContentTypeGroup::Apps => Self::Apps,
            ContentTypeGroup::Emulators => Self::Emulators,
            ContentTypeGroup::Plugins => Self::Plugins,
        }
    }
    pub fn for_store_tab(tab: crate::input::StoreTab) -> Self {
        match tab {
            crate::input::StoreTab::Categories => Self::All,
            crate::input::StoreTab::Library => Self::Library,
            crate::input::StoreTab::Updates => Self::Updates,
        }
    }
    pub fn for_category(category: Category) -> Self {
        match category {
            Category::PsVitaGame => Self::Vita,
            Category::PspGame => Self::Psp,
            Category::Ps1Game => Self::Ps1,
            Category::Port => Self::Ports,
            Category::Original => Self::Homebrew,
            Category::Utility | Category::Tool => Self::Apps,
            Category::Emulator => Self::Emulators,
            Category::Plugin => Self::Plugins,
            Category::Other => Self::All,
        }
    }
}

pub fn paint(ui: &egui::Ui, icon: ChromeIcon, rect: egui::Rect, tint: egui::Color32) {
    let painter = ui.painter();
    let center = rect.center();
    let side = rect.width().min(rect.height());
    let stroke = egui::Stroke::new((side * 0.105).clamp(1.25, 2.0), tint);
    let square = |scale| egui::Rect::from_center_size(center, egui::Vec2::splat(side * scale));
    match icon {
        ChromeIcon::Home => {
            let r = square(0.58);
            painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(r.left(), center.y - side * 0.06),
                    egui::pos2(center.x, r.top()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, r.top()),
                    egui::pos2(r.right(), center.y - side * 0.06),
                ],
                stroke,
            );
        }
        ChromeIcon::Games | ChromeIcon::Vita | ChromeIcon::Psp => {
            let r = egui::Rect::from_center_size(center, egui::vec2(side * 0.78, side * 0.48));
            painter.rect_stroke(r, side * 0.18, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(r.left() + side * 0.18, center.y),
                    egui::pos2(r.left() + side * 0.34, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(r.left() + side * 0.26, center.y - side * 0.08),
                    egui::pos2(r.left() + side * 0.26, center.y + side * 0.08),
                ],
                stroke,
            );
            painter.circle_filled(
                egui::pos2(r.right() - side * 0.20, center.y - side * 0.07),
                side * 0.045,
                tint,
            );
            painter.circle_filled(
                egui::pos2(r.right() - side * 0.12, center.y + side * 0.07),
                side * 0.045,
                tint,
            );
        }
        ChromeIcon::Apps | ChromeIcon::All => {
            for y in [-0.20, 0.20] {
                for x in [-0.20, 0.20] {
                    painter.rect_filled(
                        egui::Rect::from_center_size(
                            center + egui::vec2(x * side, y * side),
                            egui::Vec2::splat(side * 0.25),
                        ),
                        2.0,
                        tint,
                    );
                }
            }
        }
        ChromeIcon::Emulators => {
            let r = square(0.64);
            painter.rect_stroke(r, 3.0, stroke, egui::StrokeKind::Inside);
            for x in [-0.18, 0.0, 0.18] {
                for y in [-0.18, 0.0, 0.18] {
                    painter.circle_filled(
                        center + egui::vec2(x * side, y * side),
                        side * 0.035,
                        tint,
                    );
                }
            }
        }
        ChromeIcon::Plugins => {
            // Android-style extension: puzzle piece (filled — strokes vanish on Vita).
            let body = egui::Rect::from_center_size(
                center + egui::vec2(side * 0.04, side * 0.04),
                egui::vec2(side * 0.44, side * 0.44),
            );
            painter.rect_filled(body, side * 0.08, tint);
            let knob = side * 0.125;
            painter.circle_filled(egui::pos2(body.center().x, body.top()), knob, tint);
            painter.circle_filled(egui::pos2(body.right(), body.center().y), knob, tint);
        }
        ChromeIcon::Ps1 => {
            painter.circle_stroke(center, side * 0.31, stroke);
            painter.circle_stroke(center, side * 0.08, stroke);
            painter.line_segment(
                [
                    center,
                    egui::pos2(center.x + side * 0.25, center.y - side * 0.15),
                ],
                stroke,
            );
        }
        ChromeIcon::Ports => {
            let r = square(0.60);
            painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(r.left(), center.y),
                    egui::pos2(r.right(), center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, r.top()),
                    egui::pos2(center.x, r.bottom()),
                ],
                stroke,
            );
        }
        ChromeIcon::Homebrew => {
            let r = egui::Rect::from_center_size(
                center + egui::vec2(0.0, side * 0.08),
                egui::vec2(side * 0.42, side * 0.44),
            );
            painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(center.x - side * 0.12, r.top()),
                    egui::pos2(center.x + side * 0.12, r.top()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y - side * 0.34),
                    egui::pos2(center.x, center.y - side * 0.13),
                ],
                stroke,
            );
        }
        ChromeIcon::Featured => {
            let points = (0..5)
                .map(|i| {
                    let angle =
                        -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                    center + egui::vec2(angle.cos(), angle.sin()) * side * 0.32
                })
                .collect::<Vec<_>>();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
        ChromeIcon::Downloads | ChromeIcon::Updates => {
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y - side * 0.34),
                    egui::pos2(center.x, center.y + side * 0.10),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - side * 0.13, center.y - side * 0.01),
                    egui::pos2(center.x, center.y + side * 0.12),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x + side * 0.13, center.y - side * 0.01),
                    egui::pos2(center.x, center.y + side * 0.12),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - side * 0.30, center.y + side * 0.31),
                    egui::pos2(center.x + side * 0.30, center.y + side * 0.31),
                ],
                stroke,
            );
        }
        ChromeIcon::Recent => {
            painter.circle_stroke(center, side * 0.31, stroke);
            painter.line_segment(
                [center, egui::pos2(center.x, center.y - side * 0.20)],
                stroke,
            );
            painter.line_segment(
                [
                    center,
                    egui::pos2(center.x + side * 0.17, center.y + side * 0.10),
                ],
                stroke,
            );
        }
        ChromeIcon::Favorites => {
            let r = side * 0.28;
            let a = center + egui::vec2(-r * 0.52, -r * 0.30);
            let b = center + egui::vec2(r * 0.52, -r * 0.30);
            painter.circle_stroke(a, r * 0.58, stroke);
            painter.circle_stroke(b, r * 0.58, stroke);
            painter.line_segment(
                [
                    egui::pos2(center.x - r, center.y - side * 0.06),
                    egui::pos2(center.x, center.y + side * 0.33),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x + r, center.y - side * 0.06),
                    egui::pos2(center.x, center.y + side * 0.33),
                ],
                stroke,
            );
        }
        ChromeIcon::Search => {
            painter.circle_stroke(
                center - egui::vec2(side * 0.10, side * 0.10),
                side * 0.23,
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(side * 0.07, side * 0.07),
                    center + egui::vec2(side * 0.32, side * 0.32),
                ],
                stroke,
            );
        }
        ChromeIcon::Library => {
            let r = egui::Rect::from_center_size(center, egui::vec2(side * 0.62, side * 0.54));
            painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(center.x, r.top()),
                    egui::pos2(center.x, r.bottom()),
                ],
                stroke,
            );
        }
    }
}

pub const SIDEBAR_ICON: f32 = 17.0;
pub const NAV_ICON: f32 = 17.0;
pub const SEARCH_ICON: f32 = 16.0;
