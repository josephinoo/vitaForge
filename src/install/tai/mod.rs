//! taiHEN plugin management: the config.txt model, safe writes with backups,
//! and the registry of plugins VitaForge knows about.

pub mod config;
pub mod registry;
pub mod profile;
pub mod store;
pub mod verify;

use crate::data::AppEntry;
use anyhow::{Result, bail};
use config::{EntryView, TaiConfig, file_name};
use registry::{PluginRecord, Registry, Source};
use std::path::{Path, PathBuf};
use store::{Backup, Store};

/// Everything the Settings › Plugins page shows, read in one go.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub path: PathBuf,
    pub both_exist: bool,
    pub config: TaiConfig,
    pub entries: Vec<EntryView>,
    pub registry: Registry,
    pub backups: Vec<Backup>,
    pub profiles: Vec<profile::Profile>,
    /// Profile files in the export folder that aren't saved yet.
    pub importable: Vec<(PathBuf, profile::Profile)>,
    /// Same order as `entries`: the module file isn't on the memory card.
    pub missing: Vec<bool>,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn read(store: &Store, catalog: &[AppEntry]) -> Self {
        let (config, error) = match store.load() {
            Ok(config) => (config, None),
            Err(err) => (TaiConfig::parse(""), Some(format!("{err:#}"))),
        };
        let entries = config.entries();
        let mut registry = Registry::load();
        let changes = registry.reconcile(&entries, catalog);
        if changes > 0 {
            super::log_file(&format!("plugin registry reconciled with config.txt: {changes} change(s)"));
            if let Err(err) = registry.save() {
                super::log_file(&format!("plugin registry save failed: {err:#}"));
            }
        }
        let missing = entries.iter().map(|e| !Path::new(&e.path).exists()).collect();
        let profiles = profile::list(Path::new(profile::PROFILE_DIR));
        let importable = profile::importable(Path::new(profile::EXPORT_DIR), &profiles);
        Self {
            path: store.active_path().to_path_buf(),
            both_exist: store.both_exist(),
            config,
            entries,
            registry,
            backups: store.list_backups(),
            profiles,
            importable,
            missing,
            error,
        }
    }

    /// Shows an edited copy (a reorder not yet saved) in place of the file's text.
    pub fn set_config(&mut self, config: TaiConfig) {
        self.entries = config.entries();
        self.missing = self.entries.iter().map(|e| !Path::new(&e.path).exists()).collect();
        self.config = config;
    }

    /// Display name: the catalog plugin that owns the line, else its file name.
    pub fn label(&self, entry: &EntryView) -> String {
        self.registry.owner_of(&entry.path).map_or_else(|| entry.file_name().to_owned(), |r| r.name.clone())
    }

    pub fn plugin_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.protected).count()
    }

    pub fn safe_mode(&self) -> bool {
        self.registry.safe_mode.is_some()
    }
}

/// Catalog id of every plugin in the registry whose catalog version is newer.
pub fn outdated(registry: &Registry, catalog: &[AppEntry]) -> Vec<String> {
    registry
        .plugins
        .iter()
        .filter(|record| record.source == Source::Catalog && !record.version.is_empty())
        .filter(|record| {
            catalog.iter().any(|app| {
                app.id == record.id && crate::install::installed::version_is_older(&record.version, &app.version)
            })
        })
        .map(|record| record.id.clone())
        .collect()
}

/// Enabled lines that clash with `app`: its `conflicts_with` names a module
/// file or catalog id, or it `replaces` the plugin that owns the line.
pub fn conflicts(app: &AppEntry, config: &TaiConfig, registry: &Registry) -> Vec<EntryView> {
    let Some(meta) = app.plugin_meta.as_deref() else { return Vec::new() };
    let own = registry::catalog_files(app);
    config
        .entries()
        .into_iter()
        .filter(|e| e.enabled && !e.protected)
        .filter(|e| !own.iter().any(|f| file_name(f).eq_ignore_ascii_case(e.file_name())))
        .filter(|e| meta.conflicts_with.iter().chain(&meta.replaces).any(|name| names_entry(name, e, registry)))
        .collect()
}

/// `name` is a module file name, a file stem or the catalog id of the line's owner.
fn names_entry(name: &str, entry: &EntryView, registry: &Registry) -> bool {
    let owner = registry.owner_of(&entry.path).map(|r| r.id.as_str());
    name.eq_ignore_ascii_case(entry.file_name())
        || owner.is_some_and(|id| id.eq_ignore_ascii_case(name))
        || file_name(&entry.path).rsplit_once('.').is_some_and(|(stem, _)| stem.eq_ignore_ascii_case(name))
}

