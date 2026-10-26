pub mod detail_actions;
pub mod i18n;
pub mod icons;
pub mod sysinfo;
pub mod text;
pub mod ui;
use crate::data::{self, AppEntry, Category, SortDirection, SortOrder};
use crate::input::{AppCommand, ContentTypeGroup, DataStep, InputCommand, StoreTab};
use anyhow::Result;
use i18n::Language;
use icons::IconCache;
use tokio::sync::{oneshot, watch};
pub(super) fn tile_art_url(app: &AppEntry) -> Option<&str> {
    if app.platform.is_commercial() {
        app.cover_url.as_deref().or(app.icon_url.as_deref())
    } else {
        app.icon_url.as_deref().or(app.cover_url.as_deref())
    }
}

const MAX_PRECACHE_PER_LAUNCH: usize = 150;
fn precache_art_urls(apps: &[AppEntry]) -> Vec<String> {
    apps.iter()
        .filter_map(tile_art_url)
        .map(str::to_owned)
        .take(MAX_PRECACHE_PER_LAUNCH)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FocusPane {
    #[default]
    Grid,
    Sidebar,
}

pub struct CatalogState {
    pub apps: Vec<AppEntry>,
    sorted_indices: Vec<usize>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub search_requested: bool,
    pub category_filter: Option<Category>,
    pub content_type_group: ContentTypeGroup,
    pub favorites_filter: bool,
    pub genre_filter: Option<String>,
    pub sort_order: SortOrder,
    pub sort_direction: SortDirection,
    pub selected: usize,
    pub selection_active: bool,
    pub scroll_to_selected: bool,

    pub scroll_reset: bool,
    pub is_commercial_view: bool,
    pub category_counts: Vec<(Category, usize)>,
    pub genre_counts: Vec<(String, usize)>,
    pub source_scoped_count: usize,
    pub total_unique_count: usize,
    pub group_category_counts: Vec<(Category, usize)>,
    pub group_total: usize,
    pub tab: StoreTab,
    pub scroll_category_into_view: bool,
    pub focus_pane: FocusPane,
    pub sidebar_cursor: usize,
}
impl CatalogState {
    fn new(apps: Vec<AppEntry>) -> Self {
        let settings = data::settings::load();
        let mut state = Self {
            apps,
            sorted_indices: Vec::new(),
            filtered_indices: Vec::new(),
            search_query: String::new(),
            search_requested: false,
            category_filter: None,
            content_type_group: ContentTypeGroup::Home,
            favorites_filter: false,
            genre_filter: None,
            sort_order: settings.sort_order,
            sort_direction: settings.sort_direction,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            scroll_reset: false,
            is_commercial_view: false,
            category_counts: Vec::new(),
            genre_counts: Vec::new(),
            source_scoped_count: 0,
            total_unique_count: 0,
            group_category_counts: Vec::new(),
            group_total: 0,
            tab: StoreTab::Categories,
            scroll_category_into_view: false,
            focus_pane: FocusPane::Grid,
            sidebar_cursor: 0,
        };
        if state.content_type_group == ContentTypeGroup::Home {
            state.sort_order = SortOrder::Recent;
            state.sort_direction = SortDirection::Desc;
        }
        state.resort();
        state.sync_sidebar_cursor();
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
            content_type_group: ContentTypeGroup::Home,
            favorites_filter: false,
            genre_filter: None,
            sort_order: SortOrder::default(),
            sort_direction: SortDirection::default(),
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            scroll_reset: false,
            is_commercial_view: false,
            category_counts: Vec::new(),
            genre_counts: Vec::new(),
            source_scoped_count: 0,
            total_unique_count: 0,
            group_category_counts: Vec::new(),
            group_total: 0,
            tab: StoreTab::Categories,
            scroll_category_into_view: false,
            focus_pane: FocusPane::Grid,
            sidebar_cursor: 0,
        }
    }
    fn replace_apps(&mut self, apps: Vec<AppEntry>) {
        self.apps = apps;
        self.sorted_indices = self.sort_order_indices();
        self.recompute_dropdown_counts();
        self.refresh_filter_preserving_selection();
    }
    fn apply_store_tab(&mut self, tab: StoreTab, installed: &crate::install::installed::InstalledIndex) {
        self.tab = tab;
        self.scroll_reset = true;
        self.focus_pane = FocusPane::Grid;
        match tab {
            StoreTab::Categories => {
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.favorites_filter = false;
                self.refresh_filter();
                self.sync_sidebar_cursor();
            }
            StoreTab::Library => {
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.apply_install_filter(installed, false);
            }
            StoreTab::Updates => {
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.apply_install_filter(installed, true);
            }
        }
    }
    fn apply_install_filter(&mut self, installed: &crate::install::installed::InstalledIndex, updates_only: bool) {
        use crate::install::installed::InstallState;
        self.filtered_indices = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|&idx| {
                let Some(app) = self.apps.get(idx) else { return false };
                let state = installed.state(app);
                if updates_only {
                    state == InstallState::Outdated
                } else {
                    matches!(state, InstallState::Installed | InstallState::Outdated)
                }
            })
            .collect();
        self.filtered_indices =
            collapse_homebrew_duplicates(&self.apps, std::mem::take(&mut self.filtered_indices));
        self.is_commercial_view = false;
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = 0;
            self.selection_active = true;
        }
    }
    fn sort_order_indices(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.apps.len()).collect();
        let apps = &self.apps;
        let direction = self.sort_direction;
        match self.sort_order {
            SortOrder::Downloads => order.sort_by(|&a, &b| {
                match direction {
                    SortDirection::Asc => apps[a].downloads.cmp(&apps[b].downloads),
                    SortDirection::Desc => apps[b].downloads.cmp(&apps[a].downloads),
                }
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
            }),
            SortOrder::Rating => order.sort_by(|&a, &b| {
                match direction {
                    SortDirection::Asc => apps[a].rating.total_cmp(&apps[b].rating),
                    SortDirection::Desc => apps[b].rating.total_cmp(&apps[a].rating),
                }
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
            }),
            SortOrder::Recent => order.sort_by(|&a, &b| {
                let ea = apps[a].sort_epoch();
                let eb = apps[b].sort_epoch();
                match direction {
                    SortDirection::Asc => ea.cmp(&eb),
                    SortDirection::Desc => eb.cmp(&ea),
                }
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
            }),
            SortOrder::Size => order.sort_by(|&a, &b| {
                match direction {
                    SortDirection::Asc => apps[a].size_bytes.cmp(&apps[b].size_bytes),
                    SortDirection::Desc => apps[b].size_bytes.cmp(&apps[a].size_bytes),
                }
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
            }),
            SortOrder::Name => order.sort_by(|&a, &b| match direction {
                SortDirection::Asc => apps[a].name_lower.cmp(&apps[b].name_lower),
                SortDirection::Desc => apps[b].name_lower.cmp(&apps[a].name_lower),
            }.then_with(|| apps[a].id.cmp(&apps[b].id))),
        }
        order
    }
    fn set_sort(&mut self, sort: SortOrder) {
        if self.sort_order == sort {
            self.sort_direction = self.sort_direction.flipped();
        } else {
            self.sort_order = sort;
            self.sort_direction = sort.default_direction();
        }
        self.resort();
        let mut settings = data::settings::load();
        settings.sort_order = self.sort_order;
        settings.sort_direction = self.sort_direction;
        data::settings::save(&settings);
    }
    fn flip_sort_direction(&mut self) {
        self.sort_direction = self.sort_direction.flipped();
        self.resort();
        let mut settings = data::settings::load();
        settings.sort_order = self.sort_order;
        settings.sort_direction = self.sort_direction;
        data::settings::save(&settings);
    }
    fn resort(&mut self) {
        self.sorted_indices = self.sort_order_indices();
        self.recompute_dropdown_counts();
        self.refresh_filter();
    }
    fn refresh_filter_preserving_selection(&mut self) {
        let selected_id = self
            .selection_active
            .then(|| self.filtered_indices.get(self.selected).copied())
            .flatten()
            .and_then(|idx| self.apps.get(idx))
            .map(|app| app.id.clone());
        self.refresh_filter();
        if let Some(id) = selected_id {
            if let Some(pos) = self.filtered_indices.iter().position(|&idx| self.apps[idx].id == id) {
                self.selected = pos;
                self.selection_active = true;
                self.scroll_reset = false;
                self.scroll_to_selected = true;
            }
        }
    }
    fn refresh_filter(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        let apps = &self.apps;
        let category_filter = self.category_filter;
        let content_type_group = self.content_type_group;
        let favorites_filter = self.favorites_filter;
        let genre_filter = self.genre_filter.as_deref();
        self.filtered_indices = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|&index| {
                let app = &apps[index];
                let matches_group = content_type_group.contains(app.category);
                let matches_cat = category_filter.is_none_or(|c| c as u8 == app.category as u8);
                let matches_fav = !favorites_filter || app.user_liked;
                let matches_genre = genre_filter.is_none_or(|genre| {
                    app.genres.iter().any(|app_genre| app_genre.eq_ignore_ascii_case(genre))
                });
                matches_group
                    && matches_cat
                    && matches_fav
                    && matches_genre
                    && (query.is_empty()
                        || app.name_lower.contains(&query)
                        || app.author_lower.contains(&query)
                        || app.titleid_lower.contains(&query)
                        || app.genres.iter().any(|genre| genre.to_lowercase().contains(&query)))
            })
            .collect();
        self.filtered_indices =
            collapse_homebrew_duplicates(&self.apps, std::mem::take(&mut self.filtered_indices));
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = 0;
            self.selection_active = true;
            self.scroll_reset = true;
        }
        self.is_commercial_view = self.content_type_group == ContentTypeGroup::Games;
    }
    fn recompute_dropdown_counts(&mut self) {
        let unique_all = collapse_homebrew_duplicates(&self.apps, (0..self.apps.len()).collect());
        let visible = unique_all.clone();
        let apps = &self.apps;
        self.source_scoped_count = visible.len();
        self.total_unique_count = unique_all.len();
        self.category_counts = Category::ALL
            .into_iter()
            .map(|category| {
                let count = visible.iter().filter(|&&index| apps[index].category == category).count();
                (category, count)
            })
            .filter(|&(_, count)| count > 0)
            .collect();
        let mut genres = std::collections::BTreeMap::<String, usize>::new();
        for &index in &visible {
            if !apps[index].platform.is_commercial() {
                continue;
            }
            for genre in &apps[index].genres {
                if !genre.eq_ignore_ascii_case("other") {
                    *genres.entry(genre.clone()).or_default() += 1;
                }
            }
        }
        self.genre_counts = genres.into_iter().collect();
        self.recompute_group_counts();
    }

    fn recompute_group_counts(&mut self) {
        let apps = &self.apps;
        let group = self.content_type_group;
        self.group_category_counts = group
            .sidebar_categories()
            .iter()
            .map(|&category| {
                let count = apps.iter().filter(|app| app.category == category).count();
                (category, count)
            })
            .collect();
        let indices: Vec<usize> = (0..apps.len())
            .filter(|&i| group.contains(apps[i].category))
            .collect();
        self.group_total = collapse_homebrew_duplicates(apps, indices).len();
    }

    fn move_selection(&mut self, delta: isize) -> bool {
        if self.filtered_indices.is_empty() {
            return false;
        }
        self.scroll_to_selected = true;
        if !self.selection_active {
            self.selection_active = true;
            self.selected = 0;
            return true;
        }
        let last = self.filtered_indices.len() as isize - 1;
        let target = (self.selected as isize + delta).clamp(0, last) as usize;
        if target == self.selected {
            return false;
        }
        self.selected = target;
        true
    }

    pub fn shows_category_pills(&self) -> bool {
        self.tab == StoreTab::Categories
    }

    pub fn shows_sidebar(&self) -> bool {
        self.tab == StoreTab::Categories
    }

    pub fn sync_sidebar_cursor(&mut self) {
        let slots = crate::app::ui::sidebar::sidebar_slots(&self.group_category_counts);
        self.sidebar_cursor = crate::app::ui::sidebar::active_sidebar_index(
            &slots,
            self.category_filter,
            self.favorites_filter,
            self.sort_order,
        );
    }

    fn enter_sidebar(&mut self) {
        if !self.shows_sidebar() {
            return;
        }
        self.sync_sidebar_cursor();
        self.focus_pane = FocusPane::Sidebar;
    }

    fn leave_sidebar(&mut self) {
        self.focus_pane = FocusPane::Grid;
        if !self.filtered_indices.is_empty() {
            self.selection_active = true;
        }
    }

    fn move_sidebar_cursor(&mut self, delta: isize) -> bool {
        let slots = crate::app::ui::sidebar::sidebar_slots(&self.group_category_counts);
        if slots.is_empty() {
            return false;
        }
        let last = slots.len() as isize - 1;
        let next = (self.sidebar_cursor as isize + delta).clamp(0, last) as usize;
        if next == self.sidebar_cursor && self.focus_pane == FocusPane::Sidebar {
            return false;
        }
        self.sidebar_cursor = next;
        self.focus_pane = FocusPane::Sidebar;
        self.apply_sidebar_slot_at_cursor();
        true
    }

    fn apply_sidebar_slot_at_cursor(&mut self) {
        let slots = crate::app::ui::sidebar::sidebar_slots(&self.group_category_counts);
        let Some(slot) = slots.get(self.sidebar_cursor).copied() else {
            return;
        };
        match slot {
            crate::app::ui::sidebar::SidebarSlot::All => {
                self.favorites_filter = false;
                self.category_filter = None;
                self.refresh_filter();
            }
            crate::app::ui::sidebar::SidebarSlot::Category(category) => {
                self.favorites_filter = false;
                self.category_filter = Some(category);
                self.refresh_filter();
            }
            crate::app::ui::sidebar::SidebarSlot::Featured => {
                self.favorites_filter = false;
                self.ensure_sort(SortOrder::Rating);
            }
            crate::app::ui::sidebar::SidebarSlot::Downloads => {
                self.favorites_filter = false;
                self.ensure_sort(SortOrder::Downloads);
            }
            crate::app::ui::sidebar::SidebarSlot::Recent => {
                self.favorites_filter = false;
                self.ensure_sort(SortOrder::Recent);
            }
            crate::app::ui::sidebar::SidebarSlot::Favorites => {
                self.set_favorites_filter(true);
            }
        }
    }

    fn ensure_sort(&mut self, sort: SortOrder) {
        if self.sort_order == sort {
            self.refresh_filter();
            return;
        }
        self.sort_order = sort;
        self.sort_direction = sort.default_direction();
        self.resort();
        let mut settings = data::settings::load();
        settings.sort_order = self.sort_order;
        settings.sort_direction = self.sort_direction;
        data::settings::save(&settings);
    }

    fn set_content_type_group(&mut self, group: ContentTypeGroup) {
        self.content_type_group = group;
        if let Some(cat) = self.category_filter
            && !group.contains(cat)
        {
            self.category_filter = None;
        }
        if group == ContentTypeGroup::Home {
            self.sort_order = SortOrder::Recent;
            self.sort_direction = SortDirection::Desc;
            self.resort();
        } else {
            self.recompute_group_counts();
        }
        self.scroll_reset = true;
        self.refresh_filter();
        self.sync_sidebar_cursor();
        self.focus_pane = FocusPane::Grid;
    }

    fn cycle_category_filter(&mut self) {
        let cats = self.content_type_group.sidebar_categories();
        let next = match self.category_filter {
            None => cats.first().copied(),
            Some(current) => {
                let idx = cats.iter().position(|&c| c == current).unwrap_or(0);
                if idx + 1 >= cats.len() {
                    None
                } else {
                    Some(cats[idx + 1])
                }
            }
        };
        self.category_filter = next;
        self.favorites_filter = false;
        self.scroll_category_into_view = true;
        self.refresh_filter();
        self.sync_sidebar_cursor();
    }

    fn set_favorites_filter(&mut self, on: bool) {
        self.favorites_filter = on;
        if on {
            self.category_filter = None;
        }
        self.refresh_filter();
        self.sync_sidebar_cursor();
    }
}


