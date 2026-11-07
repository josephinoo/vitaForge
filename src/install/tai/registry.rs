//! What VitaForge knows about the plugins on this console: the ones it
//! installed and the ones it recognised in config.txt ("adopted").

use super::config::{EntryView, file_name};
use crate::data::AppEntry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const REGISTRY_FILE: &str = "ux0:data/vitaforge/plugins.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Installed from the catalog by VitaForge.
    #[default]
    Catalog,
    /// Found in config.txt and matched to a catalog entry by file name.
    Adopted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRecord {
    pub id: String,
    pub name: String,
    /// Empty when adopted: the module itself carries no version.
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: Source,
    /// Module paths, e.g. `ur0:tai/nonpdrm.skprx`.
    #[serde(default)]
    pub files: Vec<String>,
    /// `(section, path)` pairs it put in config.txt.
    #[serde(default)]
    pub entries: Vec<(String, String)>,
    #[serde(default)]
    pub installed_at: u64,
    /// Load order inside `*KERNEL`, from the catalog: 1 first, 5 last.
    #[serde(default)]
    pub kernel_priority: Option<i32>,
}

impl PluginRecord {
    pub fn owns(&self, path: &str) -> bool {
        let name = file_name(path);
        self.files.iter().any(|f| file_name(f).eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub plugins: Vec<PluginRecord>,
    /// Lines safe mode switched off, so leaving it turns back on exactly those.
    #[serde(default)]
    pub safe_mode: Option<Vec<(Option<String>, String)>>,
}

impl Registry {
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn load() -> Self {
        Self::load_from(Path::new(REGISTRY_FILE))
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = PathBuf::from(format!("{}.tmp", path.display()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?).context("couldn't write the plugin registry")?;
        std::fs::rename(&temporary, path).context("couldn't replace the plugin registry")?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(Path::new(REGISTRY_FILE))
    }

    pub fn get(&self, id: &str) -> Option<&PluginRecord> {
        self.plugins.iter().find(|p| p.id == id)
    }

    pub fn owner_of(&self, path: &str) -> Option<&PluginRecord> {
        self.plugins.iter().find(|p| p.owns(path))
    }

    pub fn upsert(&mut self, record: PluginRecord) {
        match self.plugins.iter_mut().find(|p| p.id == record.id) {
            Some(existing) => *existing = record,
            None => self.plugins.push(record),
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<PluginRecord> {
        let index = self.plugins.iter().position(|p| p.id == id)?;
        Some(self.plugins.remove(index))
    }

    /// Drops records none of whose modules are listed in config.txt any more
    /// (removed by hand or from another tool).
    pub fn prune(&mut self, entries: &[EntryView]) -> bool {
        let before = self.plugins.len();
        self.plugins.retain(|p| entries.iter().any(|e| p.owns(&e.path)));
        before != self.plugins.len()
    }

    /// Rewrites each record's `(section, path)` list to what config.txt really
    /// holds for its modules (edits made by hand or by another tool).
    pub fn repair_entries(&mut self, entries: &[EntryView]) -> usize {
        let mut repaired = 0;
        for record in &mut self.plugins {
            let actual: Vec<(String, String)> = entries
                .iter()
                .filter(|e| !e.protected && record.owns(&e.path))
                .map(|e| (e.section.clone().unwrap_or_default(), e.path.clone()))
                .collect();
            if !actual.is_empty() && actual != record.entries {
                record.entries = actual;
                repaired += 1;
            }
        }
        repaired
    }

    /// Brings the registry in line with config.txt: drops records whose lines
    /// are gone, fixes drifted ones and adopts catalog plugins found by hand.
    /// Returns the number of changes, 0 when nothing needs saving.
    pub fn reconcile(&mut self, entries: &[EntryView], catalog: &[AppEntry]) -> usize {
        let before = self.plugins.len();
        self.prune(entries);
        let dropped = before - self.plugins.len();
        dropped + self.repair_entries(entries) + self.adopt(entries, catalog)
    }

    /// Records catalog plugins that are in config.txt but that VitaForge didn't
    /// install, matched by module file name. Returns how many it added.
    pub fn adopt(&mut self, entries: &[EntryView], catalog: &[AppEntry]) -> usize {
        let mut added = 0;
        for entry in entries.iter().filter(|e| !e.protected) {
            if self.owner_of(&entry.path).is_some() {
                continue;
            }
            let Some(app) = catalog.iter().find(|app| catalog_files(app).iter().any(|f| file_name(f).eq_ignore_ascii_case(entry.file_name())))
            else {
                continue;
            };
            let files = catalog_files(app);
            let listed: Vec<(String, String)> = entries
                .iter()
                .filter(|e| files.iter().any(|f| file_name(f).eq_ignore_ascii_case(e.file_name())))
                .map(|e| (e.section.clone().unwrap_or_default(), e.path.clone()))
                .collect();
            self.upsert(PluginRecord {
                id: app.id.clone(),
                name: app.name.clone(),
                version: String::new(),
                source: Source::Adopted,
                files: listed.iter().map(|(_, p)| p.clone()).collect(),
                entries: listed,
                installed_at: 0,
                kernel_priority: app.plugin_meta.as_ref().and_then(|m| m.kernel_priority),
            });
            added += 1;
        }
        added
    }
}

/// The module paths a catalog plugin installs.
pub fn catalog_files(app: &AppEntry) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for line in app.plugin_config_line.as_deref().unwrap_or("").lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !files.iter().any(|f| f.eq_ignore_ascii_case(line)) {
            files.push(line.to_owned());
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tai::config::TaiConfig;

    fn plugin(id: &str, lines: &str) -> AppEntry {
        let mut app: AppEntry = serde_json::from_value(serde_json::json!({
            "id": id, "titleid": id.to_uppercase(), "name": id, "author": "", "description": "",
            "category": "plugin", "icon_url": null, "download_url": "", "version": "1.0",
            "size_bytes": 0, "downloads": 0, "rating": 0.0, "updated_at": "",
        }))
        .unwrap();
        app.platform = crate::data::Platform::Plugin;
        app.plugin_config_line = Some(lines.to_owned());
        app
    }

    #[test]
    fn adopts_known_modules_by_file_name() {
        let config = TaiConfig::parse("*KERNEL\nux0:tai/NoNpDrm.skprx\nur0:tai/mystery.skprx\n*main\nur0:tai/henkaku.suprx\n");
        let catalog = [plugin("nonpdrm", "ur0:tai/nonpdrm.skprx")];
        let mut registry = Registry::default();
        assert_eq!(registry.adopt(&config.entries(), &catalog), 1);
        let record = registry.get("nonpdrm").unwrap();
        assert_eq!(record.source, Source::Adopted);
        assert_eq!(record.entries, vec![("*KERNEL".to_owned(), "ux0:tai/NoNpDrm.skprx".to_owned())]);
        assert!(registry.owner_of("ur0:tai/mystery.skprx").is_none());
        assert_eq!(registry.adopt(&config.entries(), &catalog), 0);
    }

    #[test]
    fn reconcile_repairs_drops_adopts_and_is_idempotent() {
        let config = TaiConfig::parse("*KERNEL\nur0:tai/a.skprx\n*main\nur0:tai/a.skprx\nur0:tai/nonpdrm.skprx\n");
        let catalog = [plugin("nonpdrm", "ur0:tai/nonpdrm.skprx")];
        let mut registry = Registry::default();
        registry.upsert(PluginRecord {
            id: "a".into(),
            files: vec!["ur0:tai/a.skprx".into()],
            entries: vec![("*KERNEL".into(), "ur0:tai/a.skprx".into())],
            ..Default::default()
        });
        registry.upsert(PluginRecord { id: "gone".into(), files: vec!["ur0:tai/gone.suprx".into()], ..Default::default() });
        // 1 dropped + 1 repaired (a gained a *main line) + 1 adopted
        assert_eq!(registry.reconcile(&config.entries(), &catalog), 3);
        assert!(registry.get("gone").is_none());
        assert_eq!(registry.get("a").unwrap().entries.len(), 2);
        assert!(registry.get("nonpdrm").is_some());
        assert_eq!(registry.reconcile(&config.entries(), &catalog), 0, "second pass changes nothing");
    }

    #[test]
    fn reconcile_keeps_a_record_when_its_lines_are_only_disabled() {
        let config = TaiConfig::parse("*main\n# ur0:tai/a.suprx\n");
        let mut registry = Registry::default();
        registry.upsert(PluginRecord {
            id: "a".into(),
            files: vec!["ur0:tai/a.suprx".into()],
            entries: vec![("*main".into(), "ur0:tai/a.suprx".into())],
            ..Default::default()
        });
        assert_eq!(registry.reconcile(&config.entries(), &[]), 0);
        assert!(registry.get("a").is_some());
    }

    #[test]
    fn prunes_records_whose_lines_are_gone_and_round_trips() {
        let mut registry = Registry::default();
        registry.upsert(PluginRecord { id: "a".into(), files: vec!["ur0:tai/a.suprx".into()], ..Default::default() });
        registry.upsert(PluginRecord { id: "b".into(), files: vec!["ur0:tai/b.suprx".into()], ..Default::default() });
        let config = TaiConfig::parse("*main\n# ur0:tai/a.suprx\n");
        assert!(registry.prune(&config.entries()));
        assert_eq!(registry.plugins.len(), 1);

        let path = std::env::temp_dir().join(format!("vitaforge-registry-{}.json", std::process::id()));
        registry.safe_mode = Some(vec![(Some("*main".into()), "ur0:tai/a.suprx".into())]);
        registry.save_to(&path).unwrap();
        assert_eq!(Registry::load_from(&path), registry);
        let _ = std::fs::remove_file(path);
    }
}