/// What `app` needs (`requires`) that no enabled line provides. Warn-only.
pub fn missing_requirements(app: &AppEntry, config: &TaiConfig, registry: &Registry) -> Vec<String> {
    let Some(meta) = app.plugin_meta.as_deref() else { return Vec::new() };
    let entries: Vec<EntryView> = config.entries().into_iter().filter(|e| e.enabled).collect();
    meta.requires
        .iter()
        .filter(|need| !entries.iter().any(|e| names_entry(need, e, registry)))
        .cloned()
        .collect()
}

/// Names of installed catalog plugins that `requires` the plugin `id`; shown
/// before disabling or removing it.
pub fn dependents_of(id: &str, registry: &Registry, catalog: &[AppEntry]) -> Vec<String> {
    let Some(target) = registry.get(id) else { return Vec::new() };
    catalog
        .iter()
        .filter(|app| app.id != id && registry.get(&app.id).is_some())
        .filter(|app| {
            app.plugin_meta.as_deref().is_some_and(|meta| {
                meta.requires.iter().any(|need| {
                    need.eq_ignore_ascii_case(id)
                        || target.files.iter().any(|f| {
                            let name = file_name(f);
                            name.eq_ignore_ascii_case(need)
                                || name.rsplit_once('.').is_some_and(|(stem, _)| stem.eq_ignore_ascii_case(need))
                        })
                })
            })
        })
        .map(|app| app.name.clone())
        .collect()
}