fn is_collapsible_homebrew(app: &AppEntry) -> bool {
    !app.titleid_lower.is_empty() && !app.platform.is_commercial()
}

fn prefers_homebrew_candidate(apps: &[AppEntry], candidate: usize, incumbent: usize) -> bool {
    let a = &apps[candidate];
    let b = &apps[incumbent];
    if crate::install::installed::version_is_older(&b.version, &a.version) {
        return true;
    }
    if crate::install::installed::version_is_older(&a.version, &b.version) {
        return false;
    }
    let icon_a = a.icon_url.is_some();
    let icon_b = b.icon_url.is_some();
    if icon_a != icon_b {
        return icon_a;
    }
    if a.downloads != b.downloads {
        return a.downloads > b.downloads;
    }
    a.sort_epoch() > b.sort_epoch()
}

fn collapse_homebrew_duplicates(apps: &[AppEntry], indices: Vec<usize>) -> Vec<usize> {
    use std::collections::{HashMap, HashSet};

    let mut winners: HashMap<&str, usize> = HashMap::new();
    for &idx in &indices {
        let app = &apps[idx];
        if !is_collapsible_homebrew(app) {
            continue;
        }
        let key = app.titleid_lower.as_str();
        match winners.get(key) {
            None => {
                winners.insert(key, idx);
            }
            Some(&cur) if prefers_homebrew_candidate(apps, idx, cur) => {
                winners.insert(key, idx);
            }
            _ => {}
        }
    }

    let mut emitted_keys = HashSet::new();
    let mut out = Vec::with_capacity(indices.len());
    for idx in indices {
        let app = &apps[idx];
        if is_collapsible_homebrew(app) {
            if winners.get(app.titleid_lower.as_str()) == Some(&idx)
                && emitted_keys.insert(app.titleid_lower.as_str())
            {
                out.push(idx);
            }
        } else {
            out.push(idx);
        }
    }
    out
}


pub enum AppState {
    Loading,
    Catalog(CatalogState),
    Detail {
        app: AppEntry,
        previous: Box<CatalogState>,
        scroll_delta: f32,
        comments: Vec<data::api::Comment>,
        comments_loaded: bool,
        comment_entry_requested: bool,
        lightbox: Option<usize>,
        data_prompt: bool,
        /// See `data::with_data_language`; reset when the prompt opens.
        data_language: usize,
        data_step: DataStep,
        uninstall_prompt: bool,
    },
    Settings {
        previous: Box<CatalogState>,
        nav: ui::settings::SettingsNav,
    },
}
enum CatalogSource {
    Live(Vec<AppEntry>),
    Failed,
}

