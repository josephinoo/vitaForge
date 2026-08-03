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

    sorted_indices: Vec<usize>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub search_requested: bool,
    pub category_filter: Option<Category>,
    pub sort_order: SortOrder,
    pub selected: usize,

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

        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = self.selected.min(self.filtered_indices.len() - 1);
            self.selection_active = true;
            self.scroll_to_selected = true;
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.scroll_to_selected = true;

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

const SELF_REPO_URL: &str = "https://github.com/josephinoo/vitaForge";

#[derive(Debug, Clone)]
pub struct SelfUpdateInfo {
    pub tag: String,
    pub vpk_url: String,
}

pub struct App {
    pub state: AppState,
    pub icons: IconCache,
    pub installed: crate::install::installed::InstalledIndex,
    pub lang: Language,
    pub install: Option<InstallJob>,
    pub self_update: Option<SelfUpdateInfo>,

    needs_installed_rescan: bool,
    load_rx: Option<oneshot::Receiver<CatalogSource>>,
    hashes_rx: Option<oneshot::Receiver<Vec<data::source::MinimalEntry>>>,
    self_update_rx: Option<oneshot::Receiver<SelfUpdateInfo>>,
}

pub struct InstallJob {
    pub app_id: String,

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

        let (self_update_tx, self_update_rx) = oneshot::channel();
        tokio::spawn(async move {
            if let Some(release) = crate::install::github::latest_release(SELF_REPO_URL).await {
                let current_ver = env!("CARGO_PKG_VERSION").trim_start_matches('v');
                let release_ver = release.tag.trim_start_matches('v');
                if release_ver != current_ver && !release_ver.is_empty() {
                    let _ = self_update_tx.send(SelfUpdateInfo {
                        tag: release.tag,
                        vpk_url: release.vpk_url,
                    });
                }
            }
        });

        let cached = data::source::initial_catalog();
        let (state, hashes_rx) = if cached.is_empty() {
            (AppState::Loading, None)
        } else {
            let (hash_tx, hash_rx) = oneshot::channel();
            tokio::spawn(async move {
                match data::source::fetch_minimal().await {
                    Ok(minimal) => {
                        let _ = hash_tx.send(minimal);
                    }
                    Err(err) => eprintln!("hash refresh failed: {err:#}"),
                }
            });
            (AppState::Catalog(CatalogState::new(cached)), Some(hash_rx))
        };

        Ok(Self {
            state,
            icons: IconCache::new(),
            installed: crate::install::installed::InstalledIndex::new(),
            lang: Language::detect(),
            install: None,
            self_update: None,
            needs_installed_rescan: true,
            load_rx: Some(rx),
            hashes_rx,
            self_update_rx: Some(self_update_rx),
        })
    }

    pub fn install_busy(&self) -> bool {
        self.install.as_ref().is_some_and(|job| !job.progress.is_finished())
    }

    pub fn tick(&mut self, ctx: &egui::Context) -> Result<()> {
        if self.needs_installed_rescan {

            let entries: &[AppEntry] = match &self.state {
                AppState::Catalog(catalog) => &catalog.apps,
                AppState::Detail { previous, .. } => &previous.apps,
                AppState::Loading => &[],
            };
            if self.installed.refresh(ctx, entries) {
                self.needs_installed_rescan = false;
            }
        }
        if let Some(job) = &mut self.install {
            let previous = std::mem::replace(&mut job.progress, job.rx.borrow_and_update().clone());

            if previous != job.progress && job.progress == crate::install::Progress::Done {
                self.installed.mark_installed(&job.app_id_title);
            }
        }

        if let Some(rx) = &mut self.hashes_rx
            && let Ok(minimal) = rx.try_recv()
        {
            self.hashes_rx = None;
            if let AppState::Catalog(catalog) = &mut self.state {
                data::source::apply_hashes(&mut catalog.apps, &minimal);
                self.installed.force_refresh(ctx, &catalog.apps);
                ctx.request_repaint();
            }
        }

        if let Some(rx) = &mut self.self_update_rx
            && let Ok(info) = rx.try_recv()
        {
            self.self_update_rx = None;
            self.self_update = Some(info);

            let _ = self.handle_command(AppCommand::SelfUpdate);
            ctx.request_repaint();
        }

        let Some(rx) = &mut self.load_rx else { return Ok(()) };
        match rx.try_recv() {

            Ok(CatalogSource::Live(apps)) => {
                self.load_rx = None;
                if matches!(self.state, AppState::Loading) && !self.install_busy() {
                    let catalog = CatalogState::new(apps);
                    self.installed.force_refresh(ctx, &catalog.apps);
                    self.state = AppState::Catalog(catalog);
                    ctx.request_repaint();
                }
            }
            Ok(CatalogSource::Failed) => {
                self.load_rx = None;
                if matches!(self.state, AppState::Loading) && !self.install_busy() {
                    let catalog = CatalogState::new(data::source::initial_catalog());
                    self.installed.force_refresh(ctx, &catalog.apps);
                    self.state = AppState::Catalog(catalog);
                    ctx.request_repaint();
                }
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
                    let app_id_title = crate::install::installed::index_key(&entry);
                    let rx = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob { app_id, app_id_title, progress, rx });
                }
            }
            AppCommand::DismissInstall => self.install = None,
            AppCommand::SelfUpdate => {
                let busy = self.install.as_ref().is_some_and(|job| !job.progress.is_finished());
                if let Some(info) = self.self_update.clone()
                    && !busy
                {
                    let entry = AppEntry {
                        id: "vitaforge_self_update".to_owned(),
                        titleid: "VITAFORGE".to_owned(),
                        name: format!("VitaForge {}", info.tag),
                        name_lower: "vitaforge".to_owned(),
                        author: "josephinoo".to_owned(),
                        description: format!("Self-update for VitaForge {}", info.tag),
                        long_description: String::new(),
                        requirements: String::new(),
                        changelog: String::new(),
                        release_page: Some("https://github.com/josephinoo/vitaForge/releases".to_owned()),
                        category: Category::Utility,
                        platform: crate::data::Platform::Vita,
                        icon_url: None,
                        screenshot_urls: Vec::new(),
                        download_url: info.vpk_url.clone(),
                        source: Some(SELF_REPO_URL.to_owned()),
                        version: info.tag.clone(),
                        hash: String::new(),
                        hash2: String::new(),
                        data_url: None,
                        data_size_bytes: 0,
                        size_bytes: 0,
                        downloads: 0,
                        rating: 5.0,
                        updated_at: String::new(),
                    };
                    let app_id = entry.id.clone();
                    let app_id_title = entry.titleid.clone();
                    let rx = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob { app_id, app_id_title, progress, rx });
                }
            }
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

        catalog.selection_active = true;
        self.state = AppState::Detail { app: entry, previous: Box::new(catalog), origin, scroll_offset: 0.0 };
    }

    pub fn clear_scroll_to_selected(&mut self) {
        if let AppState::Catalog(catalog) = &mut self.state {
            catalog.scroll_to_selected = false;
        }
    }

    fn back_to_catalog(&mut self) {

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
