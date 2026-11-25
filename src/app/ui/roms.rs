use crate::app::i18n::Language;
use crate::app::icons::IconCache;
use crate::app::ui::sidebar::sidebar_row;
use crate::app::ui::grid::catalog_grid;
use crate::app::ui::widgets::filter_dropdown;
use crate::install::installed::InstalledIndex;
use crate::app::ui::theme::*;
use crate::input::AppCommand;
use crate::roms::{ROM_CONSOLES, RomConsole, RomEntry};

pub struct RomsView<'a> {
    pub rom_type: &'a str,
    pub genre_filter: Option<&'a str>,
    pub region_filter: Option<&'a str>,
    pub loading: bool,
    pub failed: bool,
    pub entries: &'a [RomEntry],
    pub tiles: &'a [crate::data::AppEntry],
    pub genre_counts: &'a [(String, usize)],
    pub region_counts: &'a [(String, usize)],
    pub totals: &'a std::collections::HashMap<&'static str, usize>,
    pub selected: usize,
    pub scroll_to_selected: bool,
    pub sidebar_focus: bool,
    pub sidebar_cursor: usize,
}

pub(crate) fn roms_view(
    ui: &mut egui::Ui,
    lang: Language,
    icons: &IconCache,
    installed: &InstalledIndex,
    search_query: &str,
    view: RomsView<'_>,
    commands: &mut Vec<AppCommand>,
) {
    let total_h = ui.available_height().max(100.0);
    // A vertical ScrollArea clips to its own height, not the panel's: never let it pass this.
    let bottom = ui.clip_rect().bottom().min(ui.max_rect().bottom());
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), total_h),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);

            // Left console sidebar (matching VitaForge catalog sidebar)
            // Same panel and rows as the catalog sidebar.
            egui::Frame::NONE
                .fill(pal().bg_header)
                .corner_radius(RADIUS_MD)
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.set_width(SIDEBAR_WIDTH - 8.0);
                    // The parent lays out left to right, and a ScrollArea inherits that without this.
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("roms_consoles")
                            .max_height(total_h - 8.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Spread the rows over the whole panel; scroll only when they don't fit.
                                let rows = ROM_CONSOLES.len() as f32;
                                let spare = ui.available_height() - rows * SIDEBAR_ROW_HEIGHT;
                                ui.spacing_mut().item_spacing.y = (spare / (rows - 1.0)).floor().max(0.0);
                                for (i, &RomConsole { code, .. }) in ROM_CONSOLES.iter().enumerate() {
                                    let active = view.rom_type == code;
                                    let focused = view.sidebar_focus && view.sidebar_cursor == i;
                                    let response = sidebar_row(
                                        ui,
                                        |ui, rect, tint| paint_console_icon(ui, code, rect.expand(2.0), tint),
                                        &code.to_uppercase(),
                                        view.totals.get(code).copied(),
                                        active,
                                        focused,
                                    );
                                    if focused {
                                        response.scroll_to_me(None);
                                    }
                                    if response.clicked() && !active {
                                        commands.push(AppCommand::RomsSelectConsole(code.to_owned()));
                                    }
                                }
                            });
                    });
                });

            // Right 5-column ROMs grid constrained strictly to available height above footer
            ui.vertical(|ui| {
                if view.scroll_to_selected {
                    icons.note_scrolling();
                }
                // Same pills, in the same place, as the Games tab.
                if !view.genre_counts.is_empty() || !view.region_counts.is_empty() {
                    // The row's height is reserved up front so the grid can never start under the pills.
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 30.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.set_min_height(30.0);
                            if let Some(genre) = filter_dropdown(ui, "roms_genre", &lang.all_genres_label(), view.genre_counts, view.genre_filter, |g| lang.genre_label(g)) {
                                commands.push(AppCommand::RomsSetGenre(genre));
                            }
                            if let Some(region) = filter_dropdown(ui, "roms_region", &lang.tr("filter-all-regions"), view.region_counts, view.region_filter, str::to_owned) {
                                commands.push(AppCommand::RomsSetRegion(region));
                            }
                        },
                    );
                    ui.add_space(6.0);
                }

                if view.failed {
                    ui.add_space(18.0);
                    ui.label(egui::RichText::new(lang.tr("roms-error")).size(FONT_LARGE).color(pal().text_dim));
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new(lang.tr("roms-retry")).min_size(egui::vec2(140.0, 44.0))).clicked() {
                        commands.push(AppCommand::RomsRetry);
                    }
                    return;
                }

                let needle = search_query.trim().to_lowercase();
                let filtered: Vec<usize> = view
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.matches(&needle, view.genre_filter, view.region_filter))
                    .map(|(i, _)| i)
                    .collect();

                if filtered.is_empty() {
                    ui.add_space(18.0);
                    let msg = if view.loading {
                        ui.spinner();
                        lang.tr_args("roms-loading", &[("console", &view.rom_type.to_uppercase())])
                    } else {
                        lang.tr("roms-empty")
                    };
                    ui.label(egui::RichText::new(msg).size(FONT_LARGE).color(pal().text_dim));
                    return;
                }

                // The Games grid itself; only the "open" command differs.
                let size = egui::vec2(ui.available_width(), (bottom - ui.cursor().top()).max(80.0));
                let selected = view.selected.min(filtered.len() - 1);
                let mut picked = Vec::new();
                ui.allocate_ui(size, |ui| {
                    catalog_grid(
                        ui,
                        icons,
                        installed,
                        view.tiles,
                        &filtered,
                        (!view.sidebar_focus).then_some(selected),
                        view.scroll_to_selected,
                        false,
                        &mut picked,
                        GRID_COLUMNS,
                    );
                });
                commands.extend(picked.into_iter().filter_map(|command| match command {
                    AppCommand::SelectApp { index } => filtered.get(index).map(|&i| AppCommand::RomsOpen(i)),
                    _ => None,
                }));
            });
        },
    );
}