const SELF_REPO_URL: &str = "https://github.com/josephinoo/vitaForge";
const SELF_UPDATE_ID: &str = "vitaforge_self_update";

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
    load_started_at: std::time::Instant,
    self_update_rx: Option<oneshot::Receiver<SelfUpdateInfo>>,
    comments_rx: Option<oneshot::Receiver<(String, Vec<data::api::Comment>)>>,
    social_rx: Option<oneshot::Receiver<(String, data::api::Social)>>,
    cache_stats_rx: Option<oneshot::Receiver<data::cache_manager::CacheStats>>,
    audio: crate::audio::AudioEngine,
    pub cache_stats: data::cache_manager::CacheStats,
    pub cache_notice: Option<String>,
    icons_need_clear: bool,
    pub purge_armed: bool,
    /// Set by the first ○ during a download; a second ○ within 3 s cancels it.
    pub cancel_armed_at: Option<std::time::Instant>,
    install_notifications: bool,
    update_queue: std::collections::VecDeque<AppEntry>,
    pub update_active: bool,
    pub update_total: usize,
    update_done: usize,
    update_failed: usize,
    uninstall_rx: Option<oneshot::Receiver<(String, String, Result<(), String>)>>,
    pub report_notice: Option<String>,
    restart_at: Option<std::time::Instant>,
    updatable_cache: std::sync::Mutex<Option<((usize, usize, usize), usize)>>,
}
const LOAD_WATCHDOG: std::time::Duration = std::time::Duration::from_secs(60);
pub struct InstallJob {
    pub app_id: String,
    pub app_id_title: String,
    pub title: String,
    pub progress: crate::install::Progress,
    rx: watch::Receiver<crate::install::Progress>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match data::source::fetch_live().await {
                Ok(apps) if !apps.is_empty() => CatalogSource::Live(apps),
                Ok(_) => CatalogSource::Failed,
                Err(err) => {
                    crate::install::log_file(&format!("live catalog fetch failed: {err:#}"));
                    eprintln!("live catalog fetch failed: {err:#}");
                    CatalogSource::Failed
                }
            };
            let _ = tx.send(result);
        });

        let (self_update_tx, self_update_rx) = oneshot::channel();
        tokio::spawn(async move {
            let Some(release) = crate::install::github::latest_release(SELF_REPO_URL).await else {
                return;
            };
            if crate::install::github::is_remote_newer(&release.tag, env!("CARGO_PKG_VERSION")) {
                let _ = self_update_tx.send(SelfUpdateInfo {
                    tag: release.tag,
                    vpk_url: release.vpk_url,
                });
            }
        });

        let settings = data::settings::load();
        ui::theme::set_theme(settings.theme);
        // So catalog outdated scans compare semver against this build, not missing SFO APP_VER.
        crate::install::installed::stamp_running_self();
        Ok(Self {
            state: AppState::Loading,
            icons: IconCache::new(),
            installed: crate::install::installed::InstalledIndex::new(),
            lang: settings.language.unwrap_or_else(Language::detect),
            install: None,
            self_update: None,
            needs_installed_rescan: true,
            load_rx: Some(rx),
            load_started_at: std::time::Instant::now(),
            self_update_rx: Some(self_update_rx),
            comments_rx: None,
            social_rx: None,
            cache_stats_rx: None,
            audio: crate::audio::AudioEngine::new(),
            cache_stats: data::cache_manager::CacheStats::default(),
            cache_notice: None,
            purge_armed: false,
            cancel_armed_at: None,
            icons_need_clear: false,
            install_notifications: settings.install_notifications,
            update_queue: std::collections::VecDeque::new(),
            update_active: false,
            update_total: 0,
            update_done: 0,
            update_failed: 0,
            uninstall_rx: None,
            report_notice: None,
            restart_at: None,
            updatable_cache: std::sync::Mutex::new(None),
        })
    }
    fn spawn_catalog_fetch(&mut self) {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match data::source::fetch_live().await {
                Ok(apps) if !apps.is_empty() => CatalogSource::Live(apps),
                Ok(_) => CatalogSource::Failed,
                Err(err) => {
                    crate::install::log_file(&format!("live catalog fetch failed: {err:#}"));
                    eprintln!("live catalog fetch failed: {err:#}");
                    CatalogSource::Failed
                }
            };
            let _ = tx.send(result);
        });
        self.load_rx = Some(rx);
        self.load_started_at = std::time::Instant::now();
    }
    fn refresh_cache_stats(&mut self) {
        let (tx, rx) = oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let stats = data::cache_manager::compute_cache_stats();
            let _ = tx.send(stats);
        });
        self.cache_stats_rx = Some(rx);
    }
    pub fn cancel_armed(&self) -> bool {
        self.cancel_armed_at.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(3))
    }
    /// A running install pins the user to the page that started it, except
    /// during "update all", where the queue runs on its own and they can browse.
    pub fn navigation_locked(&self) -> bool {
        self.install_busy() && !self.update_active
    }
    /// What "update all" would install: homebrew and plugins with a newer
    /// version, no extra data to pick, never the store itself.
    pub fn is_updatable(&self, entry: &AppEntry) -> bool {
        self.installed.state(entry) == crate::install::installed::InstallState::Outdated
            && matches!(
                entry.platform,
                crate::data::Platform::Vita | crate::data::Platform::Plugin
            )
            && entry.data_url.is_none()
            && !entry.download_url.is_empty()
            && detail_actions::removable_or_plugin(entry)
    }
    pub fn updatable_count(&self, apps: &[AppEntry]) -> usize {
        let (installed, outdated) = self.installed.counts();
        if outdated == 0 {
            return 0;
        }
        let key = (installed, outdated, apps.len());
        let mut cache = self.updatable_cache.lock().unwrap();
        if let Some((cached_key, count)) = *cache
            && cached_key == key
        {
            return count;
        }
        let mut seen = std::collections::HashSet::new();
        let count = apps
            .iter()
            .filter(|entry| self.is_updatable(entry))
            .filter(|entry| seen.insert(crate::install::installed::index_key(entry)))
            .count();
        *cache = Some((key, count));
        count
    }
    pub fn install_busy(&self) -> bool {
        self.install.as_ref().is_some_and(|job| !job.progress.is_finished())
    }
    pub fn tick(&mut self, ctx: &egui::Context) -> Result<()> {
        if self.restart_at.is_some_and(|at| std::time::Instant::now() >= at) {
            self.restart_at = None;
            if let Err(err) = crate::install::lifecycle::restart_self() {
                crate::install::log_file(&format!("restart after self-update failed: {err:#}"));
                crate::install::notify::send(self.lang.notify_restart_manual());
            }
        }
        if self.icons_need_clear {
            self.icons.clear_resident(ctx);
            self.icons_need_clear = false;
        }
        self.icons.publish_completed(ctx);
        if self.needs_installed_rescan {
            let entries: &[AppEntry] = match &self.state {
                AppState::Catalog(catalog) => &catalog.apps,
                AppState::Detail { previous, .. } | AppState::Settings { previous, .. } => &previous.apps,
                AppState::Loading => &[],
            };
            if self.installed.refresh(ctx, entries) {
                self.needs_installed_rescan = false;
                if let AppState::Catalog(catalog) = &mut self.state {
                    match catalog.tab {
                        StoreTab::Library => catalog.apply_install_filter(&self.installed, false),
                        StoreTab::Updates => catalog.apply_install_filter(&self.installed, true),
                        _ => {}
                    }
                }
            }
        }
        if let Some(job) = &mut self.install {
            let previous = std::mem::replace(&mut job.progress, job.rx.borrow_and_update().clone());
            if previous != job.progress && job.progress == crate::install::Progress::Restarting {
                self.restart_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(1500));
            }
            if previous != job.progress && job.progress == crate::install::Progress::RebootRequired {
                crate::install::requirements::invalidate_all();
            }
            if previous != job.progress && job.progress == crate::install::Progress::Done {
                self.installed.mark_installed(&job.app_id_title);
                crate::install::requirements::invalidate_all();
                if self.install_notifications {
                    crate::install::notify::install_finished(self.lang, &job.title);
                }
                let app_id = job.app_id.clone();
                tokio::spawn(async move {
                    if let Err(err) = data::api::notify_install(&app_id).await {
                        eprintln!("install counter notify failed: {err:#}");
                    }
                });
            } else if let crate::install::Progress::Failed(reason) = &job.progress
                && previous != job.progress
            {
                if self.install_notifications {
                    crate::install::notify::install_failed(self.lang, &job.title, reason);
                }
            }
        }
        self.advance_update_queue();
        if let Some(rx) = &mut self.uninstall_rx
            && let Ok((key, name, result)) = rx.try_recv()
        {
            self.uninstall_rx = None;
            match result {
                Ok(()) => {
                    self.installed.mark_removed(&key);
                    crate::install::requirements::invalidate_all();
                    self.needs_installed_rescan = true;
                    crate::install::notify::send(&self.lang.notify_uninstalled(&name));
                }
                Err(reason) => {
                    eprintln!("uninstall of {name} failed: {reason}");
                    crate::install::notify::send(&self.lang.notify_uninstall_failed(&name));
                }
            }
            ctx.request_repaint();
        }
        if let Some(rx) = &mut self.self_update_rx
            && let Ok(info) = rx.try_recv()
        {
            self.self_update_rx = None;
            if self.install_notifications {
                crate::install::notify::self_update_available(self.lang, &info.tag);
            }
            self.self_update = Some(info);
            ctx.request_repaint();
        }
        if let Some(rx) = &mut self.social_rx
            && let Ok((app_id, social)) = rx.try_recv()
        {
            self.social_rx = None;
            if let AppState::Detail { app, .. } = &mut self.state
                && app.id == app_id
            {
                app.rating = social.average_rating;
                app.ratings_count = social.ratings_count;
                app.likes_count = social.likes_count;
                app.user_liked = social.user_liked;
                app.user_rating = social.user_rating;
                ctx.request_repaint();
            }
        }
        if let Some(rx) = &mut self.comments_rx
            && let Ok((app_id, fetched)) = rx.try_recv()
        {
            self.comments_rx = None;
            if let AppState::Detail { app, comments, comments_loaded, .. } = &mut self.state
                && app.id == app_id
            {
                app.comments_count = fetched.len() as u32;
                *comments = fetched;
                *comments_loaded = true;
                ctx.request_repaint();
            }
        }
        if let Some(rx) = &mut self.cache_stats_rx
            && let Ok(stats) = rx.try_recv()
        {
            self.cache_stats_rx = None;
            self.cache_stats = stats;
            ctx.request_repaint();
        }
        let Some(rx) = &mut self.load_rx else { return Ok(()) };
        match rx.try_recv() {
            Ok(CatalogSource::Live(apps)) => {
                self.load_rx = None;
                if !self.install_busy() {
                    match &mut self.state {
                        AppState::Loading => {
                            let catalog = CatalogState::new(apps);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.icons.precache_background(precache_art_urls(&catalog.apps));
                            self.state = AppState::Catalog(catalog);
                        }
                        AppState::Catalog(catalog) => {
                            catalog.replace_apps(apps);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.icons.precache_background(precache_art_urls(&catalog.apps));
                        }
                        AppState::Detail { previous, .. } | AppState::Settings { previous, .. } => {
                            previous.replace_apps(apps);
                            self.installed.force_refresh(ctx, &previous.apps);
                        }
                    }
                    ctx.request_repaint();
                }
            }
            Ok(CatalogSource::Failed) => {
                self.load_rx = None;
                if matches!(self.state, AppState::Loading) && !self.install_busy() {
                    let apps = data::api::load_cached_catalog_sync().unwrap_or_default();
                    eprintln!("live catalog fetch failed; fallback to {} cached entries", apps.len());
                    let catalog = CatalogState::new(apps);
                    self.installed.force_refresh(ctx, &catalog.apps);
                    self.state = AppState::Catalog(catalog);
                    ctx.request_repaint();
                }
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                if matches!(self.state, AppState::Loading)
                    && self.load_started_at.elapsed() >= LOAD_WATCHDOG
                {
                    let apps = data::api::load_cached_catalog_sync().unwrap_or_default();
                    if !apps.is_empty() {
                        crate::install::log_file(&format!(
                            "catalog fetch passed {}s; showing {} cached entries while download continues",
                            LOAD_WATCHDOG.as_secs(),
                            apps.len(),
                        ));
                        let catalog = CatalogState::new(apps);
                        self.installed.force_refresh(ctx, &catalog.apps);
                        self.state = AppState::Catalog(catalog);
                        ctx.request_repaint();
                    }
                }
            }
            Err(oneshot::error::TryRecvError::Closed) => self.load_rx = None,
        }
        Ok(())
    }
    /// ✕ inside Settings: enter the rows, open a choice list, or run the row.
    fn settings_confirm(&mut self) -> Result<()> {
        use ui::settings::{Row, SubPage};
        use ui::theme::ThemeId;
        let AppState::Settings { nav, .. } = &mut self.state else { return Ok(()) };
        if let Some(sub) = nav.sub {
            let index = nav.sub_index;
            return match sub {
                SubPage::Language => self.handle_command(AppCommand::SetLanguage(Language::ALL[index])),
                SubPage::Theme => self.handle_command(AppCommand::SetTheme(ThemeId::ALL[index])),
            };
        }
        if nav.rail {
            nav.rail = nav.rows().is_empty();
            return Ok(());
        }
        let command = match nav.current_row() {
            Some(Row::Language) => {
                nav.sub = Some(SubPage::Language);
                nav.sub_index = Language::ALL.iter().position(|&l| l == self.lang).unwrap_or(0);
                return Ok(());
            }
            Some(Row::Theme) => {
                let theme = ui::theme::active_theme();
                nav.theme_before = theme;
                nav.sub = Some(SubPage::Theme);
                nav.sub_index = ThemeId::ALL.iter().position(|&t| t == theme).unwrap_or(0);
                return Ok(());
            }
            Some(Row::Notifications) => AppCommand::ToggleInstallNotifications,
            Some(Row::ClearIcons) => AppCommand::ClearIconCache,
            Some(Row::ClearCatalog) => AppCommand::ClearCatalogCache,
            Some(Row::PurgeAll) => AppCommand::PurgeAllCache,
            None => return Ok(()),
        };
        self.handle_command(command)
    }

    fn leave_settings(&mut self) {
        self.purge_armed = false;
        if let AppState::Settings { previous, nav } = &mut self.state {
            close_settings_sub(nav);
            let previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
            self.state = AppState::Catalog(previous);
            self.needs_installed_rescan = true;
            self.audio.play(crate::audio::Sfx::MenuFlyOut);
        }
    }

    pub fn handle_command(&mut self, command: AppCommand) -> Result<()> {
        match command {
            AppCommand::Input(InputCommand::Back) => {
                if matches!(self.state, AppState::Settings { .. }) {
                    return self.handle_command(AppCommand::CloseSettings);
                }
                if let AppState::Detail { lightbox: Some(_), .. } = &self.state {
                    return self.handle_command(AppCommand::CloseScreenshot);
                }
                if matches!(self.state, AppState::Detail { data_prompt: true, .. }) {
                    return self.handle_command(AppCommand::CancelDataPrompt);
                }
                if matches!(self.state, AppState::Detail { uninstall_prompt: true, .. }) {
                    return self.handle_command(AppCommand::CancelUninstall);
                }
                if matches!(self.state, AppState::Detail { .. })
                    && !self.update_active
                    && self.install.as_ref().is_some_and(|job| job.progress.is_cancellable())
                {
                    // ○ means "back" everywhere else, so one press must not throw away a download.
                    if !self.cancel_armed() {
                        self.cancel_armed_at = Some(std::time::Instant::now());
                        return Ok(());
                    }
                    self.cancel_armed_at = None;
                    return self.handle_command(AppCommand::CancelInstall);
                }
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.focus_pane == FocusPane::Sidebar {
                        catalog.leave_sidebar();
                        self.audio.play(crate::audio::Sfx::Navigate);
                        return Ok(());
                    }
                    if catalog.tab == StoreTab::Categories
                        && (catalog.category_filter.is_some()
                            || catalog.favorites_filter
                            || !catalog.search_query.is_empty()
                            || catalog.search_requested)
                    {
                        catalog.category_filter = None;
                        catalog.favorites_filter = false;
                        catalog.search_query.clear();
                        catalog.search_requested = false;
                        catalog.refresh_filter();
                        catalog.sync_sidebar_cursor();
                        return Ok(());
                    }
                    if catalog.tab == StoreTab::Categories {
                        return Ok(());
                    }
                }
                self.back_to_catalog();
            }
            AppCommand::SetSearchQuery(query) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.search_query = query;
                    if catalog.tab == StoreTab::Library {
                        catalog.apply_install_filter(&self.installed, false);
                    } else if catalog.tab == StoreTab::Updates {
                        catalog.apply_install_filter(&self.installed, true);
                    } else {
                        catalog.refresh_filter();
                    }
                }
            }
            AppCommand::RequestSearch => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.search_requested = true;
                    self.audio.play(crate::audio::Sfx::Typing);
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
                    catalog.favorites_filter = false;
                    if catalog.tab == StoreTab::Categories {
                        catalog.refresh_filter();
                    }
                    catalog.sync_sidebar_cursor();
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetContentTypeGroup(group) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.tab = StoreTab::Categories;
                    catalog.favorites_filter = false;
                    catalog.set_content_type_group(group);
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetFavoritesFilter(on) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.tab = StoreTab::Categories;
                    catalog.set_favorites_filter(on);
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::CycleCategoryFilter => {
                if matches!(self.state, AppState::Detail { .. }) {
                    return self.handle_command(AppCommand::SecondaryAction);
                }
                if matches!(&self.state, AppState::Catalog(catalog) if catalog.tab == StoreTab::Updates) {
                    return self.handle_command(AppCommand::UpdateAll);
                }
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.tab != StoreTab::Categories {
                        catalog.tab = StoreTab::Categories;
                    }
                    catalog.cycle_category_filter();
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetGenreFilter(genre) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.genre_filter = genre;
                    if catalog.tab == StoreTab::Categories {
                        catalog.refresh_filter();
                    }
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetSortOrder(sort) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.set_sort(sort);
                    catalog.sync_sidebar_cursor();
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::FlipSortDirection if matches!(self.state, AppState::Detail { .. }) => {
                return self.handle_command(AppCommand::TertiaryAction);
            }
            AppCommand::SecondaryAction
                if matches!(self.state, AppState::Detail { data_prompt: true, .. }) =>
            {
                match &self.state {
                    AppState::Detail { data_step: DataStep::Choice, .. } => {
                        return self.handle_command(AppCommand::InstallAppOnly);
                    }
                    AppState::Detail { data_step: DataStep::Method, .. } => {
                        return self.handle_command(AppCommand::InstallCurrent);
                    }
                    _ => {}
                }
            }
            AppCommand::TertiaryAction if matches!(self.state, AppState::Detail { data_prompt: true, .. }) => {}
            AppCommand::SecondaryAction => {
                if let AppState::Detail { app, lightbox: None, .. } = &self.state
                    && self.install.is_none()
                {
                    match detail_actions::secondary(app, self.installed.state(app)) {
                        Some(detail_actions::DetailAction::Open) => {
                            return self.handle_command(AppCommand::LaunchCurrent);
                        }
                        Some(detail_actions::DetailAction::Reinstall) => {
                            return self.handle_command(AppCommand::InstallCurrent);
                        }
                        _ => {}
                    }
                }
            }
            AppCommand::TertiaryAction => {
                if let AppState::Detail { app, lightbox: None, .. } = &self.state {
                    if let Some(job) = &self.install {
                        if matches!(job.progress, crate::install::Progress::Failed(_))
                            && job.progress.is_finished()
                        {
                            return self.handle_command(AppCommand::ReportError);
                        }
                    } else if detail_actions::tertiary(app, self.installed.state(app)).is_some() {
                        return self.handle_command(AppCommand::UninstallCurrent);
                    }
                }
            }
            AppCommand::LaunchCurrent => {
                if let AppState::Detail { app, .. } = &self.state
                    && !self.install_busy()
                    && self.installed.state(app) != crate::install::installed::InstallState::Absent
                    && detail_actions::launchable(app)
                {
                    match crate::install::lifecycle::launch(&app.titleid) {
                        Ok(()) => std::process::exit(0),
                        Err(err) => {
                            eprintln!("launch failed: {err:#}");
                            crate::install::notify::send(&self.lang.notify_open_failed(&app.name));
                        }
                    }
                }
            }
            AppCommand::UninstallCurrent => {
                let busy = self.install_busy() || self.uninstall_rx.is_some();
                let installed = &self.installed;
                if let AppState::Detail { app, uninstall_prompt, lightbox: None, .. } = &mut self.state
                    && !busy
                    && detail_actions::tertiary(app, installed.state(app)).is_some()
                {
                    *uninstall_prompt = true;
                    self.audio.play(crate::audio::Sfx::ShowModal);
                }
            }
            AppCommand::CancelUninstall => {
                if let AppState::Detail { uninstall_prompt, .. } = &mut self.state
                    && *uninstall_prompt
                {
                    *uninstall_prompt = false;
                    self.audio.play(crate::audio::Sfx::HideModal);
                }
            }
            AppCommand::ConfirmUninstall => {
                if let AppState::Detail { app, uninstall_prompt, .. } = &mut self.state
                    && *uninstall_prompt
                {
                    *uninstall_prompt = false;
                    let key = crate::install::installed::index_key(app);
                    let titleid = app.titleid.clone();
                    let name = app.name.clone();
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let result = crate::install::lifecycle::uninstall(&titleid)
                            .await
                            .map_err(|err| format!("{err:#}"));
                        let _ = tx.send((key, name, result));
                    });
                    self.uninstall_rx = Some(rx);
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::ReportError => {
                if let (AppState::Detail { app, .. }, Some(job)) = (&self.state, &self.install)
                    && let crate::install::Progress::Failed(reason) = &job.progress
                {
                    self.report_notice = Some(match crate::install::write_report(app, reason) {
                        Ok(path) => format!("Report saved to {path}"),
                        Err(err) => format!("Couldn't save the report: {err:#}"),
                    });
                }
            }
            AppCommand::InstallDependency { id, dir, file } => {
                if self.install_busy() {
                    return Ok(());
                }
                let found = match &self.state {
                    AppState::Detail { previous, .. } => {
                        previous.apps.iter().find(|candidate| candidate.id == id).cloned()
                    }
                    _ => None,
                };
                if let Some(mut entry) = found {
                    // The requirement says where the file must end up; a plugin
                    // that has no taiHEN entry of its own gets one there.
                    let dir = dir.trim_end_matches('/');
                    if entry.plugin_config_line.is_none()
                        && (dir.starts_with("ur0:tai") || dir.starts_with("ux0:tai"))
                    {
                        let section = if file.to_lowercase().ends_with(".skprx") {
                            "*KERNEL"
                        } else {
                            "*main"
                        };
                        entry.plugin_install_path = Some(format!("{dir}/"));
                        entry.plugin_config_line = Some(format!("{dir}/{file}"));
                        entry.plugin_config_section = Some(section.to_owned());
                    }
                    self.start_install(entry);
                }
            }
            AppCommand::ActivatePlugin { path, section } => {
                if let Ok(mut config) = crate::install::tai::TaiConfig::load_active() {
                    let sec = section.unwrap_or_else(|| {
                        if path.to_lowercase().ends_with(".skprx") {
                            "*KERNEL".to_string()
                        } else {
                            "*main".to_string()
                        }
                    });
                    config.add_or_update_plugin(&sec, &path, true);
                    let _ = config.commit_transaction();
                    crate::install::requirements::invalidate_all();
                }
            }
            AppCommand::TogglePlugin { path, enabled } => {
                if let Ok(mut config) = crate::install::tai::TaiConfig::load_active() {
                    config.set_plugin_enabled(&path, enabled);
                    let _ = config.commit_transaction();
                    crate::install::requirements::invalidate_all();
                }
            }
            AppCommand::RemovePlugin { path } => {
                if let Ok(mut config) = crate::install::tai::TaiConfig::load_active() {
                    config.remove_plugin(&path);
                    let _ = config.commit_transaction();
                    crate::install::requirements::invalidate_all();
                }
            }
            AppCommand::ConsolidateTaiConfig => {
                let _ = crate::install::tai::TaiConfig::consolidate_ux0_into_ur0();
                crate::install::requirements::invalidate_all();
            }
            AppCommand::AutoSortKernel => {
                if let Ok(mut config) = crate::install::tai::TaiConfig::load_active() {
                    if config.auto_sort_kernel() {
                        let _ = config.commit_transaction();
                        crate::install::requirements::invalidate_all();
                    }
                }
            }
            AppCommand::SafeReboot => {
                let _ = crate::install::tai::safe_reboot();
            }
            AppCommand::UpdateAll => {
                if self.update_active || self.install_busy() {
                    return Ok(());
                }
                let AppState::Catalog(catalog) = &self.state else {
                    return Ok(());
                };
                let mut seen = std::collections::HashSet::new();
                let queue: std::collections::VecDeque<AppEntry> = catalog
                    .apps
                    .iter()
                    .filter(|candidate| self.is_updatable(candidate))
                    .filter(|candidate| {
                        seen.insert(crate::install::installed::index_key(candidate))
                    })
                    .cloned()
                    .collect();
                if queue.is_empty() {
                    return Ok(());
                }
                self.update_total = queue.len();
                self.update_done = 0;
                self.update_failed = 0;
                self.update_queue = queue;
                self.update_active = true;
                self.advance_update_queue();
            }
            AppCommand::ScreenshotStep(step) => {
                if let AppState::Detail { lightbox: Some(index), app, .. } = &mut self.state {
                    let count = app.screenshot_urls.len() as i32;
                    if count > 1 {
                        *index = (*index as i32 + step).rem_euclid(count) as usize;
                        self.audio.play(crate::audio::Sfx::Navigate);
                    }
                }
            }
            AppCommand::FlipSortDirection => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.flip_sort_direction();
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::Input(direction @ (InputCommand::MoveUp | InputCommand::MoveDown))
                if matches!(
                    &self.state,
                    AppState::Detail { data_prompt: true, data_step: DataStep::Language, .. }
                ) =>
            {
                if let AppState::Detail { app, data_language, .. } = &self.state {
                    let options = app.game_data_languages.len() + 1;
                    let step = if direction == InputCommand::MoveUp { options - 1 } else { 1 };
                    return self.handle_command(AppCommand::SelectDataLanguage((*data_language + step) % options));
                }
            }
            AppCommand::Input(
                direction @ (InputCommand::MoveLeft
                | InputCommand::MoveRight
                | InputCommand::CategoryPrev
                | InputCommand::CategoryNext),
            ) if matches!(self.state, AppState::Detail { lightbox: Some(_), .. }) => {
                let step = match direction {
                    InputCommand::MoveLeft | InputCommand::CategoryPrev => -1,
                    _ => 1,
                };
                return self.handle_command(AppCommand::ScreenshotStep(step));
            }
            AppCommand::Input(InputCommand::Confirm) => {
                if let AppState::Detail { data_prompt: true, data_step, .. } = &self.state {
                    return self.handle_command(match data_step {
                        DataStep::Choice => AppCommand::WantGameData,
                        DataStep::Language => AppCommand::ConfirmDataLanguage,
                        // Heavy data: the recommended path is copying it from a PC.
                        DataStep::Method => AppCommand::InstallAppOnly,
                    });
                }
                if matches!(self.state, AppState::Detail { uninstall_prompt: true, .. }) {
                    return self.handle_command(AppCommand::ConfirmUninstall);
                }
                if let AppState::Detail { lightbox: None, app, .. } = &self.state {
                    let primary = detail_actions::primary(app, self.installed.state(app));
                    return match &self.install {
                        None if primary == detail_actions::DetailAction::Open => {
                            self.handle_command(AppCommand::LaunchCurrent)
                        }
                        None => self.handle_command(AppCommand::InstallCurrent),
                        Some(job) if job.progress.is_finished() => {
                            self.handle_command(AppCommand::DismissInstall)
                        }
                        Some(_) => Ok(()),
                    };
                }
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.focus_pane == FocusPane::Sidebar {
                        catalog.leave_sidebar();
                        self.audio.play(crate::audio::Sfx::Navigate);
                        return Ok(());
                    }
                }
                let target = match &mut self.state {
                    AppState::Catalog(catalog) => {
                        if !catalog.filtered_indices.is_empty() {
                            catalog.selection_active = true;
                            Some(catalog.selected)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(index) = target {
                    self.open_app(index);
                } else if matches!(self.state, AppState::Settings { .. }) {
                    return self.settings_confirm();
                }
            }
            AppCommand::Input(step @ (InputCommand::CategoryPrev | InputCommand::CategoryNext))
                if matches!(self.state, AppState::Settings { .. }) =>
            {
                let AppState::Settings { nav, .. } = &self.state else { unreachable!() };
                let count = ui::settings::SECTIONS.len();
                let section = if step == InputCommand::CategoryPrev { nav.section + count - 1 } else { nav.section + 1 };
                return self.handle_command(AppCommand::SettingsSection(section % count));
            }
            AppCommand::SettingsSection(section) => {
                self.purge_armed = false;
                self.cache_notice = None;
                if let AppState::Settings { nav, .. } = &mut self.state {
                    close_settings_sub(nav);
                    nav.go_section(section);
                    self.audio.play(crate::audio::Sfx::Navigate);
                }
            }
            AppCommand::SettingsRow(row) => {
                if let AppState::Settings { nav, .. } = &mut self.state {
                    nav.rail = false;
                    nav.row = row;
                }
                return self.settings_confirm();
            }
            AppCommand::SetTheme(theme) => {
                ui::theme::set_theme(theme);
                data::settings::set_theme(theme);
                if let AppState::Settings { nav, .. } = &mut self.state {
                    nav.theme_before = theme;
                    nav.sub = None;
                }
            }
            AppCommand::Input(InputCommand::CategoryPrev) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    match catalog.tab {
                        StoreTab::Categories => {
                            let groups = ContentTypeGroup::ALL;
                            let idx = groups
                                .iter()
                                .position(|&g| g == catalog.content_type_group)
                                .unwrap_or(0);
                            if idx == 0 {
                                return self.handle_command(AppCommand::SetStoreTab(StoreTab::Updates));
                            }
                            catalog.set_content_type_group(groups[idx - 1]);
                            self.audio.play(crate::audio::Sfx::TabTransition);
                            return Ok(());
                        }
                        StoreTab::Library => {
                            let last = ContentTypeGroup::ALL[ContentTypeGroup::ALL.len() - 1];
                            catalog.tab = StoreTab::Categories;
                            catalog.set_content_type_group(last);
                            self.audio.play(crate::audio::Sfx::TabTransition);
                            return Ok(());
                        }
                        StoreTab::Updates => {
                            return self.handle_command(AppCommand::SetStoreTab(StoreTab::Library));
                        }
                    }
                }
            }
            AppCommand::Input(InputCommand::CategoryNext) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    match catalog.tab {
                        StoreTab::Categories => {
                            let groups = ContentTypeGroup::ALL;
                            let idx = groups
                                .iter()
                                .position(|&g| g == catalog.content_type_group)
                                .unwrap_or(0);
                            if idx + 1 >= groups.len() {
                                return self.handle_command(AppCommand::SetStoreTab(StoreTab::Library));
                            }
                            catalog.set_content_type_group(groups[idx + 1]);
                            self.audio.play(crate::audio::Sfx::TabTransition);
                            return Ok(());
                        }
                        StoreTab::Library => {
                            return self.handle_command(AppCommand::SetStoreTab(StoreTab::Updates));
                        }
                        StoreTab::Updates => {
                            catalog.tab = StoreTab::Categories;
                            catalog.set_content_type_group(ContentTypeGroup::Home);
                            self.audio.play(crate::audio::Sfx::TabTransition);
                            return Ok(());
                        }
                    }
                }
            }
            AppCommand::Input(direction) => {
                match &mut self.state {
                    AppState::Catalog(catalog) => {
                        if catalog.focus_pane == FocusPane::Sidebar
                            && catalog.shows_sidebar()
                        {
                            match direction {
                                InputCommand::MoveUp => {
                                    if catalog.move_sidebar_cursor(-1) {
                                        self.audio.play(crate::audio::Sfx::Navigate);
                                    }
                                }
                                InputCommand::MoveDown => {
                                    if catalog.move_sidebar_cursor(1) {
                                        self.audio.play(crate::audio::Sfx::Navigate);
                                    }
                                }
                                InputCommand::MoveRight => {
                                    catalog.leave_sidebar();
                                    self.audio.play(crate::audio::Sfx::Navigate);
                                }
                                InputCommand::MoveLeft => {}
                                _ => {}
                            }
                            return Ok(());
                        }

                        let columns = crate::app::ui::GRID_COLUMNS as isize;
                        let at_left_edge = !catalog.selection_active
                            || catalog.selected % crate::app::ui::GRID_COLUMNS == 0;
                        if matches!(direction, InputCommand::MoveLeft)
                            && at_left_edge
                            && catalog.shows_sidebar()
                        {
                            catalog.enter_sidebar();
                            self.audio.play(crate::audio::Sfx::Navigate);
                            return Ok(());
                        }

                        let delta = match direction {
                            InputCommand::MoveLeft => -1,
                            InputCommand::MoveRight => 1,
                            InputCommand::MoveUp => -columns,
                            InputCommand::MoveDown => columns,
                            _ => 0,
                        };
                        if delta != 0 && catalog.move_selection(delta) {
                            self.audio.play(crate::audio::Sfx::Navigate);
                        }
                    }
                    AppState::Detail { scroll_delta, .. } => {
                        match direction {
                            InputCommand::MoveUp => *scroll_delta -= 60.0,
                            InputCommand::MoveDown => *scroll_delta += 60.0,
                            _ => {}
                        }
                    }
                    AppState::Settings { nav, .. } => {
                        self.purge_armed = false;
                        self.cache_notice = None;
                        let before = (nav.section, nav.row, nav.rail, nav.sub_index);
                        if nav.sub.is_some() {
                            let last = nav.sub_len().saturating_sub(1);
                            match direction {
                                InputCommand::MoveUp => nav.sub_index = nav.sub_index.saturating_sub(1),
                                InputCommand::MoveDown => nav.sub_index = (nav.sub_index + 1).min(last),
                                _ => {}
                            }
                            // Live preview: the whole app recolors as the cursor moves.
                            if nav.sub == Some(ui::settings::SubPage::Theme) {
                                ui::theme::set_theme(ui::theme::ThemeId::ALL[nav.sub_index]);
                            }
                        } else if nav.rail {
                            match direction {
                                InputCommand::MoveUp if nav.section > 0 => nav.go_section(nav.section - 1),
                                InputCommand::MoveDown if nav.section + 1 < ui::settings::SECTIONS.len() => {
                                    nav.go_section(nav.section + 1)
                                }
                                InputCommand::MoveRight if !nav.rows().is_empty() => nav.rail = false,
                                _ => {}
                            }
                        } else {
                            let last = nav.rows().len().saturating_sub(1);
                            match direction {
                                InputCommand::MoveUp => nav.row = nav.row.saturating_sub(1),
                                InputCommand::MoveDown => nav.row = (nav.row + 1).min(last),
                                InputCommand::MoveLeft => nav.rail = true,
                                _ => {}
                            }
                        }
                        if before != (nav.section, nav.row, nav.rail, nav.sub_index) {
                            self.audio.play(crate::audio::Sfx::Navigate);
                        }
                    }
                    _ => {}
                }
            }
            AppCommand::SelectApp { index } => self.open_app(index),
            AppCommand::SelectAppById(id) => self.open_app_by_id(&id),
            AppCommand::SetStoreTab(tab) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.apply_store_tab(tab, &self.installed);
                    self.audio.play(crate::audio::Sfx::TabTransition);
                } else if matches!(self.state, AppState::Detail { .. }) {
                    if self.navigation_locked() {
                        return Ok(());
                    }
                    let AppState::Detail { app, previous, .. } =
                        std::mem::replace(&mut self.state, AppState::Loading)
                    else {
                        unreachable!()
                    };
                    let mut previous = *previous;
                    if let Some(original) = previous.apps.iter_mut().find(|other| other.id == app.id) {
                        original.user_liked = app.user_liked;
                        original.user_rating = app.user_rating;
                        original.likes_count = app.likes_count;
                        original.ratings_count = app.ratings_count;
                        original.comments_count = app.comments_count;
                        original.rating = app.rating;
                    }
                    previous.apply_store_tab(tab, &self.installed);
                    self.state = AppState::Catalog(previous);
                    self.needs_installed_rescan = true;
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::MoreByAuthor(author) => {
                if self.navigation_locked() {
                    return Ok(());
                }
                if let AppState::Detail { app, previous, .. } = &mut self.state {
                    if let Some(original) = previous.apps.iter_mut().find(|other| other.id == app.id) {
                        original.user_liked = app.user_liked;
                        original.user_rating = app.user_rating;
                        original.likes_count = app.likes_count;
                        original.ratings_count = app.ratings_count;
                        original.comments_count = app.comments_count;
                        original.rating = app.rating;
                    }
                    let mut previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
                    previous.tab = StoreTab::Categories;
                                        previous.search_query = author;
                    previous.category_filter = None;
                    previous.refresh_filter();
                    self.state = AppState::Catalog(previous);
                    self.needs_installed_rescan = true;
                    self.audio.play(crate::audio::Sfx::OutOfDetail);
                }
            }
            AppCommand::OpenScreenshot(index) => {
                if let AppState::Detail { lightbox, app, .. } = &mut self.state
                    && index < app.screenshot_urls.len()
                {
                    *lightbox = Some(index);
                }
            }
            AppCommand::CloseScreenshot => {
                if let AppState::Detail { lightbox, .. } = &mut self.state {
                    *lightbox = None;
                }
            }
            AppCommand::BackToCatalog => self.back_to_catalog(),
            AppCommand::InstallCurrent => {
                let busy = self.install.as_ref().is_some_and(|job| !job.progress.is_finished());
                if let AppState::Detail { app, data_prompt, data_language, data_step, .. } =
                    &mut self.state
                    && !busy
                    && app.data_url.is_some()
                    && !*data_prompt
                {
                    *data_prompt = true;
                    *data_step = DataStep::Choice;
                    *data_language = preferred_data_language(app, self.lang);
                    self.audio.play(crate::audio::Sfx::ShowModal);
                    return Ok(());
                }
                if let AppState::Detail { app, data_language, .. } = &self.state
                    && !busy
                {
                    let entry = data::with_data_language(app, *data_language);
                    if crate::install::space_shortfall(&entry).is_none() {
                        self.start_install(entry);
                    }
                }
            }
            AppCommand::WantGameData => {
                if let AppState::Detail { app, data_prompt: true, data_step, .. } = &mut self.state
                    && *data_step == DataStep::Choice
                {
                    if !app.game_data_languages.is_empty() {
                        *data_step = DataStep::Language;
                        self.audio.play(crate::audio::Sfx::Activation);
                        return Ok(());
                    }
                    return self.continue_with_data();
                }
            }
            AppCommand::ConfirmDataLanguage => {
                if matches!(self.state, AppState::Detail { data_prompt: true, data_step: DataStep::Language, .. }) {
                    return self.continue_with_data();
                }
            }
            AppCommand::SelectDataLanguage(choice) => {
                if let AppState::Detail { app, data_prompt: true, data_language, .. } = &mut self.state
                    && choice <= app.game_data_languages.len()
                    && *data_language != choice
                {
                    *data_language = choice;
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::InstallAppOnly => {
                let busy = self.install.as_ref().is_some_and(|job| !job.progress.is_finished());
                if let AppState::Detail { app, data_prompt, .. } = &mut self.state
                    && !busy
                {
                    *data_prompt = false;
                    let mut entry = app.clone();
                    entry.data_url = None;
                    entry.data_size_bytes = 0;
                    if crate::install::space_shortfall(&entry).is_none() {
                        self.start_install(entry);
                    }
                }
            }
            AppCommand::CancelDataPrompt => {
                if let AppState::Detail { data_prompt, .. } = &mut self.state
                    && *data_prompt
                {
                    *data_prompt = false;
                    self.audio.play(crate::audio::Sfx::HideModal);
                }
            }
            AppCommand::DismissInstall => self.install = None,
            AppCommand::SelfUpdate => {
                let busy = self.install.as_ref().is_some_and(|job| !job.progress.is_finished());
                if let Some(info) = self.self_update.clone()
                    && !busy
                {
                    let mut entry = AppEntry {
                        id: SELF_UPDATE_ID.to_owned(),
                        titleid: "VITAFORGE".to_owned(),
                        titleid_lower: String::new(),
                        content_id: None,
                        name: format!("VitaForge {}", info.tag),
                        original_name: None,
                        name_lower: String::new(),
                        author: "josephinoo".to_owned(),
                        author_lower: String::new(),
                        description: format!("Self-update for VitaForge {}", info.tag),
                        long_description: String::new(),
                        requirements: String::new(),
                        changelog: String::new(),
                        release_page: Some("https://github.com/josephinoo/vitaForge/releases".to_owned()),
                        category: Category::Utility,
                        genres: Vec::new(),
                        platform: crate::data::Platform::Vita,
                        kind: String::new(),
                        icon_url: None,
                        cover_url: None,
                        background_url: None,
                        screenshot_urls: Vec::new(),
                        download_url: info.vpk_url.clone(),
                        source: None,
                        version: info.tag.clone(),
                        region: None,
                        zrif: None,
                        source_catalog: "vitadb".to_owned(),
                        source_labels: Vec::new(),
                        hash: String::new(),
                        hash2: String::new(),
                        data_url: None,
                        data_extract_path: None,
                        data_size_bytes: 0,
                        game_data_languages: Vec::new(),
                        plugin_install_path: None,
                        plugin_config_section: None,
                        plugin_config_line: None,
                        size_bytes: 0,
                        downloads: 0,
                        rating: 5.0,
                        updated_at: String::new(),
                        ratings_count: 0,
                        likes_count: 0,
                        comments_count: 0,
                        user_liked: false,
                        user_rating: None,
                        overview: Vec::new(),
                    };
                    entry.rebuild_derived();
                    let app_id = entry.id.clone();
                    let app_id_title = entry.titleid.clone();
                    let title = entry.name.clone();
                    let (rx, cancel) = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob {
                        app_id,
                        app_id_title,
                        title: title.clone(),
                        progress,
                        rx,
                        cancel,
                    });
                    self.audio.play(crate::audio::Sfx::Launch);
                    if self.install_notifications {
                        crate::install::notify::install_started(self.lang, &title);
                    }
                }
            }
            AppCommand::CancelInstall => {
                if self.update_active {
                    self.update_queue.clear();
                    self.update_active = false;
                }
                if let Some(job) = &self.install
                    && job.progress.is_cancellable()
                {
                    job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            AppCommand::OpenSettings => {
                let previous = match std::mem::replace(&mut self.state, AppState::Loading) {
                    AppState::Catalog(catalog) => Box::new(catalog),
                    AppState::Detail { previous, .. } => previous,
                    // Start toggles: pressing it inside Settings leaves.
                    other @ AppState::Settings { .. } => {
                        self.state = other;
                        self.leave_settings();
                        return Ok(());
                    }
                    other @ AppState::Loading => {
                        self.state = other;
                        return Ok(());
                    }
                };
                self.refresh_cache_stats();
                self.cache_notice = None;
                self.state = AppState::Settings { previous, nav: ui::settings::SettingsNav::new() };
                self.audio.play(crate::audio::Sfx::MenuFlyIn);
            }
            AppCommand::CloseSettings => {
                // ○ backs out one level: an open choice list first, then Settings itself.
                self.purge_armed = false;
                if let AppState::Settings { nav, .. } = &mut self.state {
                    if nav.sub.is_some() {
                        close_settings_sub(nav);
                        return Ok(());
                    }
                }
                self.leave_settings();
            }
            AppCommand::SetLanguage(lang) => {
                self.lang = lang;
                data::settings::set_language(lang);
                if let AppState::Settings { nav, .. } = &mut self.state {
                    nav.sub = None;
                }
            }
            AppCommand::ToggleInstallNotifications => {
                self.install_notifications = !self.install_notifications;
                data::settings::set_install_notifications(self.install_notifications);
            }
            AppCommand::ClearIconCache => {
                let freed = data::cache_manager::clear_icon_cache();
                self.icons.clear_disk_index();
                self.icons_need_clear = true;
                self.refresh_cache_stats();
                self.cache_notice =
                    Some(self.lang.settings_cleared_icons(&data::cache_manager::format_bytes(freed)));
            }
            AppCommand::ClearCatalogCache => {
                let _ = data::cache_manager::clear_catalog_cache();
                self.spawn_catalog_fetch();
                self.refresh_cache_stats();
                self.cache_notice = Some(self.lang.settings_cleared_catalog().to_owned());
            }
            AppCommand::PurgeAllCache => {
                // Destructive: the first press only arms it, the second one purges.
                self.purge_armed = !self.purge_armed;
                if self.purge_armed {
                    return Ok(());
                }
                let freed = data::cache_manager::purge_all_cache();
                self.icons.clear_disk_index();
                self.icons_need_clear = true;
                self.spawn_catalog_fetch();
                self.refresh_cache_stats();
                self.cache_notice =
                    Some(self.lang.settings_purged_all(&data::cache_manager::format_bytes(freed)));
            }
            AppCommand::ToggleLike => {
                if let AppState::Detail { app, .. } = &mut self.state {
                    let liked = !app.user_liked;
                    app.user_liked = liked;
                    app.likes_count = if liked {
                        app.likes_count.saturating_add(1)
                    } else {
                        app.likes_count.saturating_sub(1)
                    };
                    let app_id = app.id.clone();
                    tokio::spawn(async move {
                        if let Err(err) = data::api::set_like(&app_id, liked).await {
                            eprintln!("like update failed: {err:#}");
                        }
                    });
                    self.audio.play(if liked { crate::audio::Sfx::ToggleOn } else { crate::audio::Sfx::ToggleOff });
                }
            }
            AppCommand::RateCurrent(score) => {
                if let AppState::Detail { app, .. } = &mut self.state {
                    if app.user_rating.is_none() {
                        app.ratings_count = app.ratings_count.saturating_add(1);
                        let total = app.ratings_count as f32;
                        app.rating = if total > 1.0 {
                            (app.rating * (total - 1.0) + score as f32) / total
                        } else {
                            score as f32
                        };
                    }
                    app.user_rating = Some(score);
                    let app_id = app.id.clone();
                    tokio::spawn(async move {
                        if let Err(err) = data::api::post_rating(&app_id, score).await {
                            eprintln!("rating submit failed: {err:#}");
                        }
                    });
                }
            }
            AppCommand::RequestCommentEntry => {
                if let AppState::Detail { comment_entry_requested, .. } = &mut self.state {
                    *comment_entry_requested = true;
                    self.audio.play(crate::audio::Sfx::ShowModal);
                }
            }
            AppCommand::CloseCommentEntry => {
                if let AppState::Detail { comment_entry_requested, .. } = &mut self.state {
                    *comment_entry_requested = false;
                    self.audio.play(crate::audio::Sfx::HideModal);
                }
            }
            AppCommand::SubmitComment(content) => {
                let content = content.trim().to_owned();
                if !content.is_empty()
                    && let AppState::Detail { app, comments, .. } = &mut self.state
                {
                    let author_name = data::client_id::display_name();
                    comments.insert(
                        0,
                        data::api::Comment { author_name: author_name.clone(), content: content.clone() },
                    );
                    app.comments_count = app.comments_count.saturating_add(1);
                    let app_id = app.id.clone();
                    tokio::spawn(async move {
                        if let Err(err) = data::api::post_comment(&app_id, &author_name, &content).await {
                            eprintln!("comment submit failed: {err:#}");
                        }
                    });
                }
            }
        }
        Ok(())
    }
    /// After choosing game data (and its language): heavy data asks how to get it,
    /// light data starts downloading right away.
    fn continue_with_data(&mut self) -> Result<()> {
        if let AppState::Detail { app, data_language, data_step, .. } = &mut self.state
            && crate::data::is_heavy_data(&data::with_data_language(app, *data_language))
        {
            *data_step = DataStep::Method;
            self.audio.play(crate::audio::Sfx::Activation);
            return Ok(());
        }
        self.handle_command(AppCommand::InstallCurrent)
    }
    fn start_install(&mut self, entry: AppEntry) {
        let app_id = entry.id.clone();
        let app_id_title = crate::install::installed::index_key(&entry);
        let title = entry.name.clone();
        let (rx, cancel) = crate::install::start(entry);
        let progress = rx.borrow().clone();
        self.install = Some(InstallJob {
            app_id,
            app_id_title,
            title: title.clone(),
            progress,
            rx,
            cancel,
        });
        self.report_notice = None;
        self.audio.play(crate::audio::Sfx::Launch);
        if self.install_notifications {
            crate::install::notify::install_started(self.lang, &title);
        }
        if let AppState::Detail { data_prompt, .. } = &mut self.state {
            *data_prompt = false;
        }
    }
    fn advance_update_queue(&mut self) {
        if !self.update_active {
            return;
        }
        if let Some(job) = &self.install {
            if !job.progress.is_finished() {
                return;
            }
            match &job.progress {
                crate::install::Progress::Failed(_) => self.update_failed += 1,
                _ => self.update_done += 1,
            }
            self.install = None;
        }
        if let Some(next) = self.update_queue.pop_front() {
            self.start_install(next);
        } else {
            self.update_active = false;
            self.needs_installed_rescan = true;
            crate::install::notify::updates_finished(
                self.lang,
                self.update_done,
                self.update_failed,
            );
        }
    }
    pub fn update_all_label(&self) -> Option<String> {
        if !self.update_active {
            return None;
        }
        let position = (self.update_done + self.update_failed + 1).min(self.update_total);
        let percent = self
            .install
            .as_ref()
            .and_then(|job| job.progress.fraction())
            .map(|fraction| format!(" {}%", (fraction * 100.0) as u32))
            .unwrap_or_default();
        Some(format!("{position}/{}{percent}", self.update_total))
    }
    fn open_app(&mut self, filtered_index: usize) {
        let entry = match &self.state {
            AppState::Catalog(catalog) => catalog
                .filtered_indices
                .get(filtered_index)
                .and_then(|&real_index| catalog.apps.get(real_index))
                .cloned(),
            _ => None,
        };
        let Some(entry) = entry else { return };
        self.open_entry(entry, Some(filtered_index));
    }
    fn open_app_by_id(&mut self, id: &str) {
        let entry = match &self.state {
            AppState::Catalog(catalog) => catalog.apps.iter().find(|app| app.id == id).cloned(),
            _ => None,
        };
        let Some(entry) = entry else { return };
        let filtered_index = match &self.state {
            AppState::Catalog(catalog) => catalog
                .filtered_indices
                .iter()
                .position(|&idx| catalog.apps.get(idx).is_some_and(|app| app.id == id)),
            _ => None,
        };
        self.open_entry(entry, filtered_index);
    }
    fn open_entry(&mut self, entry: AppEntry, filtered_index: Option<usize>) {
        let placeholder = AppState::Detail {
            app: entry.clone(),
            previous: Box::new(CatalogState::empty()),
            scroll_delta: 0.0,
            comments: Vec::new(),
            comments_loaded: false,
            comment_entry_requested: false,
            data_prompt: false,
            data_language: 0,
            data_step: DataStep::Choice,
            uninstall_prompt: false,
            lightbox: None,
        };
        let AppState::Catalog(mut catalog) = std::mem::replace(&mut self.state, placeholder) else {
            unreachable!("checked above that self.state is Catalog")
        };
        if let Some(filtered_index) = filtered_index {
            catalog.selected = filtered_index;
            catalog.selection_active = true;
        }
        let app_id = entry.id.clone();
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            match data::api::fetch_comments(&app_id).await {
                Ok(comments) => {
                    let _ = tx.send((app_id, comments));
                }
                Err(err) => eprintln!("comments fetch failed: {err:#}"),
            }
        });
        self.comments_rx = Some(rx);
        let social_id = entry.id.clone();
        let (social_tx, social_rx) = oneshot::channel();
        tokio::spawn(async move {
            match data::api::fetch_social(&social_id).await {
                Ok(social) => {
                    let _ = social_tx.send((social_id, social));
                }
                Err(err) => eprintln!("social state fetch failed: {err:#}"),
            }
        });
        self.social_rx = Some(social_rx);
        self.state = AppState::Detail {
            app: entry,
            previous: Box::new(catalog),
            scroll_delta: 0.0,
            comments: Vec::new(),
            comments_loaded: false,
            comment_entry_requested: false,
            data_prompt: false,
            data_language: 0,
            data_step: DataStep::Choice,
            uninstall_prompt: false,
            lightbox: None,
        };
        self.audio.play(crate::audio::Sfx::IntoDetail);
    }
    pub fn clear_one_shot_ui_state(&mut self) {
        match &mut self.state {
            AppState::Catalog(catalog) => {
                catalog.scroll_to_selected = false;
                catalog.scroll_reset = false;
                catalog.scroll_category_into_view = false;
            }
            AppState::Detail { scroll_delta, .. } => *scroll_delta = 0.0,
            _ => {}
        }
    }
    fn back_to_catalog(&mut self) {
        if self.navigation_locked() {
            return;
        }
        if let AppState::Detail { lightbox: Some(_), .. } = &self.state {
            if let AppState::Detail { lightbox, .. } = &mut self.state {
                *lightbox = None;
            }
            return;
        }
        if let AppState::Catalog(catalog) = &mut self.state {
            if catalog.tab == StoreTab::Categories
                && (!catalog.search_query.is_empty() || catalog.search_requested)
            {
                catalog.search_query.clear();
                catalog.search_requested = false;
                catalog.refresh_filter();
                return;
            }
            if !catalog.search_query.is_empty() || catalog.search_requested {
                catalog.search_query.clear();
                catalog.search_requested = false;
                catalog.refresh_filter();
                return;
            }
        }
        if let AppState::Detail { app, previous, .. } = &mut self.state {
            if let Some(original) = previous.apps.iter_mut().find(|other| other.id == app.id) {
                original.user_liked = app.user_liked;
                original.user_rating = app.user_rating;
                original.likes_count = app.likes_count;
                original.ratings_count = app.ratings_count;
                original.comments_count = app.comments_count;
                original.rating = app.rating;
            }
            let mut previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
            previous.search_requested = false;
            match previous.tab {
                StoreTab::Library => previous.apply_install_filter(&self.installed, false),
                StoreTab::Updates => previous.apply_install_filter(&self.installed, true),
                _ => previous.refresh_filter_preserving_selection(),
            }
            self.state = AppState::Catalog(previous);
            self.needs_installed_rescan = true;
            self.audio.play(crate::audio::Sfx::OutOfDetail);
        }
    }
}
#[cfg(test)]
mod sort_tests {
    use super::*;
    use crate::data::Category;

