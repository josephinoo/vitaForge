use crate::data::{SortDirection, SortOrder};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "vita")]
const SETTINGS_PATH: &str = "ux0:data/vitaforge/settings.json";
#[cfg(not(target_os = "vita"))]
const SETTINGS_PATH: &str = "data/vitaforge/settings.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub sort_order: SortOrder,
    #[serde(default)]
    pub sort_direction: SortDirection,
}

pub fn load() -> Settings {
    std::fs::read(SETTINGS_PATH).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub fn save(settings: &Settings) {
    if let Some(parent) = std::path::Path::new(SETTINGS_PATH).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(settings) {
        let _ = std::fs::write(SETTINGS_PATH, bytes);
    }
}
