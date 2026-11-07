//! Named sets of enabled plugins, saved as small JSON files that can be applied
//! later or moved to another console. Applying goes through `Store::edit`, so
//! every switch is backed up like any other config.txt change.

use super::config::{TaiConfig, is_protected};
use super::store::Store;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PROFILE_DIR: &str = "ux0:data/vitaforge/profiles";
pub const EXPORT_DIR: &str = "ux0:data/vitaforge/export";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub section: Option<String>,
    pub path: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub entries: Vec<ProfileEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Lines whose state changed in config.txt.
    pub changed: usize,
    /// Modules the profile wants on but that aren't on this console; left off.
    pub missing: Vec<String>,
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// File-safe version of a profile name; errors when nothing usable is left.
pub fn slug(name: &str) -> Result<String> {
    let slug: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .take(32)
        .collect();
    if slug.trim_matches('-').is_empty() {
        bail!("the profile needs a name");
    }
    Ok(slug)
}

/// The current plugins and whether each is on. HENkaku's own module is left out.
pub fn capture(name: &str, config: &TaiConfig) -> Profile {
    Profile {
        name: name.trim().to_owned(),
        entries: config
            .entries()
            .into_iter()
            .filter(|e| !e.protected)
            .map(|e| ProfileEntry { section: e.section, path: e.path, enabled: e.enabled })
            .collect(),
    }
}

/// Makes config.txt match `profile`: listed plugins take the saved state, plugins
/// it doesn't mention are switched off, HENkaku is never touched. `exists` says
/// whether a module file is on this console.
pub fn apply(store: &Store, profile: &Profile, exists: impl Fn(&str) -> bool) -> Result<ApplyReport> {
    let reason = format!("profile-{}", slug(&profile.name).unwrap_or_default());
    let (report, _) = store.edit(&reason, |config| {
        let mut report = ApplyReport::default();
        for entry in config.entries().into_iter().filter(|e| !e.protected) {
            let wanted = profile
                .entries
                .iter()
                .find(|p| p.section == entry.section && same(&p.path, &entry.path))
                .is_some_and(|p| p.enabled);
            let wanted = wanted && (entry.enabled || exists(&entry.path) || {
                report.missing.push(entry.path.clone());
                false
            });
            if config.set_enabled(entry.line, wanted)? {
                report.changed += 1;
            }
        }
        for p in profile.entries.iter().filter(|p| p.enabled) {
            let Some(section) = p.section.as_deref() else { continue };
            if is_protected(&p.path) || config.find(section, &p.path).is_some() {
                continue;
            }
            if exists(&p.path) {
                config.add(section, &p.path, |_| false);
                report.changed += 1;
            } else {
                report.missing.push(p.path.clone());
            }
        }
        Ok(report)
    })?;
    Ok(report)
}

fn file_in(dir: &Path, name: &str) -> Result<PathBuf> {
    Ok(dir.join(format!("{}.json", slug(name)?)))
}

pub fn save(dir: &Path, profile: &Profile) -> Result<()> {
    std::fs::create_dir_all(dir).context("couldn't create the profiles folder")?;
    let path = file_in(dir, &profile.name)?;
    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&temporary, serde_json::to_vec_pretty(profile)?).context("couldn't save the profile")?;
    std::fs::rename(&temporary, &path).context("couldn't save the profile")
}

pub fn load(path: &Path) -> Result<Profile> {
    let profile: Profile =
        serde_json::from_slice(&std::fs::read(path).context("couldn't read the profile")?).context("that file isn't a VitaForge profile")?;
    slug(&profile.name)?;
    if profile.entries.iter().any(|e| e.path.contains(['\r', '\n']) || e.path.split(['/', '\\']).any(|p| p == "..")) {
        bail!("that profile has an unsafe module path");
    }
    Ok(profile)
}

/// Saved profiles, by name.
pub fn list(dir: &Path) -> Vec<Profile> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut profiles: Vec<Profile> =
        read.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")).filter_map(|e| load(&e.path()).ok()).collect();
    profiles.sort_by_key(|p| p.name.to_ascii_lowercase());
    profiles
}

/// Profile files found in `dir` that aren't saved under the same name in `saved`.
pub fn importable(dir: &Path, saved: &[Profile]) -> Vec<(PathBuf, Profile)> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(PathBuf, Profile)> = read
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| load(&e.path()).ok().map(|p| (e.path(), p)))
        .filter(|(_, p)| !saved.iter().any(|s| s.name == p.name))
        .collect();
    found.sort_by_key(|(_, p)| p.name.to_ascii_lowercase());
    found
}

