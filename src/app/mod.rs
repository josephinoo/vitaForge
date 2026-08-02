pub mod i18n;
pub mod icons;
pub mod ui;

use crate::data::{self, AppEntry, Category, SortOrder};
use crate::input::{AppCommand, InputCommand};
use anyhow::Result;
use i18n::Language;
use icons::IconCache;
use tokio::sync::{oneshot, watch};

pub struct CatalogState {
    pub apps: Vec<AppEntry>,
    /// Rebuilt only when the sort changes, not on every keystroke.
    sorted_indices: Vec<usize>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub search_requested: bool,
    pub category_filter: Option<Category>,
    pub sort_order: SortOrder,
    pub selected: usize,
    /// A fresh result list starts with nothing highlighted, so the first hit of a
    /// search never looks pre-picked.
    pub selection_active: bool,
    pub scroll_to_selected: bool,
}

impl CatalogState {
    fn new(apps: Vec<AppEntry>) -> Self {
        let mut state = Self {
            apps,
            sorted_indices: Vec::new(),
            filtered_indices: Vec::new(),
            search_query: String::new(),
            search_requested: false,
            category_filter: None,
            sort_order: SortOrder::Downloads,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
        };
        state.resort();
        state
    }

    fn empty() -> Self {
        Self {
            apps: Vec::new(),
            sorted_indices: Vec::new(),
            filtered_indices: Vec::new(),
            search_query: String::new(),
            search_requested: false,
            category_filter: None,
            sort_order: SortOrder::Downloads,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
        }
    }

    /// Resorts and refilters. Only needed when the catalog or sort order changes.
    fn resort(&mut self) {
        let mut order: Vec<usize> = (0..self.apps.len()).collect();
        let apps = &self.apps;
        match self.sort_order {
            SortOrder::Downloads => order.sort_by(|&a, &b| apps[b].downloads.cmp(&apps[a].downloads)),
            SortOrder::Rating => order.sort_by(|&a, &b| apps[b].rating.total_cmp(&apps[a].rating)),
            SortOrder::Recent => order.sort_by(|&a, &b| apps[b].updated_at.cmp(&apps[a].updated_at)),
            SortOrder::Size => order.sort_by(|&a, &b| apps[b].size_bytes.cmp(&apps[a].size_bytes)),
            SortOrder::NameAsc => order.sort_by(|&a, &b| apps[a].name_lower.cmp(&apps[b].name_lower)),
        }
        self.sorted_indices = order;
        self.refresh_filter();
    }

    /// One pass over the sorted indices; runs on every keystroke.
    fn refresh_filter(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        let apps = &self.apps;
        let category_filter = self.category_filter;

        self.filtered_indices = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|&index| {
                let app = &apps[index];
                category_filter.is_none_or(|c| c as u8 == app.category as u8)
                    && (query.is_empty() || app.name_lower.contains(&query))
            })
            .collect();

        self.selected = self.selected.min(self.filtered_indices.len().saturating_sub(1));
        // The list moved under the cursor, so drop the highlight.
        self.selection_active = false;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.scroll_to_selected = true;
        // The first press only lights up the entry, it does not step past it.
        if !self.selection_active {
            self.selection_active = true;
            self.selected = 0;
            return;
        }
        let last = self.filtered_indices.len() as isize - 1;
        let target = (self.selected as isize + delta).clamp(0, last);
        self.selected = target as usize;
    }
}

pub enum AppState {
    Loading,
    Catalog(CatalogState),
    Detail {
        app: AppEntry,
        previous: Box<CatalogState>,
        origin: Option<egui::Rect>,
        scroll_offset: f32,
    },
}

enum CatalogSource {
    Live(Vec<AppEntry>),
    Failed,
}

pub struct App {
    pub state: AppState,
    pub icons: IconCache,
    pub installed: crate::install::installed::InstalledIndex,
    pub lang: Language,
    pub install: Option<InstallJob>,
    /// Apps are uninstalled from LiveArea while we are not running, so recheck on
    /// startup and whenever the catalog comes back into view.
    needs_installed_rescan: bool,
    load_rx: Option<oneshot::Receiver<CatalogSource>>,
}

