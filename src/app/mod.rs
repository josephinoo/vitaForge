pub mod i18n;
pub mod icons;
pub mod sysinfo;
pub mod text;
pub mod ui;
use crate::data::{self, AppEntry, Category, SortDirection, SortOrder, SourceCatalog};
use crate::input::{AppCommand, DiscoverRail, InputCommand, StoreTab};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoverFocus {
    Featured,
    Top(usize),
    New(usize),
    BrowseAll,
}

pub struct CatalogState {
    pub apps: Vec<AppEntry>,
    sorted_indices: Vec<usize>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub search_requested: bool,
    pub category_filter: Option<Category>,
    pub genre_filter: Option<String>,
    pub source_filter: Option<SourceCatalog>,
    pub sort_order: SortOrder,
    pub sort_direction: SortDirection,
    pub selected: usize,
    pub selection_active: bool,
    pub scroll_to_selected: bool,

    pub scroll_reset: bool,
    pub is_commercial_view: bool,
    pub source_counts: Vec<(SourceCatalog, usize)>,
    pub category_counts: Vec<(Category, usize)>,
    pub genre_counts: Vec<(String, usize)>,
    pub source_scoped_count: usize,
    pub total_unique_count: usize,
    pub featured_index: Option<usize>,
    pub tab: StoreTab,
    pub discover_home: bool,
    pub discover_focus: DiscoverFocus,
    pub scroll_category_into_view: bool,
    pub top_rail: Vec<usize>,
    pub recent_rail: Vec<usize>,
    top_rail_ids: Vec<String>,
    recent_rail_ids: Vec<String>,
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
            genre_filter: None,
            source_filter: None,
            sort_order: settings.sort_order,
            sort_direction: settings.sort_direction,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            scroll_reset: false,
            is_commercial_view: false,
            source_counts: Vec::new(),
            category_counts: Vec::new(),
            genre_counts: Vec::new(),
            source_scoped_count: 0,
            total_unique_count: 0,
            featured_index: None,
            tab: StoreTab::Discover,
            discover_home: true,
            discover_focus: DiscoverFocus::Featured,
            scroll_category_into_view: false,
            top_rail: Vec::new(),
            recent_rail: Vec::new(),
            top_rail_ids: Vec::new(),
            recent_rail_ids: Vec::new(),
        };
        state.resort();
        state.rebuild_rails_from_ids_or_fallback();
        state.clamp_discover_focus();
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
            genre_filter: None,
            source_filter: None,
            sort_order: SortOrder::default(),
            sort_direction: SortDirection::default(),
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            scroll_reset: false,
            is_commercial_view: false,
            source_counts: Vec::new(),
            category_counts: Vec::new(),
            genre_counts: Vec::new(),
            source_scoped_count: 0,
            total_unique_count: 0,
            featured_index: None,
            tab: StoreTab::Discover,
            discover_home: true,
            discover_focus: DiscoverFocus::Featured,
            scroll_category_into_view: false,
            top_rail: Vec::new(),
            recent_rail: Vec::new(),
            top_rail_ids: Vec::new(),
            recent_rail_ids: Vec::new(),
        }
    }
    fn replace_apps(&mut self, apps: Vec<AppEntry>) {
        self.apps = apps;
        self.sorted_indices = self.sort_order_indices();
        self.recompute_dropdown_counts();
        self.rebuild_rails_from_ids_or_fallback();
        self.refresh_filter_preserving_selection();
    }
    fn set_rail_ids(&mut self, top: Vec<String>, recent: Vec<String>) {
        self.top_rail_ids = top;
        self.recent_rail_ids = recent;
        self.rebuild_rails_from_ids_or_fallback();
    }
    fn rebuild_rails_from_ids_or_fallback(&mut self) {
        let mut top = dedupe_rail_indices(resolve_rail_ids(
            &self.apps,
            &self.top_rail_ids,
            self.source_filter,
        ));
        let mut recent = dedupe_rail_indices(resolve_rail_ids(
            &self.apps,
            &self.recent_rail_ids,
            self.source_filter,
        ));
        if self.source_filter.is_none() {
            top = collapse_homebrew_duplicates(&self.apps, top);
            recent = collapse_homebrew_duplicates(&self.apps, recent);
        }
        self.top_rail = top;
        self.recent_rail = recent;
        pad_rail_with_fallback(&mut self.top_rail, &self.apps, self.source_filter, RailFallback::Downloads);
        pad_rail_with_fallback(&mut self.recent_rail, &self.apps, self.source_filter, RailFallback::Recent);
        self.featured_index = self.top_rail.first().copied();
        self.strip_featured_from_rails();
        pad_rail_with_fallback(&mut self.top_rail, &self.apps, self.source_filter, RailFallback::Downloads);
        pad_rail_with_fallback(&mut self.recent_rail, &self.apps, self.source_filter, RailFallback::Recent);
        self.strip_featured_from_rails();
        self.clamp_discover_focus();
    }

    fn strip_featured_from_rails(&mut self) {
        let Some(feat) = self.featured_index else {
            return;
        };
        let feat_titleid = self
            .apps
            .get(feat)
            .map(|a| a.titleid_lower.as_str())
            .unwrap_or("");
        let is_featured = |apps: &[AppEntry], idx: usize| {
            if idx == feat {
                return true;
            }
            if feat_titleid.is_empty() {
                return false;
            }
            apps.get(idx)
                .is_some_and(|a| !a.titleid_lower.is_empty() && a.titleid_lower == feat_titleid)
        };
        self.top_rail.retain(|&idx| !is_featured(&self.apps, idx));
        self.recent_rail.retain(|&idx| !is_featured(&self.apps, idx));
    }
    fn apply_store_tab(&mut self, tab: StoreTab, installed: &crate::install::installed::InstalledIndex) {
        self.tab = tab;
        self.scroll_reset = true;
        match tab {
            StoreTab::Discover => {
                self.discover_home = true;
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.refresh_filter();
                self.clamp_discover_focus();
            }
            StoreTab::Library => {
                self.discover_home = false;
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.apply_install_filter(installed, false);
            }
            StoreTab::Updates => {
                self.discover_home = false;
                self.search_query.clear();
                self.search_requested = false;
                self.category_filter = None;
                self.apply_install_filter(installed, true);
            }
            StoreTab::Search => {
                self.discover_home = false;
                self.refresh_filter();
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
        if self.source_filter.is_none() {
            self.filtered_indices =
                collapse_homebrew_duplicates(&self.apps, std::mem::take(&mut self.filtered_indices));
        }
        self.is_commercial_view = false;
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = 0;
            self.selection_active = true;
        }
    }
    fn see_all_rail(&mut self, rail: DiscoverRail) {
        self.discover_home = false;
        self.tab = StoreTab::Discover;
        self.search_query.clear();
        self.category_filter = None;
        match rail {
            DiscoverRail::Top => {
                self.sort_order = SortOrder::Downloads;
                self.sort_direction = SortDirection::Desc;
            }
            DiscoverRail::New => {
                self.sort_order = SortOrder::Recent;
                self.sort_direction = SortDirection::Desc;
            }
        }
        self.resort();
    }
    fn source_api_id(&self) -> Option<&'static str> {
        match self.source_filter {
            Some(SourceCatalog::VitaDb) => Some("vitadb"),
            Some(SourceCatalog::VitaDbToo) => Some("vitadbtoo"),
            Some(SourceCatalog::Nps) => Some("nps"),
            None => None,
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
        let genre_filter = self.genre_filter.as_deref();
        let source_filter = self.source_filter;
        self.filtered_indices = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|&index| {
                let app = &apps[index];
                let matches_cat = category_filter.is_none_or(|c| c as u8 == app.category as u8);
                let matches_genre = genre_filter.is_none_or(|genre| {
                    app.genres.iter().any(|app_genre| app_genre.eq_ignore_ascii_case(genre))
                });
                let matches_source = source_filter.is_none_or(|s| s.matches(&app.source_catalog));
                matches_cat
                    && matches_genre
                    && matches_source
                    && (query.is_empty()
                        || app.name_lower.contains(&query)
                        || app.author_lower.contains(&query)
                        || app.titleid_lower.contains(&query)
                        || app.genres.iter().any(|genre| genre.to_lowercase().contains(&query)))
            })
            .collect();
        if source_filter.is_none() {
            self.filtered_indices =
                collapse_homebrew_duplicates(&self.apps, std::mem::take(&mut self.filtered_indices));
        }
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = 0;
            self.selection_active = true;
            self.scroll_reset = true;
        }
        self.is_commercial_view = self.source_filter == Some(SourceCatalog::Nps);
        if self.tab == StoreTab::Discover && self.discover_home {
        } else {
            self.featured_index = self
                .filtered_indices
                .iter()
                .copied()
                .max_by(|&a, &b| {
                    apps[a]
                        .downloads
                        .cmp(&apps[b].downloads)
                        .then_with(|| apps[a].rating.total_cmp(&apps[b].rating))
                });
        }
    }
    fn recompute_dropdown_counts(&mut self) {
        let source_filter = self.source_filter;
        let unique_all = collapse_homebrew_duplicates(&self.apps, (0..self.apps.len()).collect());
        let visible: Vec<usize> = match source_filter {
            None => unique_all.clone(),
            Some(source) => (0..self.apps.len())
                .filter(|&index| source.matches(&self.apps[index].source_catalog))
                .collect(),
        };
        let apps = &self.apps;
        self.source_counts = SourceCatalog::ALL
            .into_iter()
            .map(|source| (source, apps.iter().filter(|app| source.matches(&app.source_catalog)).count()))
            .filter(|&(_, count)| count > 0)
            .collect();
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
        if source_filter == Some(SourceCatalog::Nps) {
            for &index in &visible {
                for genre in &apps[index].genres {
                    if !genre.eq_ignore_ascii_case("other") {
                        *genres.entry(genre.clone()).or_default() += 1;
                    }
                }
            }
        }
        self.genre_counts = genres.into_iter().collect();
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

    fn shows_category_pills(&self) -> bool {
        matches!(self.tab, StoreTab::Discover | StoreTab::Search)
            && !(self.tab == StoreTab::Discover && self.discover_home)
    }

    fn shows_discover_home(&self) -> bool {
        self.tab == StoreTab::Discover && self.discover_home && self.search_query.trim().is_empty()
    }

    fn featured_available(&self) -> bool {
        !self.is_commercial_view && self.featured_index.is_some()
    }

    fn clamp_discover_focus(&mut self) {
        let featured = self.featured_available();
        let top_len = self.top_rail.len().min(16);
        let new_len = self.recent_rail.len().min(16);
        self.discover_focus = match self.discover_focus {
            DiscoverFocus::Featured if featured => DiscoverFocus::Featured,
            DiscoverFocus::Top(i) if top_len > 0 => DiscoverFocus::Top(i.min(top_len - 1)),
            DiscoverFocus::New(i) if new_len > 0 => DiscoverFocus::New(i.min(new_len - 1)),
            DiscoverFocus::BrowseAll => DiscoverFocus::BrowseAll,
            _ if featured => DiscoverFocus::Featured,
            _ if top_len > 0 => DiscoverFocus::Top(0),
            _ if new_len > 0 => DiscoverFocus::New(0),
            _ => DiscoverFocus::Featured,
        };
    }

    fn discover_focus_app_id(&self) -> Option<&str> {
        match self.discover_focus {
            DiscoverFocus::Featured => self
                .featured_index
                .and_then(|idx| self.apps.get(idx))
                .map(|app| app.id.as_str()),
            DiscoverFocus::Top(i) => self
                .top_rail
                .get(i)
                .and_then(|&idx| self.apps.get(idx))
                .map(|app| app.id.as_str()),
            DiscoverFocus::New(i) => self
                .recent_rail
                .get(i)
                .and_then(|&idx| self.apps.get(idx))
                .map(|app| app.id.as_str()),
            DiscoverFocus::BrowseAll => None,
        }
    }

    fn move_discover_focus(&mut self, direction: InputCommand) -> bool {
        self.clamp_discover_focus();
        let featured = self.featured_available();
        let top_len = self.top_rail.len().min(16);
        let new_len = self.recent_rail.len().min(16);
        let before = self.discover_focus;
        match direction {
            InputCommand::MoveLeft | InputCommand::MoveRight => {
                let delta: isize = if direction == InputCommand::MoveLeft { -1 } else { 1 };
                match self.discover_focus {
                    DiscoverFocus::Top(i) if top_len > 0 => {
                        let next = (i as isize + delta).clamp(0, top_len as isize - 1) as usize;
                        self.discover_focus = DiscoverFocus::Top(next);
                    }
                    DiscoverFocus::New(i) if new_len > 0 => {
                        let next = (i as isize + delta).clamp(0, new_len as isize - 1) as usize;
                        self.discover_focus = DiscoverFocus::New(next);
                    }
                    _ => {}
                }
            }
            InputCommand::MoveUp => {
                self.discover_focus = match self.discover_focus {
                    DiscoverFocus::BrowseAll if new_len > 0 => DiscoverFocus::New(0),
                    DiscoverFocus::BrowseAll if top_len > 0 => DiscoverFocus::Top(0),
                    DiscoverFocus::BrowseAll if featured => DiscoverFocus::Featured,
                    DiscoverFocus::New(i) if top_len > 0 => DiscoverFocus::Top(i.min(top_len - 1)),
                    DiscoverFocus::New(_) if featured => DiscoverFocus::Featured,
                    DiscoverFocus::Top(_) if featured => DiscoverFocus::Featured,
                    other => other,
                };
            }
            InputCommand::MoveDown => {
                self.discover_focus = match self.discover_focus {
                    DiscoverFocus::Featured if top_len > 0 => DiscoverFocus::Top(0),
                    DiscoverFocus::Featured if new_len > 0 => DiscoverFocus::New(0),
                    DiscoverFocus::Top(i) if new_len > 0 => DiscoverFocus::New(i.min(new_len - 1)),
                    DiscoverFocus::BrowseAll => DiscoverFocus::BrowseAll,
                    _ => DiscoverFocus::BrowseAll,
                };
            }
            _ => {}
        }
        let changed = self.discover_focus != before;
        if changed {
            self.scroll_to_selected = true;
        }
        changed
    }
}