/// libretro's monochrome system icons (CC BY 4.0), named by console code.
fn console_png(code: &str) -> Option<&'static [u8]> {
    macro_rules! png {
        ($($code:literal),*) => {
            match code {
                $($code => Some(include_bytes!(concat!("../../../assets/consoles/", $code, ".png")).as_slice()),)*
                _ => None,
            }
        };
    }
    png!("3ds", "ds", "gba", "snes", "n64", "gb", "gbc", "nes")
}

fn paint_console_icon(ui: &egui::Ui, code: &str, rect: egui::Rect, tint: egui::Color32) {
    let id = egui::Id::new(("console_icon", code));
    let texture = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        // Downscaled once: sampling 256 px into a 20 px square shimmers.
        let small = image::load_from_memory(console_png(code)?)
            .ok()?
            .resize(48, 48, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let size = [small.width() as usize, small.height() as usize];
        let pixels = egui::ColorImage::from_rgba_unmultiplied(size, small.as_raw());
        let texture = ui.ctx().load_texture(format!("console_{code}"), pixels, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, texture.clone()));
        Some(texture)
    });
    if let Some(texture) = texture {
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture.id(), rect, uv, tint);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consoles_are_listed_top_to_bottom() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let icons = IconCache::new();
        let installed = InstalledIndex::default();
        let mut ys = Vec::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 362.0))),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let view = RomsView {
                        rom_type: "3ds", genre_filter: None, region_filter: None, loading: false, failed: false,
                        entries: &[], tiles: &[], genre_counts: &[], region_counts: &[], totals: &Default::default(), selected: 0,
                        scroll_to_selected: false, sidebar_focus: false, sidebar_cursor: 0,
                    };
                    roms_view(ui, crate::app::i18n::Language::English, &icons, &installed, "", view, &mut Vec::new());
                });
            });
            ys = output.shapes.iter().filter_map(|c| match &c.shape {
                egui::Shape::Text(t) if ROM_CONSOLES.iter().any(|k| k.code.to_uppercase() == t.galley.text()) => Some(t.pos.y),
                _ => None,
            }).collect();
        }
        assert!(ROM_CONSOLES.iter().all(|c| console_png(c.code).is_some_and(|png| image::load_from_memory(png).is_ok())));
        assert!(ys.len() > 1 && ys.windows(2).all(|w| w[1] > w[0] + 20.0), "{ys:?}");
    }

    /// Many ROMs above a footer: art stays inside the panel and the d-pad cursor scrolls into view.
    #[test]
    fn grid_stays_above_the_footer_and_follows_the_cursor() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let icons = IconCache::new();
        let installed = InstalledIndex::default();
        let json = format!(
            "{{\"data\":[{}]}}",
            (0..60).map(|i| format!("{{\"id\":\"{i}\",\"title\":\"Game{i}\",\"filename\":\"{i}\",\"download_url\":\"u\"}}")).collect::<Vec<_>>().join(",")
        );
        let entries = crate::roms::parse_snapshot(json.as_bytes()).unwrap();
        let tiles: Vec<_> = entries.iter().map(|e| e.to_app_entry_in("3ds", std::path::Path::new("x"))).collect();
        let genres = [("Action".to_owned(), 60)];
        let mut seen = false;
        let mut footer_top = 0.0;
        for frame in 0..60 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 362.0))),
                time: Some(f64::from(frame) * 0.05),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                footer_top = egui::TopBottomPanel::bottom("f").exact_height(40.0).show(ctx, |_| {}).response.rect.top();
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(50.0);
                    let view = RomsView {
                        rom_type: "3ds", genre_filter: None, region_filter: None, loading: false, failed: false,
                        entries: &entries, tiles: &tiles, genre_counts: &genres, region_counts: &genres, totals: &Default::default(),
                        selected: 42, scroll_to_selected: matches!(frame, 2 | 4 | 6), sidebar_focus: false, sidebar_cursor: 0,
                    };
                    ui.horizontal_top(|ui| roms_view(ui, Language::English, &icons, &installed, "", view, &mut Vec::new()));
                });
            });
            seen = false;
            let pills_bottom = output.shapes.iter().filter_map(|c| match &c.shape {
                egui::Shape::Rect(r) if r.rect.height() == 26.0 && r.rect.width() >= 112.0 => Some(r.rect.bottom()),
                _ => None,
            }).fold(0.0_f32, f32::max);
            assert!(pills_bottom > 0.0, "filters not drawn");
            for clipped in &output.shapes {
                if matches!(&clipped.shape, egui::Shape::Text(t) if t.galley.text().starts_with("Game")) {
                    assert!(clipped.clip_rect.top() >= pills_bottom, "grid {:?} under filters ending at {pills_bottom}", clipped.clip_rect);
                }
            }
            for clipped in &output.shapes {
                if let egui::Shape::Text(t) = &clipped.shape
                    && t.galley.text() == "Game42"
                {
                    seen = t.pos.y >= 0.0 && t.pos.y < clipped.clip_rect.bottom();
                }
                if matches!(&clipped.shape, egui::Shape::Rect(_)) && clipped.clip_rect.top() > 40.0 && clipped.clip_rect.top() < footer_top {
                    assert!(clipped.clip_rect.bottom() <= footer_top + 4.0, "{:?} over footer {footer_top}", clipped.clip_rect);
                }
            }
        }
        assert!(seen, "the selected ROM is not on screen");
    }
}
