pub mod i18n;
pub mod icons;
pub mod text;
pub mod ui;

use crate::data::{self, AppEntry, Category, SortOrder, SourceCatalog};
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
    /// Scoped to `source_filter`: only categories present in the selected
    /// catalog are ever offered, same as vitaforge-core's own filter form.
    pub category_filter: Option<Category>,
    pub source_filter: Option<SourceCatalog>,
    pub sort_order: SortOrder,
    pub selected: usize,

    pub selection_active: bool,
    pub scroll_to_selected: bool,
    /// Whether any currently-filtered entry uses commercial (box-art) cover
    /// art rather than a homebrew icon — decides the grid's aspect ratio.
    /// Computed once per filter change instead of scanned every frame.
    pub is_commercial_view: bool,

    /// `(source, count)` over the whole catalog, one row per entry in
    /// `SourceCatalog::ALL` that has at least one match. Feeds the "source"
    /// filter dropdown's row labels. Computed once per filter change instead
    /// of an `apps.iter().filter(...).count()` scan per row, per frame, for
    /// as long as that dropdown's popup stays open.
    pub source_counts: Vec<(SourceCatalog, usize)>,
    /// `(category, count)` scoped to `source_filter`, same reasoning as
    /// `source_counts` above — feeds the "category" dropdown.
    pub category_counts: Vec<(Category, usize)>,
    /// How many entries match `source_filter` alone (ignoring
    /// `category_filter`) — the "All categories (N)" row's count.
    pub source_scoped_count: usize,
    /// The sort dropdown's always-visible label (e.g. "Sort by Downloads"),
    /// recomputed only when `sort_order` changes instead of `format!`-ed and
    /// text-shaped fresh on every catalog frame.
    pub sort_label: String,
}