    pub(super) fn entry(id: &str, source_catalog: &str, size_bytes: u64, downloads: u64, rating: f32, updated_at: &str) -> AppEntry {
        let mut e = AppEntry {
            id: id.to_owned(),
            titleid: String::new(),
            titleid_lower: String::new(),
            content_id: None,
            name: id.to_owned(),
            original_name: None,
            name_lower: String::new(),
            author: "unknown".to_owned(),
            author_lower: String::new(),
            description: String::new(),
            long_description: String::new(),
            requirements: String::new(),
            changelog: String::new(),
            release_page: None,
            category: Category::PsVitaGame,
            genres: Vec::new(),
            platform: crate::data::Platform::Vita,
            kind: String::new(),
            icon_url: None,
            cover_url: None,
            background_url: None,
            screenshot_urls: Vec::new(),
            download_url: "http://example.com".to_owned(),
            source: None,
            version: "1.0".to_owned(),
            region: None,
            zrif: None,
            source_catalog: source_catalog.to_owned(),
            source_labels: Vec::new(),
            hash: String::new(),
            hash2: String::new(),
            data_url: None,
            data_extract_path: None,
            data_size_bytes: 0,
            game_data_languages: Vec::new(),
            plugin_install_path: None,
            plugin_config_section: None,
            plugin_config_line: None,
            size_bytes,
            downloads,
            rating,
            updated_at: updated_at.to_owned(),
            ratings_count: 0,
            likes_count: 0,
            comments_count: 0,
            user_liked: false,
            user_rating: None,
            overview: Vec::new(),
        };
        e.rebuild_derived();
        e
    }

