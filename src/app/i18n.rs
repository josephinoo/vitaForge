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

    pub fn discover(self) -> &'static str {
        match self {
            Language::Spanish => "DESCUBRIR",
            Language::English => "DISCOVER",
        }
    }

    pub fn apps_count(self, count: usize) -> String {
        match self {
            Language::Spanish => format!("{count} APPS"),
            Language::English => format!("{count} APPS"),
        }
    }

    pub fn category_label(self, cat: Option<crate::data::Category>) -> &'static str {
        match cat {
            None => match self {
                Language::Spanish => "TODOS",
                Language::English => "ALL",
            },
            Some(c) => match c {
                crate::data::Category::Game => match self {
                    Language::Spanish => "Juegos",
                    Language::English => "Games",
                },
                crate::data::Category::Emulator => match self {
                    Language::Spanish => "Emuladores",
                    Language::English => "Emulators",
                },
                crate::data::Category::Utility => match self {
                    Language::Spanish => "Utilidades",
                    Language::English => "Utilities",
                },
                crate::data::Category::Port => match self {
                    Language::Spanish => "Ports",
                    Language::English => "Ports",
                },
                crate::data::Category::Plugin => match self {
                    Language::Spanish => "Plugins",
                    Language::English => "Plugins",
                },
                crate::data::Category::Media => match self {
                    Language::Spanish => "Multimedia",
                    Language::English => "Media",
                },
                crate::data::Category::Theme => match self {
                    Language::Spanish => "Temas",
                    Language::English => "Themes",
                },
                crate::data::Category::Other => match self {
                    Language::Spanish => "Otros",
                    Language::English => "Other",
                },
            },
        }
    }

    pub fn sort_by_prefix(self) -> &'static str {
        match self {
            Language::Spanish => "ORDENAR:",
            Language::English => "SORT BY:",
        }
    }

    pub fn sort_label(self, sort: crate::data::SortOrder) -> &'static str {
        match sort {
            crate::data::SortOrder::Downloads => match self {
                Language::Spanish => "Más descargados",
                Language::English => "Most downloaded",
            },
            crate::data::SortOrder::Rating => match self {
                Language::Spanish => "Mejor valorados",
                Language::English => "Top rated",
            },
            crate::data::SortOrder::Recent => match self {
                Language::Spanish => "Recientes",
                Language::English => "Recently updated",
            },
            crate::data::SortOrder::Size => match self {
                Language::Spanish => "Tamaño",
                Language::English => "Size",
            },
            crate::data::SortOrder::NameAsc => match self {
                Language::Spanish => "A - Z",
                Language::English => "A - Z",
            },
        }
    }

    pub fn search_placeholder(self) -> &'static str {
        match self {
            Language::Spanish => "Buscar homebrew...",
            Language::English => "Search...",
        }
    }

    pub fn no_results(self) -> &'static str {
        match self {
            Language::Spanish => "No se encontraron resultados",
            Language::English => "No homebrews found",
        }
    }

    pub fn no_results_sub(self) -> &'static str {
        match self {
            Language::Spanish => "Intenta cambiar los filtros o la búsqueda",
            Language::English => "Try changing filters or search terms",
        }
    }

    pub fn featured(self) -> &'static str {
        match self {
            Language::Spanish => "DESTACADO",
            Language::English => "FEATURED",
        }
    }

    pub fn view_details(self) -> &'static str {
        match self {
            Language::Spanish => "VER DETALLES",
            Language::English => "VIEW DETAILS",
        }
    }

    pub fn install(self) -> &'static str {
        match self {
            Language::Spanish => "INSTALAR",
            Language::English => "INSTALL",
        }
    }

    pub fn reinstall(self) -> &'static str {
        match self {
            Language::Spanish => "REINSTALAR",
            Language::English => "REINSTALL",
        }
    }

    pub fn update(self) -> &'static str {
        match self {
            Language::Spanish => "ACTUALIZAR",
            Language::English => "UPDATE",
        }
    }

    pub fn installed(self) -> &'static str {
        match self {
            Language::Spanish => "INSTALADO",
            Language::English => "INSTALLED",
        }
    }

    pub fn update_available(self) -> &'static str {
        match self {
            Language::Spanish => "ACTUALIZACIÓN DISPONIBLE",
            Language::English => "UPDATE AVAILABLE",
        }
    }

    pub fn installed_version(self) -> &'static str {
        match self {
            Language::Spanish => "Estado",
            Language::English => "Status",
        }
    }

    pub fn install_in_progress(self) -> &'static str {
        match self {
            Language::Spanish => "Instalación en curso, no salgas de esta pantalla",
            Language::English => "Install running - stay on this screen",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Language::Spanish => "Descripción",
            Language::English => "Description",
        }
    }

    pub fn technical_info(self) -> &'static str {
        match self {
            Language::Spanish => "Información Técnica",
            Language::English => "Technical Details",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Language::Spanish => "Versión",
            Language::English => "Version",
        }
    }

    pub fn size(self) -> &'static str {
        match self {
            Language::Spanish => "Tamaño",
            Language::English => "Size",
        }
    }

    pub fn downloads(self) -> &'static str {
        match self {
            Language::Spanish => "Descargas",
            Language::English => "Downloads",
        }
    }

    pub fn rating(self) -> &'static str {
        match self {
            Language::Spanish => "Valoración",
            Language::English => "Rating",
        }
    }

    pub fn updated(self) -> &'static str {
        match self {
            Language::Spanish => "Actualizado",
            Language::English => "Updated",
        }
    }

    pub fn back(self) -> &'static str {
        match self {
            Language::Spanish => "Atrás",
            Language::English => "Back",
        }
    }

    pub fn by_author(self, author: &str) -> String {
        match self {
            Language::Spanish => format!("por {author}"),
            Language::English => format!("by {author}"),
        }
    }

    pub fn btn_open(self) -> &'static str {
        match self {
            Language::Spanish => "Abrir",
            Language::English => "Select",
        }
    }

    pub fn btn_back(self) -> &'static str {
        match self {
            Language::Spanish => "Atrás",
            Language::English => "Back",
        }
    }

    pub fn btn_search(self) -> &'static str {
        match self {
            Language::Spanish => "Buscar",
            Language::English => "Search",
        }
    }

    pub fn btn_category(self) -> &'static str {
        match self {
            Language::Spanish => "Categorías",
            Language::English => "Categories",
        }
    }

    pub fn loading_msg(self) -> &'static str {
        match self {
            Language::Spanish => "Cargando catálogo...",
            Language::English => "Fetching catalog...",
        }
    }
}