impl CatalogState {
    fn new(apps: Vec<AppEntry>, lang: Language) -> Self {
        let mut state = Self {
            apps,
            sorted_indices: Vec::new(),
            filtered_indices: Vec::new(),
            search_query: String::new(),
            search_requested: false,
            category_filter: None,
            source_filter: None,
            sort_order: SortOrder::Downloads,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            is_commercial_view: false,
            source_counts: Vec::new(),
            category_counts: Vec::new(),
            source_scoped_count: 0,
            sort_label: String::new(),
        };
        state.resort(lang);
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
            source_filter: None,
            sort_order: SortOrder::Downloads,
            selected: 0,
            selection_active: false,
            scroll_to_selected: false,
            is_commercial_view: false,
            source_counts: Vec::new(),
            category_counts: Vec::new(),
            source_scoped_count: 0,
            sort_label: String::new(),
        }
    }

    /// Swaps in a freshly-fetched catalog while keeping the user's current
    /// sort/filter/search picks and scroll position — used when the live
    /// fetch lands after the catalog was already showing (e.g. restored
    /// from `cache.json`), instead of building a whole new `CatalogState`
    /// and silently resetting everything the user had set up.
    fn replace_apps(&mut self, apps: Vec<AppEntry>, lang: Language) {
        self.apps = apps;
        self.resort(lang);
    }

    fn resort(&mut self, lang: Language) {
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
        self.sort_label = format!("{} {}", lang.sort_by_prefix(), lang.sort_label(self.sort_order));
        self.refresh_filter();
    }

    fn refresh_filter(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        let apps = &self.apps;
        let category_filter = self.category_filter;
        let source_filter = self.source_filter;

        self.filtered_indices = self
            .sorted_indices
            .iter()
            .copied()
            .filter(|&index| {
                let app = &apps[index];
                let matches_cat = category_filter.is_none_or(|c| c as u8 == app.category as u8);
                let matches_source = source_filter.is_none_or(|s| s.matches(&app.source_catalog));
                matches_cat
                    && matches_source
                    && (query.is_empty()
                        || app.name_lower.contains(&query)
                        || app.author.to_lowercase().contains(&query)
                        || app.titleid.to_lowercase().contains(&query))
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

        // Dropdown row counts, recomputed here (once per filter change)
        // instead of once per row per frame while a dropdown popup is open —
        // that used to be an `apps.iter().filter(...).count()` scan over the
        // whole catalog for every visible row, every frame, for as long as
        // the popup stayed open.
        self.source_counts = SourceCatalog::ALL
            .into_iter()
            .map(|source| (source, apps.iter().filter(|app| source.matches(&app.source_catalog)).count()))
            .filter(|&(_, count)| count > 0)
            .collect();
        self.source_scoped_count =
            apps.iter().filter(|app| source_filter.is_none_or(|s| s.matches(&app.source_catalog))).count();
        self.category_counts = Category::ALL
            .into_iter()
            .map(|category| {
                let count = apps
                    .iter()
                    .filter(|app| {
                        app.category == category && source_filter.is_none_or(|s| s.matches(&app.source_catalog))
                    })
                    .count();
                (category, count)
            })
            .filter(|&(_, count)| count > 0)
            .collect();

        // Recomputed here (once per filter change) instead of every frame: a
        // "mixed" view still gets 2:3 box-art cards as soon as any commercial
        // entry is present; an all-homebrew view stays square.
        self.is_commercial_view = self
            .filtered_indices
            .iter()
            .any(|&index| self.apps.get(index).is_some_and(|e| e.platform.is_commercial()));
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
        comments: Vec<data::api::Comment>,
        comments_loaded: bool,
        comment_entry_requested: bool,
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
    pub loading_start_time: std::time::Instant,
    self_update_rx: Option<oneshot::Receiver<SelfUpdateInfo>>,
    comments_rx: Option<oneshot::Receiver<(String, Vec<data::api::Comment>)>>,
    social_rx: Option<oneshot::Receiver<(String, data::api::Social)>>,
}

pub struct InstallJob {
    pub app_id: String,

    pub app_id_title: String,
    /// Human-readable name, used for the system notification on completion.
    pub title: String,
    pub progress: crate::install::Progress,
    rx: watch::Receiver<crate::install::Progress>,
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

        Ok(Self {
            state: AppState::Loading,
            icons: IconCache::new(),
            installed: crate::install::installed::InstalledIndex::new(),
            lang: Language::detect(),
            install: None,
            self_update: None,
            needs_installed_rescan: true,
            load_rx: Some(rx),
            loading_start_time: std::time::Instant::now(),
            self_update_rx: Some(self_update_rx),
            comments_rx: None,
            social_rx: None,
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
                crate::install::notify::install_finished(&job.title);
                let app_id = job.app_id.clone();
                tokio::spawn(async move {
                    if let Err(err) = data::api::notify_install(&app_id).await {
                        eprintln!("install counter notify failed: {err:#}");
                    }
                });
            } else if let crate::install::Progress::Failed(reason) = &job.progress
                && previous != job.progress
            {
                crate::install::notify::install_failed(&job.title, reason);
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

        let Some(rx) = &mut self.load_rx else { return Ok(()) };
        match rx.try_recv() {

            Ok(CatalogSource::Live(apps)) => {
                self.load_rx = None;
                if !self.install_busy() {
                    match &mut self.state {
                        AppState::Loading => {
                            let catalog = CatalogState::new(apps, self.lang);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.state = AppState::Catalog(catalog);
                        }
                        // A cache already populated the catalog before this
                        // fetch landed — update it in place instead of
                        // discarding the fresh data, which is what silently
                        // left stale entries on screen forever once a
                        // `cache.json` existed.
                        AppState::Catalog(catalog) => {
                            catalog.replace_apps(apps, self.lang);
                            self.installed.force_refresh(ctx, &catalog.apps);
                        }
                        AppState::Detail { previous, .. } => {
                            previous.replace_apps(apps, self.lang);
                            self.installed.force_refresh(ctx, &previous.apps);
                        }
                    }
                    ctx.request_repaint();
                }
            }
            Ok(CatalogSource::Failed) => {
                self.load_rx = None;
                // No on-disk fallback anymore — a failed fetch means an
                // empty catalog (the "no homebrews found" screen) rather
                // than silently reusing stale data.
                if matches!(self.state, AppState::Loading) && !self.install_busy() {
                    let catalog = CatalogState::new(Vec::new(), self.lang);
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
            AppCommand::SetSourceFilter(source) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.source_filter = source;
                    // Changing the catalog can invalidate the current category
                    // pick (e.g. "PS Vita Game" only exists under PKGj) — clear
                    // it rather than leave a filter that now matches nothing.
                    catalog.category_filter = None;
                    catalog.refresh_filter();
                }
            }
            AppCommand::SetSortOrder(sort) => {
                let lang = self.lang;
                if let AppState::Catalog(catalog) = &mut self.state
                    && catalog.sort_order != sort
                {
                    catalog.sort_order = sort;
                    catalog.resort(lang);
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
                    let title = entry.name.clone();
                    let rx = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob { app_id, app_id_title, title, progress, rx });
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
                        content_id: None,
                        name: format!("VitaForge {}", info.tag),
                        original_name: None,
                        overview: Vec::new(),
                        name_lower: "vitaforge".to_owned(),
                        author: "josephinoo".to_owned(),
                        description: format!("Self-update for VitaForge {}", info.tag),
                        long_description: String::new(),
                        requirements: String::new(),
                        changelog: String::new(),
                        release_page: Some("https://github.com/josephinoo/vitaForge/releases".to_owned()),
                        category: Category::Utility,
                        platform: crate::data::Platform::Vita,
                        kind: "app".to_owned(),
                        icon_url: None,
                        cover_url: None,
                        background_url: None,
                        screenshot_urls: Vec::new(),
                        download_url: info.vpk_url.clone(),
                        source: Some(SELF_REPO_URL.to_owned()),
                        version: info.tag.clone(),
                        region: None,
                        source_catalog: "self".to_owned(),
                        source_labels: Vec::new(),
                        hash: String::new(),
                        hash2: String::new(),
                        data_url: None,
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
                    };
                    let app_id = entry.id.clone();
                    let app_id_title = entry.titleid.clone();
                    let title = entry.name.clone();
                    let rx = crate::install::start(entry);
                    let progress = rx.borrow().clone();
                    self.install = Some(InstallJob { app_id, app_id_title, title, progress, rx });
                }
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
                }
            }
            AppCommand::CloseCommentEntry => {
                if let AppState::Detail { comment_entry_requested, .. } = &mut self.state {
                    *comment_entry_requested = false;
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
                        data::api::Comment {
                            id: String::new(),
                            author_name: author_name.clone(),
                            content: content.clone(),
                            created_at: String::new(),
                        },
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

        let placeholder = AppState::Detail {
            app: entry.clone(),
            previous: Box::new(CatalogState::empty()),
            origin,
            scroll_offset: 0.0,
            comments: Vec::new(),
            comments_loaded: false,
            comment_entry_requested: false,
        };
        let AppState::Catalog(mut catalog) = std::mem::replace(&mut self.state, placeholder) else {
            unreachable!("checked above that self.state is Catalog")
        };
        catalog.selected = filtered_index;

        catalog.selection_active = true;

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

        // Likes and ratings come back from the server rather than the cached
        // catalog, which cannot know about them (see `api::fetch_social`).
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
            origin,
            scroll_offset: 0.0,
            comments: Vec::new(),
            comments_loaded: false,
            comment_entry_requested: false,
        };
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
        if let AppState::Detail { app, previous, .. } = &mut self.state {
            // The detail screen works on its own copy of the entry, so a like
            // or a rating given here has to be carried back into the catalog —
            // otherwise reopening the app showed the pre-click state again and
            // the vote looked like it had been thrown away.
            if let Some(original) = previous.apps.iter_mut().find(|other| other.id == app.id) {
                original.user_liked = app.user_liked;
                original.user_rating = app.user_rating;
                original.likes_count = app.likes_count;
                original.ratings_count = app.ratings_count;
                original.comments_count = app.comments_count;
                original.rating = app.rating;
            }
            let previous = std::mem::replace(previous.as_mut(), CatalogState::empty());
            self.state = AppState::Catalog(previous);
            self.needs_installed_rescan = true;
        }
    }
}
