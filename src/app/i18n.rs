#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
}

impl Language {
    pub fn detect() -> Self {
        Language::English
    }

    pub fn apps_count(self, count: usize) -> String {
        format!("{count} APPS")
    }

    pub fn category_label(self, cat: Option<crate::data::Category>) -> &'static str {
        match cat {
            None => "ALL",
            Some(c) => match c {
                crate::data::Category::Game => "Games",
                crate::data::Category::Emulator => "Emulators",
                crate::data::Category::Utility => "Utilities",
                crate::data::Category::Port => "Ports",
                crate::data::Category::Plugin => "Plugins",
                crate::data::Category::Media => "Media",
                crate::data::Category::Theme => "Themes",
                crate::data::Category::Other => "Other",
            },
        }
    }

    pub fn sort_by_prefix(self) -> &'static str {
        "Sort by:"
    }

    pub fn sort_label(self, sort: crate::data::SortOrder) -> &'static str {
        match sort {
            crate::data::SortOrder::Downloads => "Most downloaded",
            crate::data::SortOrder::Rating => "Top rated",
            crate::data::SortOrder::Recent => "Recently updated",
            crate::data::SortOrder::Size => "Size",
            crate::data::SortOrder::NameAsc => "A - Z",
        }
    }

    pub fn search_placeholder(self) -> &'static str {
        "Search..."
    }

    pub fn no_results(self) -> &'static str {
        "No homebrews found"
    }

    pub fn no_results_sub(self) -> &'static str {
        "Try changing filters or search terms"
    }

    pub fn install(self) -> &'static str {
        "INSTALL"
    }

    pub fn reinstall(self) -> &'static str {
        "REINSTALL"
    }

    pub fn update(self) -> &'static str {
        "UPDATE"
    }

    pub fn installed(self) -> &'static str {
        "INSTALLED"
    }

    pub fn update_available(self) -> &'static str {
        "UPDATE AVAILABLE"
    }

    pub fn installed_version(self) -> &'static str {
        "Status"
    }

    pub fn install_in_progress(self) -> &'static str {
        "Install running - stay on this screen"
    }

    pub fn plugin_manual_note(self) -> &'static str {
        "Downloaded to ux0:data/vitaforge/plugins. Moving it and editing config.txt is up to you."
    }

    pub fn release_page(self) -> &'static str {
        "Release page"
    }

    pub fn download(self) -> &'static str {
        "DOWNLOAD"
    }

    pub fn requirements(self) -> &'static str {
        "Requirements"
    }

    pub fn changelog(self) -> &'static str {
        "What's new"
    }

    pub fn needs_game_data(self) -> &'static str {
        "NEEDS EXTRA GAME DATA"
    }

    pub fn description(self) -> &'static str {
        "Description"
    }

    pub fn technical_info(self) -> &'static str {
        "Technical Details"
    }

    pub fn version(self) -> &'static str {
        "Version"
    }

    pub fn size(self) -> &'static str {
        "Size"
    }

    pub fn downloads(self) -> &'static str {
        "Downloads"
    }

    pub fn rating(self) -> &'static str {
        "Rating"
    }

    pub fn updated(self) -> &'static str {
        "Updated"
    }

    pub fn back(self) -> &'static str {
        "Back"
    }

    pub fn by_author(self, author: &str) -> String {
        format!("by {author}")
    }

    pub fn btn_open(self) -> &'static str {
        "Select"
    }

    pub fn btn_back(self) -> &'static str {
        "Back"
    }

    pub fn btn_search(self) -> &'static str {
        "Search"
    }

    pub fn btn_category(self) -> &'static str {
        "Categories"
    }

    pub fn loading_msg(self) -> &'static str {
        "Loading app catalog & pre-caching icons..."
    }
}