    fn mixed_catalog() -> CatalogState {
        let apps = vec![
            entry("vitadb-small", "vitadb", 1_000_000, 500, 4.0, "2024-01-01"),
            entry("vitadb-big", "vitadb", 900_000_000, 5000, 4.5, "2025-06-15"),
            entry("pkgj-unknown", "nps", 0, 0, 0.0, ""),
            entry("pkgj-known", "nps", 50_000_000, 0, 0.0, ""),
        ];
        let mut catalog = CatalogState::new(apps);
        catalog.sort_order = SortOrder::Downloads;
        catalog.sort_direction = SortDirection::Desc;
        catalog.resort();
        catalog
    }

    fn ordered_ids(catalog: &CatalogState) -> Vec<String> {
        catalog.filtered_indices.iter().map(|&i| catalog.apps[i].id.clone()).collect()
    }

    #[test]
    fn size_ascending_puts_zero_first_as_true_reversal_of_descending() {
        let mut catalog = mixed_catalog();
        catalog.set_sort(SortOrder::Size);
        catalog.set_sort(SortOrder::Size); // second press flips to Asc
        assert_eq!(catalog.sort_direction, SortDirection::Asc);
        let ids = ordered_ids(&catalog);
        assert_eq!(ids, vec!["pkgj-unknown", "vitadb-small", "pkgj-known", "vitadb-big"]);
    }