enum RailFallback {
    Downloads,
    Recent,
}

fn resolve_rail_ids(
    apps: &[AppEntry],
    ids: &[String],
    source_filter: Option<SourceCatalog>,
) -> Vec<usize> {
    let mut out = Vec::new();
    for id in ids {
        if let Some((idx, app)) = apps.iter().enumerate().find(|(_, app)| app.id == *id) {
            if source_filter.is_none_or(|s| s.matches(&app.source_catalog)) {
                out.push(idx);
            }
        }
    }
    out
}

fn dedupe_rail_indices(indices: Vec<usize>) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    indices.into_iter().filter(|&idx| seen.insert(idx)).collect()
}

fn homebrew_source_rank(source: SourceCatalog) -> u8 {
    match source {
        SourceCatalog::VitaDb => 0,
        SourceCatalog::VitaDbToo => 1,
        SourceCatalog::Nps => 255,
    }
}

fn is_collapsible_homebrew(app: &AppEntry) -> bool {
    if app.titleid_lower.is_empty() {
        return false;
    }
    matches!(
        SourceCatalog::from_api(&app.source_catalog),
        Some(SourceCatalog::VitaDb | SourceCatalog::VitaDbToo)
    )
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
    let rank_a = SourceCatalog::from_api(&a.source_catalog)
        .map(homebrew_source_rank)
        .unwrap_or(100);
    let rank_b = SourceCatalog::from_api(&b.source_catalog)
        .map(homebrew_source_rank)
        .unwrap_or(100);
    if rank_a != rank_b {
        return rank_a < rank_b;
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

const RAIL_MIN_LEN: usize = 12;

fn pad_rail_with_fallback(
    rail: &mut Vec<usize>,
    apps: &[AppEntry],
    source_filter: Option<SourceCatalog>,
    kind: RailFallback,
) {
    if rail.len() >= RAIL_MIN_LEN {
        return;
    }
    let existing: std::collections::HashSet<usize> = rail.iter().copied().collect();
    let fallback = fallback_rail(apps, source_filter, kind, RAIL_MIN_LEN);
    for idx in fallback {
        if rail.len() >= RAIL_MIN_LEN {
            break;
        }
        if existing.contains(&idx) {
            continue;
        }
        rail.push(idx);
    }
}

fn fallback_rail(
    apps: &[AppEntry],
    source_filter: Option<SourceCatalog>,
    kind: RailFallback,
    limit: usize,
) -> Vec<usize> {
    let mut indices: Vec<usize> = apps
        .iter()
        .enumerate()
        .filter(|(_, app)| source_filter.is_none_or(|s| s.matches(&app.source_catalog)))
        .map(|(idx, _)| idx)
        .collect();
    match kind {
        RailFallback::Downloads => indices.sort_by(|&a, &b| {
            apps[b]
                .downloads
                .cmp(&apps[a].downloads)
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
        }),
        RailFallback::Recent => indices.sort_by(|&a, &b| {
            apps[b]
                .sort_epoch()
                .cmp(&apps[a].sort_epoch())
                .then_with(|| apps[a].name_lower.cmp(&apps[b].name_lower))
        }),
    }
    if source_filter.is_none() {
        indices = collapse_homebrew_duplicates(apps, indices);
    }
    indices.truncate(limit);
    indices
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
    },
    Settings {
        previous: Box<CatalogState>,
        selected: usize,
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
    rails_rx: Option<oneshot::Receiver<(Vec<String>, Vec<String>)>>,
    cache_stats_rx: Option<oneshot::Receiver<data::cache_manager::CacheStats>>,
    audio: crate::audio::AudioEngine,
    pub cache_stats: data::cache_manager::CacheStats,
    pub cache_notice: Option<String>,
    icons_need_clear: bool,
    install_notifications: bool,
}
const LOAD_WATCHDOG: std::time::Duration = std::time::Duration::from_secs(25);
pub struct InstallJob {
    pub app_id: String,
    pub app_id_title: String,
    pub title: String,
    pub progress: crate::install::Progress,
    rx: watch::Receiver<crate::install::Progress>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    last_notification_progress: Option<(u8, u8)>,
}

fn progress_notification(progress: &crate::install::Progress) -> Option<(u8, u8)> {
    match progress {
        crate::install::Progress::DownloadingData { received, total: Some(total), .. } if *total > 0 => {
            Some((1, ((*received * 100 / *total).min(100)) as u8))
        }
        crate::install::Progress::Downloading { received, total: Some(total), .. } if *total > 0 => {
            Some((2, ((*received * 100 / *total).min(100)) as u8))
        }
        crate::install::Progress::Extracting { done, total } if *total > 0 => {
            Some((3, ((*done as u64 * 100 / *total as u64).min(100)) as u8))
        }
        _ => None,
    }
}

#[cfg(test)]
mod install_notification_tests {
    use super::progress_notification;
    use crate::install::Progress;

    #[test]
    fn progress_notifications_only_use_known_totals() {
        assert_eq!(
            progress_notification(&Progress::Downloading {
                received: 50,
                total: Some(200),
                elapsed_secs: 1,
            }),
            Some((2, 25))
        );
        assert_eq!(
            progress_notification(&Progress::Downloading {
                received: 50,
                total: None,
                elapsed_secs: 1,
            }),
            None
        );
    }
}
impl App {
    pub fn new() -> Result<Self> {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match data::source::fetch_live().await {
                Ok(apps) if !apps.is_empty() => CatalogSource::Live(apps),
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
            rails_rx: None,
            cache_stats_rx: None,
            audio: crate::audio::AudioEngine::new(),
            cache_stats: data::cache_manager::CacheStats::default(),
            cache_notice: None,
            icons_need_clear: false,
            install_notifications: settings.install_notifications,
        })
    }
    fn spawn_catalog_fetch(&mut self) {
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = match data::source::fetch_live().await {
                Ok(apps) if !apps.is_empty() => CatalogSource::Live(apps),
                Ok(_) => CatalogSource::Failed,
                Err(err) => {
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
    pub fn install_busy(&self) -> bool {
        self.install.as_ref().is_some_and(|job| !job.progress.is_finished())
    }
    pub fn tick(&mut self, ctx: &egui::Context) -> Result<()> {
        if self.icons_need_clear {
            self.icons.clear_resident(ctx);
            self.icons_need_clear = false;
        }
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
            if self.install_notifications && previous != job.progress {
                if let Some((stage, percent)) = progress_notification(&job.progress) {
                    let milestone = percent / 25 * 25;
                    if job.last_notification_progress != Some((stage, milestone)) {
                        crate::install::notify::install_progress(&job.title, milestone);
                        job.last_notification_progress = Some((stage, milestone));
                    }
                }
            }
            if previous != job.progress && job.progress == crate::install::Progress::Done {
                self.installed.mark_installed(&job.app_id_title);
                if self.install_notifications {
                    crate::install::notify::install_finished(&job.title);
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
                    crate::install::notify::install_failed(&job.title, reason);
                }
            }
        }
        if let Some(rx) = &mut self.self_update_rx
            && let Ok(info) = rx.try_recv()
        {
            self.self_update_rx = None;
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
        if let Some(rx) = &mut self.rails_rx
            && let Ok((top, recent)) = rx.try_recv()
        {
            self.rails_rx = None;
            if let AppState::Catalog(catalog) = &mut self.state {
                catalog.set_rail_ids(top, recent);
                ctx.request_repaint();
            } else if let AppState::Detail { previous, .. } | AppState::Settings { previous, .. } =
                &mut self.state
            {
                previous.set_rail_ids(top, recent);
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
                            let source = catalog.source_api_id();
                            self.spawn_rails_fetch(source);
                            self.state = AppState::Catalog(catalog);
                        }
                        AppState::Catalog(catalog) => {
                            catalog.replace_apps(apps);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.icons.precache_background(precache_art_urls(&catalog.apps));
                            let source = catalog.source_api_id();
                            self.spawn_rails_fetch(source);
                        }
                        AppState::Detail { previous, .. } | AppState::Settings { previous, .. } => {
                            previous.replace_apps(apps);
                            self.installed.force_refresh(ctx, &previous.apps);
                            let source = previous.source_api_id();
                            self.spawn_rails_fetch(source);
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
                    self.load_rx = None;
                    let apps = data::api::load_cached_catalog_sync().unwrap_or_default();
                    eprintln!(
                        "catalog fetch never resolved after {}s; falling back to {} cached entries",
                        LOAD_WATCHDOG.as_secs(),
                        apps.len(),
                    );
                    crate::install::log_file(&format!(
                        "catalog fetch stalled past the {}s watchdog; showing {} cached entries",
                        LOAD_WATCHDOG.as_secs(),
                        apps.len(),
                    ));
                    let catalog = CatalogState::new(apps);
                    self.installed.force_refresh(ctx, &catalog.apps);
                    self.state = AppState::Catalog(catalog);
                    ctx.request_repaint();
                }
            }
            Err(oneshot::error::TryRecvError::Closed) => self.load_rx = None,
        }
        Ok(())
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
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.tab == StoreTab::Discover && !catalog.discover_home {
                        catalog.discover_home = true;
                        catalog.category_filter = None;
                        catalog.search_query.clear();
                        catalog.refresh_filter();
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
                        if catalog.tab != StoreTab::Search && !catalog.search_query.is_empty() {
                            catalog.tab = StoreTab::Search;
                            catalog.discover_home = false;
                        }
                        catalog.refresh_filter();
                    }
                }
            }
            AppCommand::RequestSearch => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.tab = StoreTab::Search;
                    catalog.discover_home = false;
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
                    catalog.discover_home = false;
                    if catalog.tab == StoreTab::Discover || catalog.tab == StoreTab::Search {
                        catalog.refresh_filter();
                    }
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetGenreFilter(genre) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.genre_filter = genre;
                    catalog.discover_home = false;
                    if catalog.tab == StoreTab::Discover || catalog.tab == StoreTab::Search {
                        catalog.refresh_filter();
                    }
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetSourceFilter(source) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.source_filter = source;
                    catalog.category_filter = None;
                    catalog.genre_filter = None;
                    catalog.recompute_dropdown_counts();
                    catalog.rebuild_rails_from_ids_or_fallback();
                    match catalog.tab {
                        StoreTab::Library => catalog.apply_install_filter(&self.installed, false),
                        StoreTab::Updates => catalog.apply_install_filter(&self.installed, true),
                        _ => catalog.refresh_filter(),
                    }
                    let api_source = catalog.source_api_id();
                    self.spawn_rails_fetch(api_source);
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetSortOrder(sort) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.set_sort(sort);
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::FlipSortDirection => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.flip_sort_direction();
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::Input(InputCommand::Confirm) => {
                if matches!(self.state, AppState::Detail { data_prompt: true, .. }) {
                    return self.handle_command(AppCommand::InstallCurrent);
                }
                if let AppState::Catalog(catalog) = &self.state
                    && catalog.shows_discover_home()
                {
                    if catalog.discover_focus == DiscoverFocus::BrowseAll {
                        return self.handle_command(AppCommand::SeeAllRail(DiscoverRail::Top));
                    }
                    if let Some(id) = catalog.discover_focus_app_id().map(str::to_owned) {
                        return self.handle_command(AppCommand::SelectAppById(id));
                    }
                    return Ok(());
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
                } else if let AppState::Settings { selected, .. } = &self.state {
                    return match *selected {
                        index if index < Language::ALL.len() => self.handle_command(AppCommand::SetLanguage(Language::ALL[index])),
                        5 => self.handle_command(AppCommand::ClearIconCache),
                        6 => self.handle_command(AppCommand::ClearCatalogCache),
                        7 => self.handle_command(AppCommand::PurgeAllCache),
                        8 => self.handle_command(AppCommand::ToggleInstallNotifications),
                        _ => Ok(()),
                    };
                }
            }
            AppCommand::Input(InputCommand::CategoryPrev) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.shows_category_pills() {
                        let next_cat = match catalog.category_filter {
                            None => Some(Category::ALL[Category::ALL.len() - 1]),
                            Some(c) => {
                                let idx = Category::ALL.iter().position(|&x| x == c).unwrap_or(0);
                                if idx == 0 { None } else { Some(Category::ALL[idx - 1]) }
                            }
                        };
                        catalog.category_filter = next_cat;
                        catalog.scroll_category_into_view = true;
                        catalog.refresh_filter();
                        self.audio.play(crate::audio::Sfx::TabTransition);
                    } else {
                        let prev = match catalog.tab {
                            StoreTab::Discover => StoreTab::Search,
                            StoreTab::Library => StoreTab::Discover,
                            StoreTab::Updates => StoreTab::Library,
                            StoreTab::Search => StoreTab::Updates,
                        };
                        return self.handle_command(AppCommand::SetStoreTab(prev));
                    }
                }
            }
            AppCommand::Input(InputCommand::CategoryNext) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    if catalog.shows_category_pills() {
                        let next_cat = match catalog.category_filter {
                            None => Some(Category::ALL[0]),
                            Some(c) => {
                                let idx = Category::ALL.iter().position(|&x| x == c).unwrap_or(0);
                                if idx + 1 >= Category::ALL.len() { None } else { Some(Category::ALL[idx + 1]) }
                            }
                        };
                        catalog.category_filter = next_cat;
                        catalog.scroll_category_into_view = true;
                        catalog.refresh_filter();
                        self.audio.play(crate::audio::Sfx::TabTransition);
                    } else {
                        let next = match catalog.tab {
                            StoreTab::Discover => StoreTab::Library,
                            StoreTab::Library => StoreTab::Updates,
                            StoreTab::Updates => StoreTab::Search,
                            StoreTab::Search => StoreTab::Discover,
                        };
                        return self.handle_command(AppCommand::SetStoreTab(next));
                    }
                }
            }
            AppCommand::Input(direction) => {
                match &mut self.state {
                    AppState::Catalog(catalog) if catalog.shows_discover_home() => {
                        if catalog.move_discover_focus(direction) {
                            self.audio.play(crate::audio::Sfx::Navigate);
                        }
                    }
                    AppState::Catalog(catalog) => {
                        let columns = crate::app::ui::GRID_COLUMNS as isize;
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
                    AppState::Settings { selected, .. } => {
                        match direction {
                            InputCommand::MoveUp => *selected = selected.saturating_sub(1),
                            InputCommand::MoveDown => *selected = (*selected + 1).min(8),
                            _ => {}
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
                    if self.install_busy() {
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
                    if tab == StoreTab::Search {
                        previous.search_requested = true;
                    }
                    self.state = AppState::Catalog(previous);
                    self.needs_installed_rescan = true;
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SeeAllRail(rail) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.see_all_rail(rail);
                    self.audio.play(crate::audio::Sfx::Activation);
                }
            }
            AppCommand::BackToDiscoverHome => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.tab = StoreTab::Discover;
                    catalog.discover_home = true;
                    catalog.category_filter = None;
                    catalog.search_query.clear();
                    catalog.refresh_filter();
                    catalog.clamp_discover_focus();
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::MoreByAuthor(author) => {
                if self.install_busy() {
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
                    previous.tab = StoreTab::Search;
                    previous.discover_home = false;
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
                if let AppState::Detail { app, data_prompt, .. } = &mut self.state
                    && !busy
                    && app.data_url.is_some()
                    && !*data_prompt
                {
                    *data_prompt = true;
                    self.audio.play(crate::audio::Sfx::ShowModal);
                    return Ok(());
                }
                if let AppState::Detail { app, .. } = &self.state
                    && !busy
                {
                    let entry = app.clone();
                    let app_id = entry.id.clone();
                    let app_id_title = crate::install::installed::index_key(&entry);
                    let title = entry.name.clone();
                    let (rx, cancel) = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob {
                        app_id,
                        app_id_title,
                        title,
                        progress,
                        rx,
                        cancel,
                        last_notification_progress: None,
                    });
                    self.audio.play(crate::audio::Sfx::Launch);
                    if let AppState::Detail { data_prompt, .. } = &mut self.state {
                        *data_prompt = false;
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
                        title,
                        progress,
                        rx,
                        cancel,
                        last_notification_progress: None,
                    });
                    self.audio.play(crate::audio::Sfx::Launch);
                }
            }
            AppCommand::CancelInstall => {
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
                    other @ (AppState::Loading | AppState::Settings { .. }) => {
                        self.state = other;
                        return Ok(());
                    }
                };
                let selected = Language::ALL.iter().position(|&language| language == self.lang).unwrap_or(0);
                self.refresh_cache_stats();
                self.cache_notice = None;
                self.state = AppState::Settings { previous, selected };
                self.audio.play(crate::audio::Sfx::MenuFlyIn);
            }
            AppCommand::CloseSettings => {
                if let AppState::Settings { previous, .. } = &mut self.state {
                    let previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
                    self.state = AppState::Catalog(previous);
                    self.needs_installed_rescan = true;
                    self.audio.play(crate::audio::Sfx::MenuFlyOut);
                }
            }
            AppCommand::SetLanguage(lang) => {
                self.lang = lang;
                data::settings::set_language(lang);
                if let AppState::Settings { selected, .. } = &mut self.state {
                    *selected = Language::ALL.iter().position(|&language| language == lang).unwrap_or(0);
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
    fn spawn_rails_fetch(&mut self, source: Option<&'static str>) {
        let (tx, rx) = oneshot::channel();
        self.rails_rx = Some(rx);
        tokio::spawn(async move {
            let rails = data::api::fetch_discover_rails(source).await;
            let _ = tx.send(rails);
        });
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
        if self.install_busy() {
            return;
        }
        if let AppState::Detail { lightbox: Some(_), .. } = &self.state {
            if let AppState::Detail { lightbox, .. } = &mut self.state {
                *lightbox = None;
            }
            return;
        }
        if let AppState::Catalog(catalog) = &mut self.state {
            if catalog.tab == StoreTab::Search
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
            category: Category::Tool,
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
        let apps = vec![
            entry_titled("official-ra", "RETROARCH", "vitadb", 100),
            entry_titled("dbtoo-ra", "RETROARCH", "vitadbtoo", 9_999),
            entry_titled("dbtoo-only", "ONLYTOO", "vitadbtoo", 50),
            entry_titled("nps-same-tid", "RETROARCH", "nps", 0),
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
        assert!(ids.contains(&"official-ra".to_owned()));
        assert!(!ids.contains(&"dbtoo-ra".to_owned())); // the same title from the other catalog
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
    fn source_filter_vitadbtoo_keeps_dbtoo_twin() {
        let mut catalog = twin_catalog();
        catalog.source_filter = Some(SourceCatalog::VitaDbToo);
        catalog.refresh_filter();
        let ids = ordered_ids(&catalog);
        assert_eq!(ids, vec!["dbtoo-ra", "dbtoo-only"]);
    }

    #[test]
    fn collapse_does_not_merge_nps_with_homebrew() {
        let apps = vec![
            entry_titled("hb", "PCSF00092", "vitadb", 10),
            entry_titled("nps", "PCSF00092", "nps", 0),
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

    #[test]
    fn discover_rails_exclude_featured_app() {
        let apps = vec![
            entry_titled("top1", "AAA000001", "vitadb", 1000),
            entry_titled("top2", "AAA000002", "vitadb", 900),
            entry_titled("top3", "AAA000003", "vitadb", 800),
        ];
        let mut catalog = CatalogState::new(apps);
        catalog.sort_order = SortOrder::Downloads;
        catalog.sort_direction = SortDirection::Desc;
        catalog.rebuild_rails_from_ids_or_fallback();
        let feat = catalog.featured_index.expect("featured");
        assert_eq!(catalog.apps[feat].id, "top1");
        assert!(!catalog.top_rail.contains(&feat));
        assert!(!catalog.recent_rail.contains(&feat));
        assert!(catalog.top_rail.iter().any(|&i| catalog.apps[i].id == "top2"));
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

    fn discover_catalog() -> CatalogState {
        let apps = vec![
            entry("a", "vitadb", 1_000, 900, 4.0, "2024-06-01"),
            entry("b", "vitadb", 1_000, 800, 4.0, "2024-06-02"),
        ];
        let mut catalog = CatalogState::new(apps);
        catalog.top_rail = vec![0, 1];
        catalog.recent_rail = vec![1, 0];
        catalog.featured_index = Some(0);
        catalog.discover_focus = DiscoverFocus::Featured;
        catalog
    }

    #[test]
    fn browse_all_is_reachable_going_down_past_both_rails() {
        let mut catalog = discover_catalog();
        assert!(catalog.move_discover_focus(InputCommand::MoveDown));
        assert_eq!(catalog.discover_focus, DiscoverFocus::Top(0));
        assert!(catalog.move_discover_focus(InputCommand::MoveDown));
        assert_eq!(catalog.discover_focus, DiscoverFocus::New(0));
        assert!(catalog.move_discover_focus(InputCommand::MoveDown));
        assert_eq!(catalog.discover_focus, DiscoverFocus::BrowseAll);
        assert!(!catalog.move_discover_focus(InputCommand::MoveDown));
        assert_eq!(catalog.discover_focus, DiscoverFocus::BrowseAll);
    }

    #[test]
    fn browse_all_gives_the_focus_back_going_up() {
        let mut catalog = discover_catalog();
        catalog.discover_focus = DiscoverFocus::BrowseAll;
        assert!(catalog.move_discover_focus(InputCommand::MoveUp));
        assert_eq!(catalog.discover_focus, DiscoverFocus::New(0));
    }

    #[test]
    fn browse_all_is_reachable_with_no_rails_at_all() {
        let mut catalog = discover_catalog();
        catalog.top_rail.clear();
        catalog.recent_rail.clear();
        assert!(catalog.move_discover_focus(InputCommand::MoveDown));
        assert_eq!(catalog.discover_focus, DiscoverFocus::BrowseAll);
        assert!(catalog.move_discover_focus(InputCommand::MoveUp));
        assert_eq!(catalog.discover_focus, DiscoverFocus::Featured);
    }

    #[test]
    fn browse_all_focus_is_a_button_not_an_app() {
        let mut catalog = discover_catalog();
        catalog.discover_focus = DiscoverFocus::BrowseAll;
        assert_eq!(catalog.discover_focus_app_id(), None);
        catalog.clamp_discover_focus();
        assert_eq!(catalog.discover_focus, DiscoverFocus::BrowseAll); // survives a clamp
    }
}