/// Adds a freshly copied plugin to config.txt and records it. Lines that
/// `conflicts` reported are switched off in the same write.
pub fn activate(store: &Store, app: &AppEntry, lines: &[(String, String)], files: &[PathBuf]) -> Result<bool> {
    let mut registry = Registry::load();
    let priority = app.plugin_meta.as_ref().and_then(|m| m.kernel_priority);
    let (disabled, changed) = store.edit(&format!("install-{}", app.id), |config| {
        let clashing = conflicts(app, config, &registry);
        for entry in &clashing {
            config.set_enabled(entry.line, false)?;
        }
        for (section, path) in lines {
            let later = |other: &str| {
                section == "*KERNEL"
                    && priority.is_some_and(|mine| {
                        registry.owner_of(other).and_then(|r| r.kernel_priority).is_some_and(|theirs| theirs > mine)
                    })
            };
            config.add(section, path, later);
        }
        Ok(clashing)
    })?;
    for entry in &disabled {
        super::log_file(&format!("Disabled conflicting plugin {} for {}", entry.path, app.name));
    }
    registry.upsert(PluginRecord {
        id: app.id.clone(),
        name: app.name.clone(),
        version: app.version.clone(),
        source: Source::Catalog,
        files: files.iter().map(|f| f.display().to_string()).collect(),
        entries: lines.to_vec(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        kernel_priority: priority,
    });
    registry.save()?;
    Ok(changed)
}

/// Takes a plugin's lines out of config.txt and deletes the module files no
/// other line still loads.
pub fn uninstall(store: &Store, app: &AppEntry) -> Result<()> {
    let mut registry = Registry::load();
    let files: Vec<String> = match registry.get(&app.id) {
        Some(record) => record.files.clone(),
        None => registry::catalog_files(app),
    };
    if files.is_empty() {
        bail!("VitaForge doesn't know which files this plugin uses");
    }
    let owns = |path: &str| files.iter().any(|f| file_name(f).eq_ignore_ascii_case(file_name(path)));
    let (remaining, _) = store.edit(&format!("uninstall-{}", app.id), |config| {
        let mut lines: Vec<usize> =
            config.entries().into_iter().filter(|e| !e.protected && owns(&e.path)).map(|e| e.line).collect();
        lines.sort_unstable_by(|a, b| b.cmp(a));
        for line in lines {
            config.remove(line)?;
        }
        Ok(config.entries())
    })?;
    for file in &files {
        let still_used = remaining.iter().any(|e| e.path.eq_ignore_ascii_case(file));
        let in_tai = file.starts_with("ur0:tai/") || file.starts_with("ux0:tai/");
        if !still_used && in_tai && !file.contains("..") && !config::is_protected(file) {
            let _ = std::fs::remove_file(file);
        }
    }
    registry.remove(&app.id);
    registry.save()?;
    super::log_file(&format!("Uninstalled plugin '{}'", app.name));
    Ok(())
}

pub fn set_enabled(store: &Store, line: usize, enabled: bool) -> Result<bool> {
    let reason = if enabled { "enable" } else { "disable" };
    Ok(store.edit(reason, |config| config.set_enabled(line, enabled))?.1)
}

pub fn remove(store: &Store, line: usize) -> Result<()> {
    store.edit("remove", |config| config.remove(line))?;
    Ok(())
}

/// Safe mode: every plugin but HENkaku's own is commented out; leaving it turns
/// back on exactly the lines it switched off.
pub fn set_safe_mode(store: &Store, on: bool) -> Result<()> {
    let mut registry = Registry::load();
    if on {
        if registry.safe_mode.is_some() {
            return Ok(());
        }
        let (disabled, _) = store.edit("safe-mode", |config| Ok(config.disable_all()))?;
        registry.safe_mode = Some(disabled);
    } else {
        let Some(lines) = registry.safe_mode.take() else { return Ok(()) };
        store.edit("safe-mode-off", |config| Ok(config.restore_enabled(&lines)))?;
    }
    registry.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(id: &str, lines: &str, conflicts: &[&str]) -> AppEntry {
        let mut app: AppEntry = serde_json::from_value(serde_json::json!({
            "id": id, "titleid": id.to_uppercase(), "name": id, "author": "", "description": "",
            "category": "plugin", "icon_url": null, "download_url": "", "version": "1.0",
            "size_bytes": 0, "downloads": 0, "rating": 0.0, "updated_at": "",
        }))
        .unwrap();
        app.platform = crate::data::Platform::Plugin;
        app.plugin_config_line = Some(lines.to_owned());
        app.plugin_meta = Some(Box::new(crate::data::PluginMeta {
            conflicts_with: conflicts.iter().map(|c| (*c).to_owned()).collect(),
            ..Default::default()
        }));
        app
    }

    #[test]
    fn finds_conflicts_by_file_name_stem_or_owner() {
        let config = TaiConfig::parse("*KERNEL\nur0:tai/refood.skprx\n# ur0:tai/other.skprx\nur0:tai/0syscall6.skprx\n");
        let app = plugin("0syscall6", "ur0:tai/0syscall6.skprx", &["refood.skprx", "other"]);
        let found = conflicts(&app, &config, &Registry::default());
        assert_eq!(found.len(), 1, "disabled lines and the plugin itself don't count");
        assert_eq!(found[0].path, "ur0:tai/refood.skprx");

        let mut registry = Registry::default();
        registry.upsert(PluginRecord { id: "refood".into(), files: vec!["ur0:tai/refood.skprx".into()], ..Default::default() });
        let by_id = plugin("x", "ur0:tai/x.skprx", &["REFOOD"]);
        assert_eq!(conflicts(&by_id, &config, &registry).len(), 1);
    }

    fn needing(id: &str, requires: &[&str]) -> AppEntry {
        let mut app = plugin(id, &format!("ur0:tai/{id}.suprx"), &[]);
        app.plugin_meta.as_mut().unwrap().requires = requires.iter().map(|r| (*r).to_owned()).collect();
        app
    }

    #[test]
    fn reports_requirements_nothing_enabled_provides() {
        let config = TaiConfig::parse("*KERNEL\nur0:tai/kubridge.skprx\n# ur0:tai/fd_fix.skprx\n");
        let app = needing("port", &["kubridge", "fd_fix.skprx", "libshacccg"]);
        assert_eq!(missing_requirements(&app, &config, &Registry::default()), ["fd_fix.skprx", "libshacccg"]);
        assert!(missing_requirements(&needing("x", &[]), &config, &Registry::default()).is_empty());
        let mut bare = needing("y", &["kubridge"]);
        bare.plugin_meta = None;
        assert!(missing_requirements(&bare, &config, &Registry::default()).is_empty());
        let mut registry = Registry::default();
        registry.upsert(PluginRecord { id: "fdfix".into(), files: vec!["ur0:tai/fd_fix.skprx".into()], ..Default::default() });
        let enabled = TaiConfig::parse("*KERNEL\nur0:tai/fd_fix.skprx\n");
        assert!(missing_requirements(&needing("z", &["fdfix"]), &enabled, &registry).is_empty(), "by owner id");
    }

    #[test]
    fn lists_installed_plugins_that_depend_on_one() {
        let mut registry = Registry::default();
        registry.upsert(PluginRecord { id: "kubridge".into(), files: vec!["ur0:tai/kubridge.skprx".into()], ..Default::default() });
        registry.upsert(PluginRecord { id: "port".into(), files: vec!["ur0:tai/port.suprx".into()], ..Default::default() });
        let catalog = [needing("port", &["kubridge"]), needing("other", &["kubridge.skprx"]), needing("kubridge", &[])];
        assert_eq!(dependents_of("kubridge", &registry, &catalog), ["port"], "'other' isn't installed");
        assert!(dependents_of("port", &registry, &catalog).is_empty());
        assert!(dependents_of("unknown", &registry, &catalog).is_empty());
    }

    #[test]
    fn plugins_without_catalog_hints_never_conflict() {
        let config = TaiConfig::parse("*KERNEL\nur0:tai/a.skprx\n");
        let mut app = plugin("b", "ur0:tai/b.skprx", &[]);
        app.plugin_meta = None;
        assert!(conflicts(&app, &config, &Registry::default()).is_empty());
    }
}