    #[test]
    fn size_descending_puts_largest_first_and_unknowns_last() {
        let mut catalog = mixed_catalog();
        catalog.set_sort(SortOrder::Size);
        assert_eq!(catalog.sort_direction, SortDirection::Desc);
        let ids = ordered_ids(&catalog);
        assert_eq!(ids, vec!["vitadb-big", "pkgj-known", "vitadb-small", "pkgj-unknown"]);
    }

    #[test]
    fn downloads_ascending_puts_zero_first_as_true_reversal_of_descending() {
        let mut catalog = mixed_catalog();
        catalog.set_sort(SortOrder::Downloads); // mode already active -> flips to Asc
        assert_eq!(catalog.sort_direction, SortDirection::Asc);
        let ids = ordered_ids(&catalog);
        assert_eq!(ids, vec!["pkgj-known", "pkgj-unknown", "vitadb-small", "vitadb-big"]);
    }

    #[test]
    fn recent_orders_by_parsed_date_with_empty_last() {
        let mut catalog = mixed_catalog();
        catalog.set_sort(SortOrder::Recent);
        let ids = ordered_ids(&catalog);
        assert_eq!(&ids[0], "vitadb-big");
        assert_eq!(&ids[1], "vitadb-small");
        assert!(ids[2] == "pkgj-known" || ids[2] == "pkgj-unknown");
    }