pub struct InstallJob {
    pub app_id: String,
    /// Kept here so a finished job can update the index without a lookup.
    pub app_id_title: String,
    pub progress: crate::install::Progress,
    rx: watch::Receiver<crate::install::Progress>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match data::source::fetch_live().await {
                Ok(apps) if !apps.is_empty() => {
                    // Handed in and back out so the UI thread never pays for a copy.
                    let apps = tokio::task::spawn_blocking(move || {
                        data::source::save_cache(&apps);
                        apps
                    })
                    .await
                    .unwrap_or_else(|err| {
                        eprintln!("catalog cache worker crashed: {err}");
                        Vec::new()
                    });
                    if apps.is_empty() { CatalogSource::Failed } else { CatalogSource::Live(apps) }
                }
                Ok(_) => CatalogSource::Failed,
                Err(err) => {
                    eprintln!("live catalog fetch failed: {err:#}");
                    CatalogSource::Failed
                }
            };
            let _ = tx.send(result);
        });
        Ok(Self {
            state: AppState::Loading,
            icons: IconCache::new(),
            installed: crate::install::installed::InstalledIndex::new(),
            lang: Language::detect(),
            install: None,
            needs_installed_rescan: true,
            load_rx: Some(rx),
        })
    }

    /// True while a package is still downloading, extracting or promoting.
    pub fn install_busy(&self) -> bool {
        self.install.as_ref().is_some_and(|job| !job.progress.is_finished())
    }

    pub fn tick(&mut self, ctx: &egui::Context) -> Result<()> {
        if self.needs_installed_rescan {
            self.needs_installed_rescan = false;
            // Needs the catalog's hashes, so it waits for the catalog.
            let entries: &[AppEntry] = match &self.state {
                AppState::Catalog(catalog) => &catalog.apps,
                AppState::Detail { previous, .. } => &previous.apps,
                AppState::Loading => &[],
            };
            self.installed.refresh(ctx, entries);
        }
        if let Some(job) = &mut self.install {
            let previous = std::mem::replace(&mut job.progress, job.rx.borrow_and_update().clone());
            // The package carries its own stamp, so this needs no rehash.
            if previous != job.progress && job.progress == crate::install::Progress::Done {
                self.installed.mark_installed(&job.app_id_title);
            }
        }

        let Some(rx) = &mut self.load_rx else { return Ok(()) };
        match rx.try_recv() {
            // The first request fired before the hashes existed, so ask again.
            Ok(CatalogSource::Live(apps)) => {
                self.state = AppState::Catalog(CatalogState::new(apps));
                self.load_rx = None;
                self.needs_installed_rescan = true;
                ctx.request_repaint();
            }
            Ok(CatalogSource::Failed) => {
                self.state = AppState::Catalog(CatalogState::new(data::source::initial_catalog()));
                self.load_rx = None;
                self.needs_installed_rescan = true;
                ctx.request_repaint();
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(oneshot::error::TryRecvError::Closed) => self.load_rx = None,
        }
        Ok(())
    }

    pub fn handle_command(&mut self, command: AppCommand) -> Result<()> {
        match command {
            AppCommand::Input(InputCommand::Back) => self.back_to_catalog(),
            AppCommand::SetSearchQuery(query) => {
                // Refiltering drops the highlight, so an unchanged query must be a
                // no-op or dismissing the keyboard would reset the cursor.
                if let AppState::Catalog(catalog) = &mut self.state
                    && catalog.search_query != query
                {
                    catalog.search_query = query;
                    catalog.refresh_filter();
                }
            }
            AppCommand::RequestSearch => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.search_requested = true;
                }
            }
            AppCommand::CloseSearch => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.search_requested = false;
                }
            }
            AppCommand::SetCategoryFilter(category) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.category_filter = category;
                    catalog.refresh_filter();
                }
            }
            AppCommand::SetSortOrder(sort) => {
                if let AppState::Catalog(catalog) = &mut self.state
                    && catalog.sort_order != sort
                {
                    catalog.sort_order = sort;
                    catalog.resort();
                }
            }
            AppCommand::Input(InputCommand::Confirm) => {
                // With nothing highlighted, confirm picks up the cursor instead.
                let target = match &mut self.state {
                    AppState::Catalog(catalog) if catalog.selection_active => Some(catalog.selected),
                    AppState::Catalog(catalog) => {
                        catalog.move_selection(0);
                        None
                    }
                    _ => None,
                };
                if let Some(index) = target {
                    self.open_app(index, None);
                }
            }
            AppCommand::Input(InputCommand::CategoryPrev) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    let next_cat = match catalog.category_filter {
                        None => Some(Category::ALL[Category::ALL.len() - 1]),
                        Some(c) => {
                            let idx = Category::ALL.iter().position(|&x| x == c).unwrap_or(0);
                            if idx == 0 { None } else { Some(Category::ALL[idx - 1]) }
                        }
                    };
                    catalog.category_filter = next_cat;
                    catalog.refresh_filter();
                }
            }
            AppCommand::Input(InputCommand::CategoryNext) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    let next_cat = match catalog.category_filter {
                        None => Some(Category::ALL[0]),
                        Some(c) => {
                            let idx = Category::ALL.iter().position(|&x| x == c).unwrap_or(0);
                            if idx + 1 >= Category::ALL.len() { None } else { Some(Category::ALL[idx + 1]) }
                        }
                    };
                    catalog.category_filter = next_cat;
                    catalog.refresh_filter();
                }
            }
            AppCommand::Input(direction) => {
                match &mut self.state {
                    AppState::Catalog(catalog) => {
                        let columns = crate::app::ui::GRID_COLUMNS as isize;
                        let delta = match direction {
                            InputCommand::MoveLeft => -1,
                            InputCommand::MoveRight => 1,
                            InputCommand::MoveUp => -columns,
                            InputCommand::MoveDown => columns,
                            _ => 0,
                        };
                        if delta != 0 {
                            catalog.move_selection(delta);
                        }
                    }
                    AppState::Detail { scroll_offset, .. } => {
                        match direction {
                            InputCommand::MoveUp => {
                                *scroll_offset = (*scroll_offset - 60.0).max(0.0);
                            }
                            InputCommand::MoveDown => {
                                *scroll_offset += 60.0;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            AppCommand::SelectApp { index, origin } => self.open_app(index, origin),
            AppCommand::BackToCatalog => self.back_to_catalog(),
            AppCommand::InstallCurrent => {
                let busy = self.install.as_ref().is_some_and(|job| !job.progress.is_finished());
                if let AppState::Detail { app, .. } = &self.state
                    && !busy
                {
                    let entry = app.clone();
                    let app_id = entry.id.clone();
                    let app_id_title = entry.titleid.clone();
                    let rx = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob { app_id, app_id_title, progress, rx });
                }
            }
            AppCommand::DismissInstall => self.install = None,
        }
        Ok(())
    }

    fn open_app(&mut self, filtered_index: usize, origin: Option<egui::Rect>) {
        let entry = match &self.state {
            AppState::Catalog(catalog) => catalog
                .filtered_indices
                .get(filtered_index)
                .and_then(|&real_index| catalog.apps.get(real_index))
                .cloned(),
            _ => None,
        };
        let Some(entry) = entry else { return };

        let placeholder =
            AppState::Detail { app: entry.clone(), previous: Box::new(CatalogState::empty()), origin, scroll_offset: 0.0 };
        let AppState::Catalog(mut catalog) = std::mem::replace(&mut self.state, placeholder) else {
            unreachable!("checked above that self.state is Catalog")
        };
        catalog.selected = filtered_index;
        // Coming back from the detail page should land on the entry that was opened.
        catalog.selection_active = true;
        self.state = AppState::Detail { app: entry, previous: Box::new(catalog), origin, scroll_offset: 0.0 };
    }

    pub fn clear_scroll_to_selected(&mut self) {
        if let AppState::Catalog(catalog) = &mut self.state {
            catalog.scroll_to_selected = false;
        }
    }

    fn back_to_catalog(&mut self) {
        // The detail page is the only place progress is shown, so stay put.
        if self.install_busy() {
            return;
        }
        if let AppState::Detail { previous, .. } = &mut self.state {
            let previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
            self.state = AppState::Catalog(previous);
            self.needs_installed_rescan = true;
        }
    }
}
