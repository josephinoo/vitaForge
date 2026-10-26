use crate::data::{SortDirection, SortOrder};
use serde::{Deserialize, Serialize};

const SETTINGS_PATH: &str = "ux0:data/vitaforge/settings.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub sort_order: SortOrder,
    #[serde(default)]
    pub sort_direction: SortDirection,
    #[serde(default)]
    pub language: Option<crate::app::i18n::Language>,
    #[serde(default = "default_install_notifications")]
    pub install_notifications: bool,
    #[serde(default)]
    pub theme: crate::app::ui::theme::ThemeId,
}

const fn default_install_notifications() -> bool { true }

impl Default for Settings {
    fn default() -> Self {
        Self {
            sort_order: SortOrder::default(),
            sort_direction: SortDirection::default(),
            language: None,
            install_notifications: true,
            theme: Default::default(),
        }
    }
}

pub fn set_language(language: crate::app::i18n::Language) {
    let mut settings = load();
    settings.language = Some(language);
    save(&settings);
}

pub fn set_theme(theme: crate::app::ui::theme::ThemeId) {
    let mut settings = load();
    settings.theme = theme;
    save(&settings);
}

pub fn set_install_notifications(enabled: bool) {
    let mut settings = load();
    settings.install_notifications = enabled;
    save(&settings);
}

static SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn load() -> Settings {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match std::fs::read(SETTINGS_PATH) {
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(settings) => {
                crate::install::log_file(&format!("settings loaded: theme={:?}", settings.theme));
                settings
            }
            Err(err) => {
                crate::install::log_file(&format!("settings json parse failed: {err}"));
                Settings::default()
            }
        },
        Err(err) => {
            crate::install::log_file(&format!("settings read failed from {SETTINGS_PATH}: {err}"));
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = std::path::Path::new(SETTINGS_PATH).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_vec_pretty(settings) {
        Ok(bytes) => {
            let tmp_path = format!("{SETTINGS_PATH}.tmp");
            match std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    if let Err(err) = file.write_all(&bytes) {
                        crate::install::log_file(&format!("settings write error: {err}"));
                    } else if let Err(err) = file.sync_all() {
                        crate::install::log_file(&format!("settings sync error: {err}"));
                    } else {
                        drop(file);
                        if let Err(err) = std::fs::rename(&tmp_path, SETTINGS_PATH) {
                            crate::install::log_file(&format!("settings rename error: {err}"));
                        } else {
                            crate::install::log_file(&format!("settings saved: theme={:?}", settings.theme));
                        }
                    }
                }
                Err(err) => {
                    crate::install::log_file(&format!("settings file open error: {err}"));
                }
            }
        }
        Err(err) => {
            crate::install::log_file(&format!("settings serialize error: {err}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_notifications_default_to_enabled() {
        assert!(Settings::default().install_notifications);
    }

    #[test]
    fn theme_round_trip() {
        let original = load().theme;
        let test_theme = crate::app::ui::theme::ThemeId::Sunset;
        set_theme(test_theme);
        assert_eq!(load().theme, test_theme);
        set_theme(original);
    }

    #[test]
    fn parses_various_json_formats() {
        // Old JSON without theme field:
        let old_json = r#"{"sort_order":"recent","sort_direction":"desc","language":null,"install_notifications":true}"#;
        let s: Settings = serde_json::from_str(old_json).unwrap();
        assert_eq!(s.theme, crate::app::ui::theme::ThemeId::Midnight);

        // JSON with PascalCase theme:
        let pascal = r#"{"theme":"Sunset"}"#;
        let s: Settings = serde_json::from_str(pascal).unwrap();
        assert_eq!(s.theme, crate::app::ui::theme::ThemeId::Sunset);

        // JSON with lowercase theme:
        let lower = r#"{"theme":"sunset"}"#;
        let s: Settings = serde_json::from_str(lower).unwrap();
        assert_eq!(s.theme, crate::app::ui::theme::ThemeId::Sunset);

        // JSON with snake_case theme:
        let snake = r#"{"theme":"high_contrast"}"#;
        let s: Settings = serde_json::from_str(snake).unwrap();
        assert_eq!(s.theme, crate::app::ui::theme::ThemeId::HighContrast);

        // JSON with unknown/invalid theme:
        let invalid = r#"{"theme":"something_weird"}"#;
        let s: Settings = serde_json::from_str(invalid).unwrap();
        assert_eq!(s.theme, crate::app::ui::theme::ThemeId::Midnight);
    }
}