    #[test]
    fn set_sort_same_mode_flips_direction_different_mode_resets_default() {
        let mut catalog = mixed_catalog();
        assert_eq!(catalog.sort_order, SortOrder::Downloads);
        assert_eq!(catalog.sort_direction, SortDirection::Desc);
        catalog.set_sort(SortOrder::Downloads);
        assert_eq!(catalog.sort_direction, SortDirection::Asc);
        catalog.set_sort(SortOrder::Name);
        assert_eq!(catalog.sort_order, SortOrder::Name);
        assert_eq!(catalog.sort_direction, SortDirection::Asc);
    }

    #[test]
    fn sort_change_resets_selection_to_first_item() {
        let mut catalog = mixed_catalog();
        let pos = catalog.filtered_indices.iter().position(|&i| catalog.apps[i].id == "vitadb-big").unwrap();
        catalog.selected = pos;
        catalog.selection_active = true;
        catalog.set_sort(SortOrder::Name);
        assert_eq!(catalog.selected, 0);
        assert!(catalog.selection_active);
    }

    fn entry_titled(id: &str, titleid: &str, source_catalog: &str, downloads: u64) -> AppEntry {
        let mut e = entry(id, source_catalog, 1_000_000, downloads, 4.0, "2024-06-01");
        e.titleid = titleid.to_owned();
        e.name = format!("App {titleid}");
        e.rebuild_derived();
        e
    }

