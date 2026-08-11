pub mod i18n;
pub mod icons;
pub mod text;
pub mod ui;
use crate::data::{self, AppEntry, Category, SortDirection, SortOrder, SourceCatalog};
use crate::input::{AppCommand, InputCommand};
use anyhow::Result;
use i18n::Language;
use icons::IconCache;
use tokio::sync::{oneshot, watch};
/// covers/screenshots) is worth precaching here — it's the one image every grid row needs.
fn icon_urls(apps: &[AppEntry]) -> Vec<String> {
    apps.iter().filter_map(|app| app.icon_url.clone()).collect()
}
pub struct CatalogState {
    pub apps: Vec<AppEntry>,
    sorted_indices: Vec<usize>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub search_requested: bool,
    pub category_filter: Option<Category>,
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
    pub source_scoped_count: usize,
    pub featured_index: Option<usize>,
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
            source_scoped_count: 0,
            featured_index: None,
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
            source_scoped_count: 0,
            featured_index: None,
        }
    }
    fn replace_apps(&mut self, apps: Vec<AppEntry>) {
        self.apps = apps;
        self.sorted_indices = self.sort_order_indices();
        self.recompute_dropdown_counts();
        self.refresh_filter_preserving_selection();
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
        data::settings::save(&data::settings::Settings {
            sort_order: self.sort_order,
            sort_direction: self.sort_direction,
        });
    }
    fn flip_sort_direction(&mut self) {
        self.sort_direction = self.sort_direction.flipped();
        self.resort();
        data::settings::save(&data::settings::Settings {
            sort_order: self.sort_order,
            sort_direction: self.sort_direction,
        });
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
                        || app.author_lower.contains(&query)
                        || app.titleid_lower.contains(&query))
            })
            .collect();
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.selection_active = false;
        } else {
            self.selected = 0;
            self.selection_active = true;
            self.scroll_reset = true;
        }
        self.is_commercial_view = self.source_filter == Some(SourceCatalog::Nps);
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
    fn recompute_dropdown_counts(&mut self) {
        let apps = &self.apps;
        let source_filter = self.source_filter;
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
        scroll_delta: f32,
        comments: Vec<data::api::Comment>,
        comments_loaded: bool,
        comment_entry_requested: bool,
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
pub struct App {
    pub state: AppState,
    pub icons: IconCache,
    pub installed: crate::install::installed::InstalledIndex,
    pub lang: Language,
    pub install: Option<InstallJob>,
    needs_installed_rescan: bool,
    load_rx: Option<oneshot::Receiver<CatalogSource>>,
    comments_rx: Option<oneshot::Receiver<(String, Vec<data::api::Comment>)>>,
    social_rx: Option<oneshot::Receiver<(String, data::api::Social)>>,
    audio: crate::audio::AudioEngine,
}
pub struct InstallJob {
    pub app_id: String,
    pub app_id_title: String,
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
        Ok(Self {
            state: AppState::Loading,
            icons: IconCache::new(),
            installed: crate::install::installed::InstalledIndex::new(),
            lang: Language::detect(),
            install: None,
            needs_installed_rescan: true,
            load_rx: Some(rx),
            comments_rx: None,
            social_rx: None,
            audio: crate::audio::AudioEngine::new(),
        })
    }
    pub fn install_busy(&self) -> bool {
        self.install.as_ref().is_some_and(|job| !job.progress.is_finished())
    }
    pub fn tick(&mut self, ctx: &egui::Context) -> Result<()> {
        if self.needs_installed_rescan {
            let entries: &[AppEntry] = match &self.state {
                AppState::Catalog(catalog) => &catalog.apps,
                AppState::Detail { previous, .. } | AppState::Settings { previous, .. } => &previous.apps,
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
                            let catalog = CatalogState::new(apps);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.icons.precache_background(icon_urls(&catalog.apps));
                            self.state = AppState::Catalog(catalog);
                        }
                        AppState::Catalog(catalog) => {
                            catalog.replace_apps(apps);
                            self.installed.force_refresh(ctx, &catalog.apps);
                            self.icons.precache_background(icon_urls(&catalog.apps));
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
                    let catalog = CatalogState::new(Vec::new());
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
            AppCommand::Input(InputCommand::Back) => {
                if matches!(self.state, AppState::Settings { .. }) {
                    return self.handle_command(AppCommand::CloseSettings);
                }
                self.back_to_catalog();
            }
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
                    catalog.refresh_filter();
                    self.audio.play(crate::audio::Sfx::TabTransition);
                }
            }
            AppCommand::SetSourceFilter(source) => {
                if let AppState::Catalog(catalog) = &mut self.state {
                    catalog.source_filter = source;
                    catalog.category_filter = None;
                    catalog.recompute_dropdown_counts();
                    catalog.refresh_filter();
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
                    let lang = if *selected == 0 { Language::English } else { Language::Spanish };
                    return self.handle_command(AppCommand::SetLanguage(lang));
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
                            InputCommand::MoveDown => *selected = (*selected + 1).min(1),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            AppCommand::SelectApp { index } => self.open_app(index),
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
                    self.audio.play(crate::audio::Sfx::Launch);
                }
            }
            AppCommand::DismissInstall => self.install = None,
            AppCommand::OpenSettings => {
                let previous = match std::mem::replace(&mut self.state, AppState::Loading) {
                    AppState::Catalog(catalog) => Box::new(catalog),
                    AppState::Detail { previous, .. } => previous,
                    other @ (AppState::Loading | AppState::Settings { .. }) => {
                        self.state = other;
                        return Ok(());
                    }
                };
                let selected = if self.lang == Language::English { 0 } else { 1 };
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
                if let AppState::Settings { selected, .. } = &mut self.state {
                    *selected = if lang == Language::English { 0 } else { 1 };
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
        let placeholder = AppState::Detail {
            app: entry.clone(),
            previous: Box::new(CatalogState::empty()),
            scroll_delta: 0.0,
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
        };
        self.audio.play(crate::audio::Sfx::IntoDetail);
    }
    pub fn clear_one_shot_ui_state(&mut self) {
        match &mut self.state {
            AppState::Catalog(catalog) => {
                catalog.scroll_to_selected = false;
                catalog.scroll_reset = false;
            }
            AppState::Detail { scroll_delta, .. } => *scroll_delta = 0.0,
            _ => {}
        }
    }
    fn back_to_catalog(&mut self) {
        if self.install_busy() {
            return;
        }
        if let AppState::Catalog(catalog) = &mut self.state {
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
            previous.search_query.clear();
            previous.search_requested = false;
            // Keep whatever app was selected before opening the detail screen instead of
            // jumping back to the top of the list.
            previous.refresh_filter_preserving_selection();
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

    fn entry(id: &str, source_catalog: &str, size_bytes: u64, downloads: u64, rating: f32, updated_at: &str) -> AppEntry {
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
        // Tests must not depend on whatever sort preference a previous test run
        // persisted to disk (set_sort() writes settings.json as a side effect).
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
}
