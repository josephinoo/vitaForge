use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
const EN_US_FTL: &str = include_str!("i18n/en-US.ftl");
const ES_ES_FTL: &str = include_str!("i18n/es-ES.ftl");
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}
impl Language {
    pub fn detect() -> Self {
        #[cfg(target_os = "vita")]
        {
            let mut lang: i32 = 0;
            unsafe {
                if vitasdk_sys::sceAppUtilSystemParamGetInt(
                    vitasdk_sys::SCE_SYSTEM_PARAM_ID_LANG as u32,
                    &mut lang,
                ) == 0 {
                    if lang == 5 {
                        return Language::Spanish;
                    }
                }
            }
        }
        if let Ok(sys_lang) = std::env::var("LANG").or_else(|_| std::env::var("LC_ALL")) {
            if sys_lang.to_lowercase().starts_with("es") {
                return Language::Spanish;
            }
        }
        Language::English
    }
    fn strings<R>(self, f: impl FnOnce(&LocalizedStrings) -> R) -> R {
        thread_local! {
            static EN: LocalizedStrings = LocalizedStrings::resolve(&build_bundle("en-US", EN_US_FTL));
            static ES: LocalizedStrings = LocalizedStrings::resolve(&build_bundle("es-ES", ES_ES_FTL));
        }
        match self {
            Language::English => EN.with(f),
            Language::Spanish => ES.with(f),
        }
    }
    fn with_bundle<R>(self, f: impl FnOnce(&FluentBundle<FluentResource>) -> R) -> R {
        thread_local! {
            static EN_BUNDLE: FluentBundle<FluentResource> = build_bundle("en-US", EN_US_FTL);
            static ES_BUNDLE: FluentBundle<FluentResource> = build_bundle("es-ES", ES_ES_FTL);
        }
        match self {
            Language::English => EN_BUNDLE.with(f),
            Language::Spanish => ES_BUNDLE.with(f),
        }
    }
    pub fn apps_count(self, count: usize) -> String {
        let mut args = FluentArgs::new();
        args.set("count", count);
        self.with_bundle(|bundle| format_message(bundle, "apps-count", Some(&args)))
    }
    pub fn category_label(self, cat: Option<crate::data::Category>) -> &'static str {
        self.strings(|s| match cat {
            None => s.category_all,
            Some(c) => match c {
                crate::data::Category::Emulator => s.category_emulator,
                crate::data::Category::Original => s.category_original,
                crate::data::Category::PsVitaGame => s.category_ps_vita_game,
                crate::data::Category::Ps1Game => s.category_ps1_game,
                crate::data::Category::PspGame => s.category_psp_game,
                crate::data::Category::Plugin => s.category_plugin,
                crate::data::Category::Port => s.category_port,
                crate::data::Category::Tool => s.category_tool,
                crate::data::Category::Utility => s.category_utility,
                crate::data::Category::Other => s.category_other,
            },
        })
    }
    pub fn sort_by_prefix(self) -> &'static str {
        self.strings(|s| s.sort_by_prefix)
    }
    pub fn sort_label(self, sort: crate::data::SortOrder) -> &'static str {
        self.strings(|s| match sort {
            crate::data::SortOrder::Downloads => s.sort_downloads,
            crate::data::SortOrder::Rating => s.sort_rating,
            crate::data::SortOrder::Recent => s.sort_recent,
            crate::data::SortOrder::Size => s.sort_size,
            crate::data::SortOrder::Name => s.sort_name,
        })
    }
    pub fn search_placeholder(self) -> &'static str {
        self.strings(|s| s.search_placeholder)
    }
    pub fn no_results(self) -> &'static str {
        self.strings(|s| s.no_results)
    }
    pub fn no_results_sub(self) -> &'static str {
        self.strings(|s| s.no_results_sub)
    }
    pub fn featured_label(self) -> &'static str {
        self.strings(|s| s.featured_label)
    }
    pub fn view_details_label(self) -> &'static str {
        self.strings(|s| s.view_details_label)
    }
    pub fn install(self) -> &'static str {
        self.strings(|s| s.install)
    }
    pub fn reinstall(self) -> &'static str {
        self.strings(|s| s.reinstall)
    }
    pub fn update(self) -> &'static str {
        self.strings(|s| s.update)
    }
    pub fn installed(self) -> &'static str {
        self.strings(|s| s.installed)
    }
    pub fn update_available(self) -> &'static str {
        self.strings(|s| s.update_available)
    }
    pub fn installed_version(self) -> &'static str {
        self.strings(|s| s.installed_version)
    }
    pub fn install_in_progress(self) -> &'static str {
        self.strings(|s| s.install_in_progress)
    }
    pub fn plugin_manual_note(self) -> &'static str {
        self.strings(|s| s.plugin_manual_note)
    }
    pub fn release_page(self) -> &'static str {
        self.strings(|s| s.release_page)
    }
    pub fn download(self) -> &'static str {
        self.strings(|s| s.download)
    }
    pub fn requirements(self) -> &'static str {
        self.strings(|s| s.requirements)
    }
    pub fn changelog(self) -> &'static str {
        self.strings(|s| s.changelog)
    }
    pub fn needs_game_data(self) -> &'static str {
        self.strings(|s| s.needs_game_data)
    }
    pub fn overview(self) -> &'static str {
        self.strings(|s| s.overview)
    }
    pub fn needs_nonpdrm(self) -> &'static str {
        self.strings(|s| s.needs_nonpdrm)
    }
    pub fn description(self) -> &'static str {
        self.strings(|s| s.description)
    }
    pub fn technical_info(self) -> &'static str {
        self.strings(|s| s.technical_info)
    }
    pub fn version(self) -> &'static str {
        self.strings(|s| s.version)
    }
    pub fn size(self) -> &'static str {
        self.strings(|s| s.size)
    }
    pub fn downloads(self) -> &'static str {
        self.strings(|s| s.downloads)
    }
    pub fn rating(self) -> &'static str {
        self.strings(|s| s.rating)
    }
    pub fn updated(self) -> &'static str {
        self.strings(|s| s.updated)
    }
    pub fn community(self) -> &'static str {
        self.strings(|s| s.community)
    }
    pub fn your_rating(self) -> &'static str {
        self.strings(|s| s.your_rating)
    }
    pub fn ratings_count(self) -> &'static str {
        self.strings(|s| s.ratings_count)
    }
    pub fn comments(self) -> &'static str {
        self.strings(|s| s.comments)
    }
    pub fn add_comment(self) -> &'static str {
        self.strings(|s| s.add_comment)
    }
    pub fn loading_comments(self) -> &'static str {
        self.strings(|s| s.loading_comments)
    }
    pub fn no_comments_yet(self) -> &'static str {
        self.strings(|s| s.no_comments_yet)
    }
    pub fn back(self) -> &'static str {
        self.strings(|s| s.back)
    }
    pub fn settings_title(self) -> &'static str {
        self.strings(|s| s.settings_title)
    }
    pub fn language_label(self) -> &'static str {
        self.strings(|s| s.language_label)
    }
    pub fn settings_storage(self) -> &'static str {
        self.strings(|s| s.settings_storage)
    }
    pub fn settings_version(self) -> &'static str {
        self.strings(|s| s.settings_version)
    }
    pub fn settings_catalog(self) -> &'static str {
        self.strings(|s| s.settings_catalog)
    }
    pub fn settings_icon_cache(self) -> &'static str {
        self.strings(|s| s.settings_icon_cache)
    }
    pub fn settings_catalog_cache(self) -> &'static str {
        self.strings(|s| s.settings_catalog_cache)
    }
    pub fn settings_hash_cache(self) -> &'static str {
        self.strings(|s| s.settings_hash_cache)
    }
    pub fn settings_total(self) -> &'static str {
        self.strings(|s| s.settings_total)
    }
    pub fn settings_clear_icons(self) -> &'static str {
        self.strings(|s| s.settings_clear_icons)
    }
    pub fn settings_clear_catalog(self) -> &'static str {
        self.strings(|s| s.settings_clear_catalog)
    }
    pub fn settings_purge_all(self) -> &'static str {
        self.strings(|s| s.settings_purge_all)
    }
    pub fn settings_cleared_icons(self, size: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("size", FluentValue::from(size));
        self.with_bundle(|bundle| format_message(bundle, "settings-cleared-icons", Some(&args)))
    }
    pub fn settings_cleared_catalog(self) -> &'static str {
        self.strings(|s| s.settings_cleared_catalog)
    }
    pub fn settings_purged_all(self, size: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("size", FluentValue::from(size));
        self.with_bundle(|bundle| format_message(bundle, "settings-purged-all", Some(&args)))
    }
    pub fn by_author(self, author: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("author", FluentValue::from(author));
        self.with_bundle(|bundle| format_message(bundle, "by-author", Some(&args)))
    }
    pub fn btn_open(self) -> &'static str {
        self.strings(|s| s.btn_open)
    }
    pub fn btn_back(self) -> &'static str {
        self.strings(|s| s.btn_back)
    }
    pub fn btn_search(self) -> &'static str {
        self.strings(|s| s.btn_search)
    }
    pub fn btn_category(self) -> &'static str {
        self.strings(|s| s.btn_category)
    }
    pub fn btn_tabs(self) -> &'static str {
        self.strings(|s| s.btn_tabs)
    }
    pub fn btn_clear(self) -> &'static str {
        self.strings(|s| s.btn_clear)
    }
    pub fn tab_discover(self) -> &'static str {
        self.strings(|s| s.tab_discover)
    }
    pub fn tab_library(self) -> &'static str {
        self.strings(|s| s.tab_library)
    }
    pub fn tab_updates(self) -> &'static str {
        self.strings(|s| s.tab_updates)
    }
    pub fn tab_search(self) -> &'static str {
        self.strings(|s| s.tab_search)
    }
    pub fn rail_top(self) -> &'static str {
        self.strings(|s| s.rail_top)
    }
    pub fn rail_new(self) -> &'static str {
        self.strings(|s| s.rail_new)
    }
    pub fn see_all(self) -> &'static str {
        self.strings(|s| s.see_all)
    }
    pub fn see_all_catalog(self) -> &'static str {
        self.strings(|s| s.see_all_catalog)
    }
    pub fn see_all_back(self) -> &'static str {
        self.strings(|s| s.see_all_back)
    }
    pub fn library_empty(self) -> &'static str {
        self.strings(|s| s.library_empty)
    }
    pub fn updates_empty(self) -> &'static str {
        self.strings(|s| s.updates_empty)
    }
    pub fn loading_msg(self) -> &'static str {
        self.strings(|s| s.loading_msg)
    }
    pub fn cancel_btn(self) -> &'static str {
        self.strings(|s| s.cancel_btn)
    }
    pub fn installed_version_value(self) -> &'static str {
        self.strings(|s| s.installed_version_value)
    }
}
fn build_bundle(locale: &str, source: &str) -> FluentBundle<FluentResource> {
    let langid: unic_langid::LanguageIdentifier =
        locale.parse().unwrap_or_else(|err| panic!("bad locale id {locale}: {err}"));
    let resource = FluentResource::try_new(source.to_owned())
        .unwrap_or_else(|(_, errs)| panic!("bad ftl syntax in {locale}: {errs:?}"));
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle.set_use_isolating(false);
    bundle.add_resource(resource).unwrap_or_else(|errs| panic!("duplicate ftl id in {locale}: {errs:?}"));
    bundle
}
fn format_message(bundle: &FluentBundle<FluentResource>, id: &str, args: Option<&FluentArgs>) -> String {
    let msg = bundle.get_message(id).unwrap_or_else(|| panic!("missing ftl id: {id}"));
    let pattern = msg.value().unwrap_or_else(|| panic!("ftl id {id} has no value"));
    let mut errors = vec![];
    let formatted = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        panic!("fluent format errors for {id}: {errors:?}");
    }
    formatted.into_owned()
}
struct LocalizedStrings {
    category_all: &'static str,
    category_emulator: &'static str,
    category_original: &'static str,
    category_ps_vita_game: &'static str,
    category_ps1_game: &'static str,
    category_psp_game: &'static str,
    category_plugin: &'static str,
    category_port: &'static str,
    category_tool: &'static str,
    category_utility: &'static str,
    category_other: &'static str,
    sort_by_prefix: &'static str,
    sort_downloads: &'static str,
    sort_rating: &'static str,
    sort_recent: &'static str,
    sort_size: &'static str,
    sort_name: &'static str,
    search_placeholder: &'static str,
    no_results: &'static str,
    no_results_sub: &'static str,
    featured_label: &'static str,
    view_details_label: &'static str,
    install: &'static str,
    reinstall: &'static str,
    update: &'static str,
    installed: &'static str,
    update_available: &'static str,
    installed_version: &'static str,
    install_in_progress: &'static str,
    plugin_manual_note: &'static str,
    release_page: &'static str,
    download: &'static str,
    requirements: &'static str,
    changelog: &'static str,
    needs_game_data: &'static str,
    overview: &'static str,
    needs_nonpdrm: &'static str,
    description: &'static str,
    technical_info: &'static str,
    version: &'static str,
    size: &'static str,
    downloads: &'static str,
    rating: &'static str,
    updated: &'static str,
    community: &'static str,
    your_rating: &'static str,
    ratings_count: &'static str,
    comments: &'static str,
    add_comment: &'static str,
    loading_comments: &'static str,
    no_comments_yet: &'static str,
    back: &'static str,
    btn_open: &'static str,
    btn_back: &'static str,
    btn_search: &'static str,
    btn_category: &'static str,
    btn_tabs: &'static str,
    btn_clear: &'static str,
    tab_discover: &'static str,
    tab_library: &'static str,
    tab_updates: &'static str,
    tab_search: &'static str,
    rail_top: &'static str,
    rail_new: &'static str,
    see_all: &'static str,
    see_all_catalog: &'static str,
    see_all_back: &'static str,
    library_empty: &'static str,
    updates_empty: &'static str,
    loading_msg: &'static str,
    settings_title: &'static str,
    language_label: &'static str,
    settings_storage: &'static str,
    settings_version: &'static str,
    settings_catalog: &'static str,
    settings_icon_cache: &'static str,
    settings_catalog_cache: &'static str,
    settings_hash_cache: &'static str,
    settings_total: &'static str,
    settings_clear_icons: &'static str,
    settings_clear_catalog: &'static str,
    settings_purge_all: &'static str,
    settings_cleared_catalog: &'static str,
    cancel_btn: &'static str,
    installed_version_value: &'static str,
}
impl LocalizedStrings {
    fn resolve(bundle: &FluentBundle<FluentResource>) -> Self {
        let r = |id: &str| -> &'static str {
            let text = format_message(bundle, id, None);
            Box::leak(text.into_boxed_str())
        };
        Self {
            category_all: r("category-all"),
            category_emulator: r("category-emulator"),
            category_original: r("category-original"),
            category_ps_vita_game: r("category-ps-vita-game"),
            category_ps1_game: r("category-ps1-game"),
            category_psp_game: r("category-psp-game"),
            category_plugin: r("category-plugin"),
            category_port: r("category-port"),
            category_tool: r("category-tool"),
            category_utility: r("category-utility"),
            category_other: r("category-other"),
            sort_by_prefix: r("sort-by-prefix"),
            sort_downloads: r("sort-downloads"),
            sort_rating: r("sort-rating"),
            sort_recent: r("sort-recent"),
            sort_size: r("sort-size"),
            sort_name: r("sort-name"),
            search_placeholder: r("search-placeholder"),
            no_results: r("no-results"),
            no_results_sub: r("no-results-sub"),
            featured_label: r("featured-label"),
            view_details_label: r("view-details-label"),
            install: r("install-btn"),
            reinstall: r("reinstall-btn"),
            update: r("update-btn"),
            installed: r("installed-label"),
            update_available: r("update-available"),
            installed_version: r("installed-version-label"),
            install_in_progress: r("install-in-progress"),
            plugin_manual_note: r("plugin-manual-note"),
            release_page: r("release-page"),
            download: r("download-btn"),
            requirements: r("requirements-label"),
            changelog: r("changelog-label"),
            needs_game_data: r("needs-game-data"),
            overview: r("overview-label"),
            needs_nonpdrm: r("needs-nonpdrm"),
            description: r("description-label"),
            technical_info: r("technical-info"),
            version: r("version-label"),
            size: r("size-label"),
            downloads: r("downloads-label"),
            rating: r("rating-label"),
            updated: r("updated-label"),
            community: r("community-label"),
            your_rating: r("your-rating"),
            ratings_count: r("ratings-count"),
            comments: r("comments-label"),
            add_comment: r("add-comment"),
            loading_comments: r("loading-comments"),
            no_comments_yet: r("no-comments-yet"),
            back: r("back-label"),
            btn_open: r("btn-open"),
            btn_back: r("btn-back"),
            btn_search: r("btn-search"),
            btn_category: r("btn-category"),
            btn_tabs: r("btn-tabs"),
            btn_clear: r("btn-clear"),
            tab_discover: r("tab-discover"),
            tab_library: r("tab-library"),
            tab_updates: r("tab-updates"),
            tab_search: r("tab-search"),
            rail_top: r("rail-top"),
            rail_new: r("rail-new"),
            see_all: r("see-all"),
            see_all_catalog: r("see-all-catalog"),
            see_all_back: r("see-all-back"),
            library_empty: r("library-empty"),
            updates_empty: r("updates-empty"),
            loading_msg: r("loading-msg"),
            settings_title: r("settings-title"),
            language_label: r("language-label"),
            settings_storage: r("settings-storage"),
            settings_version: r("settings-version"),
            settings_catalog: r("settings-catalog"),
            settings_icon_cache: r("settings-icon-cache"),
            settings_catalog_cache: r("settings-catalog-cache"),
            settings_hash_cache: r("settings-hash-cache"),
            settings_total: r("settings-total"),
            settings_clear_icons: r("settings-clear-icons"),
            settings_clear_catalog: r("settings-clear-catalog"),
            settings_purge_all: r("settings-purge-all"),
            settings_cleared_catalog: r("settings-cleared-catalog"),
            cancel_btn: r("cancel-btn"),
            installed_version_value: r("installed-version-value"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_strings_match_both_languages() {
        assert_eq!(Language::English.install(), "INSTALL");
        assert_eq!(Language::Spanish.install(), "INSTALAR");
        assert_eq!(Language::English.update(), "UPDATE");
        assert_eq!(Language::Spanish.update(), "ACTUALIZAR");
        assert_eq!(Language::English.no_results(), "No homebrews found");
        assert_eq!(Language::Spanish.no_results(), "No se encontraron resultados");
    }
    #[test]
    fn interpolated_strings_substitute_args() {
        assert_eq!(Language::English.apps_count(42), "42 APPS");
        assert_eq!(Language::Spanish.apps_count(42), "42 APPS");
        assert_eq!(Language::English.by_author("josephinoo"), "by josephinoo");
        assert_eq!(Language::Spanish.by_author("josephinoo"), "por josephinoo");
        assert!(Language::English.settings_cleared_icons("2.0 MB").contains("2.0 MB"));
        assert!(Language::Spanish.settings_purged_all("1.5 MB").contains("1.5 MB"));
        assert!(!Language::English.settings_cleared_catalog().is_empty());
        assert!(!Language::English.settings_purge_all().is_empty());
    }
    #[test]
    fn sort_label_covers_every_variant() {
        for order in crate::data::SortOrder::ALL {
            assert!(!Language::English.sort_label(order).is_empty());
            assert!(!Language::Spanish.sort_label(order).is_empty());
        }
    }
    #[test]
    fn category_label_none_is_all() {
        assert_eq!(Language::English.category_label(None), "ALL");
        assert_eq!(Language::Spanish.category_label(None), "ALL");
    }
}