/// Saves `profile` under `new_name` and removes the old file; refuses a name taken by another profile.
pub fn rename(dir: &Path, old_name: &str, new_name: &str) -> Result<()> {
    let new_name = new_name.trim();
    let (old_file, new_file) = (file_in(dir, old_name)?, file_in(dir, new_name)?);
    // The memory card is case-insensitive: "Racing" and "racing" are one file.
    let same_file = old_file.to_string_lossy().eq_ignore_ascii_case(&new_file.to_string_lossy());
    if !same_file && new_file.exists() {
        bail!("a profile with that name already exists");
    }
    let mut profile = load(&old_file)?;
    profile.name = new_name.to_owned();
    if !same_file {
        save(dir, &profile)?;
        let _ = std::fs::remove_file(old_file);
    } else {
        let _ = std::fs::remove_file(&old_file);
        save(dir, &profile)?;
    }
    Ok(())
}

pub fn delete(dir: &Path, name: &str) -> Result<()> {
    std::fs::remove_file(file_in(dir, name)?).context("couldn't delete the profile")
}

/// Copies a saved profile to `export_dir` for moving to another console.
pub fn export(dir: &Path, name: &str, export_dir: &Path) -> Result<PathBuf> {
    let source = file_in(dir, name)?;
    load(&source)?;
    std::fs::create_dir_all(export_dir).context("couldn't create the export folder")?;
    let target = export_dir.join(source.file_name().unwrap_or_default());
    std::fs::copy(&source, &target).context("couldn't export the profile")?;
    Ok(target)
}