    fn twin_catalog() -> CatalogState {
        let mut nps = entry_titled("nps-same-tid", "RETROARCH", "vitaforge", 0);
        nps.platform = crate::data::Platform::NpsVita;
        nps.rebuild_derived();
        let apps = vec![
            entry_titled("official-ra", "RETROARCH", "vitaforge", 100),
            entry_titled("dbtoo-ra", "RETROARCH", "vitaforge", 9_999),
            entry_titled("dbtoo-only", "ONLYTOO", "vitaforge", 50),
            nps,
        ];
        let mut catalog = CatalogState::new(apps);
        catalog.sort_order = SortOrder::Downloads;
        catalog.sort_direction = SortDirection::Desc;
        catalog.resort();
        catalog
    }

    #[test]
    fn all_catalogs_collapses_homebrew_twins() {
        let catalog = twin_catalog();
        let ids = ordered_ids(&catalog);
        // Same titleid: higher downloads wins; commercial NPS twin stays separate.
        assert!(ids.contains(&"dbtoo-ra".to_owned()));
        assert!(!ids.contains(&"official-ra".to_owned()));
        assert!(ids.contains(&"dbtoo-only".to_owned()));
        assert!(ids.contains(&"nps-same-tid".to_owned()));
    }
    #[test]
    fn the_all_catalogs_count_matches_the_collapsed_grid() {
        let mut catalog = twin_catalog();
        catalog.recompute_dropdown_counts();
        assert_eq!(catalog.total_unique_count, ordered_ids(&catalog).len());
        assert_eq!(catalog.source_scoped_count, catalog.total_unique_count);
    }

    #[test]
    fn collapse_does_not_merge_nps_with_homebrew() {
        let mut nps = entry_titled("nps", "PCSF00092", "vitaforge", 0);
        nps.platform = crate::data::Platform::NpsVita;
        nps.rebuild_derived();
        let apps = vec![
            entry_titled("hb", "PCSF00092", "vitaforge", 10),
            nps,
        ];
        let mut catalog = CatalogState::new(apps);
        catalog.sort_order = SortOrder::Name;
        catalog.sort_direction = SortDirection::Asc;
        catalog.resort();
        let ids = ordered_ids(&catalog);
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"hb".to_owned()));
        assert!(ids.contains(&"nps".to_owned()));
    }

}

#[cfg(test)]
mod precache_tests {
    use super::sort_tests::entry;
    use super::*;

    fn art(platform: crate::data::Platform, icon: Option<&str>, cover: Option<&str>) -> AppEntry {
        let mut e = entry("x", "nps", 0, 0, 0.0, "");
        e.platform = platform;
        e.icon_url = icon.map(str::to_owned);
        e.cover_url = cover.map(str::to_owned);
        e
    }

    #[test]
    fn commercial_entries_precache_their_cover_not_a_missing_icon() {
        let nps = art(crate::data::Platform::NpsVita, None, Some("https://x/cover.jpg"));
        assert_eq!(tile_art_url(&nps), Some("https://x/cover.jpg"));

        let urls = precache_art_urls(&[nps]);
        assert_eq!(urls, vec!["https://x/cover.jpg".to_owned()]);
    }

    #[test]
    fn homebrew_still_prefers_its_curated_icon() {
        let hb = art(
            crate::data::Platform::Vita,
            Some("https://x/icon.png"),
            Some("https://x/cover.jpg"),
        );
        assert_eq!(tile_art_url(&hb), Some("https://x/icon.png"));
    }

    #[test]
    fn precache_is_capped_so_one_launch_cannot_burn_the_request_window() {
        let apps: Vec<AppEntry> = (0..MAX_PRECACHE_PER_LAUNCH * 3)
            .map(|_| art(crate::data::Platform::NpsVita, None, Some("https://x/c.jpg")))
            .collect();
        assert_eq!(precache_art_urls(&apps).len(), MAX_PRECACHE_PER_LAUNCH);
    }
}

/// Preselects the extra language matching the app's UI language, else the default (0).
fn preferred_data_language(app: &AppEntry, ui: crate::app::i18n::Language) -> usize {
    use crate::app::i18n::Language;
    let code = match ui {
        Language::English => "en",
        Language::Spanish => "es",
        Language::French => "fr",
        Language::Italian => "it",
        Language::Portuguese => "pt",
    };
    app.game_data_languages
        .iter()
        .position(|l| l.language.split('-').next() == Some(code))
        .map_or(0, |i| i + 1)
}

#[cfg(test)]
mod data_language_tests {
    use super::preferred_data_language;
    use crate::app::i18n::Language;
    use crate::data::GameDataLanguage;

    #[test]
    fn preselects_ui_language_or_default() {
        let mut app = super::sort_tests::entry("1", "vitadb", 0, 0, 0.0, "");
        let lang = |code: &str| GameDataLanguage {
            language: code.to_owned(),
            url: format!("https://h/{code}.zip"),
            extract_path: None,
            size_bytes: 0,
        };
        assert_eq!(preferred_data_language(&app, Language::Spanish), 0);
        app.game_data_languages = vec![lang("ja"), lang("es")];
        assert_eq!(preferred_data_language(&app, Language::Spanish), 2);
        assert_eq!(preferred_data_language(&app, Language::French), 0);
    }
}

/// Closes an open choice list; an unconfirmed theme preview snaps back.
fn close_settings_sub(nav: &mut ui::settings::SettingsNav) {
    if nav.sub == Some(ui::settings::SubPage::Theme) {
        ui::theme::set_theme(nav.theme_before);
    }
    nav.sub = None;
}
