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

    pub fn apps_count(self, count: usize) -> String {
        match self {
            Language::Spanish => format!("{count} APPS"),
            Language::English => format!("{count} APPS"),
        }
    }

    pub fn category_label(self, cat: Option<crate::data::Category>) -> &'static str {
        match cat {
            None => "ALL",
            Some(c) => match c {
                crate::data::Category::Emulator => "Emulator",
                crate::data::Category::Original => "Original",
                crate::data::Category::PsVitaGame => "PS Vita Game",
                crate::data::Category::Ps1Game => "PS1 Game",
                crate::data::Category::PspGame => "PSP Game",
                crate::data::Category::Plugin => "Plugin",
                crate::data::Category::Port => "Port",
                crate::data::Category::Tool => "Tool",
                crate::data::Category::Utility => "Utility",
                crate::data::Category::Other => "Other",
            },
        }
    }

    pub fn sort_by_prefix(self) -> &'static str {
        match self {
            Language::Spanish => "Ordenar por:",
            Language::English => "Sort by:",
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

    pub fn plugin_manual_note(self) -> &'static str {
        match self {
            Language::Spanish => {
                "Se descarga en ux0:data/vitaforge/plugins. Debes moverlo y añadirlo a config.txt tú mismo."
            }
            Language::English => {
                "Downloaded to ux0:data/vitaforge/plugins. Moving it and editing config.txt is up to you."
            }
        }
    }

    pub fn release_page(self) -> &'static str {
        match self {
            Language::Spanish => "Página del autor",
            Language::English => "Release page",
        }
    }

    pub fn download(self) -> &'static str {
        match self {
            Language::Spanish => "DESCARGAR",
            Language::English => "DOWNLOAD",
        }
    }

    pub fn requirements(self) -> &'static str {
        match self {
            Language::Spanish => "Requisitos",
            Language::English => "Requirements",
        }
    }

    pub fn changelog(self) -> &'static str {
        match self {
            Language::Spanish => "Novedades",
            Language::English => "What's new",
        }
    }

    pub fn needs_game_data(self) -> &'static str {
        match self {
            Language::Spanish => "REQUIERE DATOS ADICIONALES",
            Language::English => "NEEDS EXTRA GAME DATA",
        }
    }

    pub fn overview(self) -> &'static str {
        match self {
            Language::Spanish => "Resumen",
            Language::English => "Overview",
        }
    }

    pub fn needs_nonpdrm(self) -> &'static str {
        match self {
            Language::Spanish => "Requiere NoNpDrm / fake license",
            Language::English => "Requires NoNpDrm / fake license",
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

    pub fn community(self) -> &'static str {
        match self {
            Language::Spanish => "Comunidad",
            Language::English => "Community",
        }
    }

    pub fn your_rating(self) -> &'static str {
        match self {
            Language::Spanish => "Tu valoración",
            Language::English => "Your rating",
        }
    }

    pub fn ratings_count(self) -> &'static str {
        match self {
            Language::Spanish => "Valoraciones",
            Language::English => "Ratings",
        }
    }

    pub fn comments(self) -> &'static str {
        match self {
            Language::Spanish => "Comentarios",
            Language::English => "Comments",
        }
    }

    pub fn add_comment(self) -> &'static str {
        match self {
            Language::Spanish => "+ Añadir comentario",
            Language::English => "+ Add comment",
        }
    }

    pub fn loading_comments(self) -> &'static str {
        match self {
            Language::Spanish => "Cargando comentarios…",
            Language::English => "Loading comments…",
        }
    }

    pub fn no_comments_yet(self) -> &'static str {
        match self {
            Language::Spanish => "Aún no hay comentarios. ¡Sé el primero!",
            Language::English => "No comments yet. Be the first!",
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