/// Validates a profile file and saves it among the profiles.
pub fn import(file: &Path, dir: &Path) -> Result<Profile> {
    let profile = load(file)?;
    save(dir, &profile)?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tai::store::temp_store;

    const CONFIG: &str = "*KERNEL\nur0:tai/kubridge.skprx\n*main\nur0:tai/henkaku.suprx\nur0:tai/a.suprx\n# ur0:tai/b.suprx\n";

    fn all_present(_: &str) -> bool {
        true
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vitaforge-profile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn capture_skips_henkaku_and_keeps_state() {
        let profile = capture(" Gaming ", &TaiConfig::parse(CONFIG));
        assert_eq!(profile.name, "Gaming");
        let paths: Vec<_> = profile.entries.iter().map(|e| (e.path.as_str(), e.enabled)).collect();
        assert_eq!(paths, [("ur0:tai/kubridge.skprx", true), ("ur0:tai/a.suprx", true), ("ur0:tai/b.suprx", false)]);
    }

    #[test]
    fn apply_round_trips_and_backs_up() {
        let store = temp_store("profile-apply");
        std::fs::write(&store.ur0, CONFIG).unwrap();
        let profile = capture("base", &TaiConfig::parse(CONFIG));
        store.edit("mess", |c| {
            let lines: Vec<_> = c.entries().into_iter().filter(|e| !e.protected).map(|e| (e.line, e.enabled)).collect();
            for (line, enabled) in lines {
                c.set_enabled(line, !enabled)?;
            }
            Ok(())
        }).unwrap();
        let report = apply(&store, &profile, all_present).unwrap();
        assert_eq!(report.changed, 3);
        assert!(report.missing.is_empty());
        assert_eq!(std::fs::read_to_string(&store.ur0).unwrap(), CONFIG);
        assert!(store.list_backups().iter().any(|b| b.reason == "profile-base"));
        assert_eq!(apply(&store, &profile, all_present).unwrap(), ApplyReport::default(), "idempotent");
    }

    #[test]
    fn apply_disables_unlisted_plugins_but_never_henkaku() {
        let store = temp_store("profile-unlisted");
        std::fs::write(&store.ur0, CONFIG).unwrap();
        let only_kubridge = Profile {
            name: "min".into(),
            entries: vec![ProfileEntry { section: Some("*KERNEL".into()), path: "ur0:tai/kubridge.skprx".into(), enabled: true }],
        };
        apply(&store, &only_kubridge, all_present).unwrap();
        assert_eq!(
            std::fs::read_to_string(&store.ur0).unwrap(),
            "*KERNEL\nur0:tai/kubridge.skprx\n*main\nur0:tai/henkaku.suprx\n# ur0:tai/a.suprx\n# ur0:tai/b.suprx\n"
        );
    }

    #[test]
    fn missing_modules_stay_off_and_are_reported() {
        let store = temp_store("profile-missing");
        std::fs::write(&store.ur0, CONFIG).unwrap();
        let profile = Profile {
            name: "p".into(),
            entries: vec![
                ProfileEntry { section: Some("*main".into()), path: "ur0:tai/b.suprx".into(), enabled: true },
                ProfileEntry { section: Some("*main".into()), path: "ur0:tai/new.suprx".into(), enabled: true },
                ProfileEntry { section: Some("*main".into()), path: "ur0:tai/henkaku.suprx".into(), enabled: false },
            ],
        };
        let report = apply(&store, &profile, |p| !p.ends_with("b.suprx") && !p.ends_with("new.suprx")).unwrap();
        assert_eq!(report.missing, ["ur0:tai/b.suprx", "ur0:tai/new.suprx"]);
        let text = std::fs::read_to_string(&store.ur0).unwrap();
        assert!(text.contains("# ur0:tai/b.suprx") && !text.contains("new.suprx"));
        assert!(text.contains("\nur0:tai/henkaku.suprx\n"), "HENkaku untouched");
    }

    #[test]
    fn adds_lines_the_config_lacks_when_the_module_exists() {
        let store = temp_store("profile-add");
        std::fs::write(&store.ur0, "*main\nur0:tai/a.suprx\n").unwrap();
        let profile = Profile {
            name: "p".into(),
            entries: vec![
                ProfileEntry { section: Some("*main".into()), path: "ur0:tai/a.suprx".into(), enabled: true },
                ProfileEntry { section: Some("*main".into()), path: "ur0:tai/c.suprx".into(), enabled: true },
            ],
        };
        assert_eq!(apply(&store, &profile, all_present).unwrap().changed, 1);
        assert_eq!(std::fs::read_to_string(&store.ur0).unwrap(), "*main\nur0:tai/a.suprx\nur0:tai/c.suprx\n");
    }

    #[test]
    fn save_list_delete_export_import() {
        let (profiles, exported, other) = (dir("a"), dir("b"), dir("c"));
        let profile = capture("My Set!", &TaiConfig::parse(CONFIG));
        save(&profiles, &profile).unwrap();
        save(&profiles, &capture("alpha", &TaiConfig::parse(CONFIG))).unwrap();
        assert_eq!(list(&profiles).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["alpha", "My Set!"]);
        let file = export(&profiles, "My Set!", &exported).unwrap();
        assert_eq!(import(&file, &other).unwrap(), profile);
        assert_eq!(list(&other), [profile]);
        delete(&profiles, "My Set!").unwrap();
        assert_eq!(list(&profiles).len(), 1);
        assert!(delete(&profiles, "My Set!").is_err());
        for d in [profiles, exported, other] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn rename_moves_the_file_and_refuses_duplicates() {
        let d = dir("rename");
        save(&d, &capture("one", &TaiConfig::parse(CONFIG))).unwrap();
        save(&d, &capture("two", &TaiConfig::parse(CONFIG))).unwrap();
        assert!(rename(&d, "one", "two").unwrap_err().to_string().contains("already exists"));
        rename(&d, "one", " Racing ").unwrap();
        assert_eq!(list(&d).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Racing", "two"]);
        rename(&d, "Racing", "racing").unwrap();
        assert_eq!(list(&d).len(), 2, "same file name is just a rewrite");
        assert_eq!(list(&d)[0].name, "racing");
        assert!(rename(&d, "ghost", "x").is_err(), "unknown profile");
        assert!(rename(&d, "two", "   ").is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn importable_skips_profiles_already_saved() {
        let (saved, shared) = (dir("imp-a"), dir("imp-b"));
        let a = capture("alpha", &TaiConfig::parse(CONFIG));
        save(&saved, &a).unwrap();
        save(&shared, &a).unwrap();
        save(&shared, &capture("beta", &TaiConfig::parse(CONFIG))).unwrap();
        let found = importable(&shared, &list(&saved));
        assert_eq!(found.iter().map(|(_, p)| p.name.as_str()).collect::<Vec<_>>(), ["beta"]);
        assert!(importable(&dir("nope"), &[]).is_empty());
        for d in [saved, shared] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn rejects_bad_names_and_bad_files() {
        assert!(slug("").is_err() && slug("  ").is_err() && slug("///").is_err());
        assert_eq!(slug("a b/c").unwrap(), "a-b-c");
        let d = dir("bad");
        std::fs::create_dir_all(&d).unwrap();
        let junk = d.join("junk.json");
        std::fs::write(&junk, "not json").unwrap();
        assert!(load(&junk).is_err());
        let evil = d.join("evil.json");
        std::fs::write(&evil, r#"{"name":"x","entries":[{"section":"*main","path":"ur0:tai/../../x.suprx","enabled":true}]}"#).unwrap();
        assert!(load(&evil).unwrap_err().to_string().contains("unsafe"));
        assert!(load(&d.join("none.json")).is_err());
        assert!(list(&d.join("nope")).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
