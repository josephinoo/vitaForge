use super::installed::vita_fs;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReqStatus {
    Found,
    Disabled,
    Unconfigured,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReqItem {
    pub text: String,
    pub status: ReqStatus,
}

const DEVICES: [&str; 4] = ["ux0:", "ur0:", "uma0:", "imc0:"];

pub fn split_lines(requirements: &str) -> Vec<String> {
    requirements
        .lines()
        .map(|line| line.trim().trim_start_matches(['-', '*']).trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_line(line: &str, titleid: &str) -> Option<(String, String)> {
    let at = line.rfind(" in ")?;
    let item = line[..at].trim();
    let path = line[at + 4..].trim_start();
    if !DEVICES.iter().any(|device| path.starts_with(device)) {
        return None;
    }
    let end = path
        .find(|c: char| c.is_whitespace() || c == ',')
        .unwrap_or(path.len());
    let dir = path[..end].trim_end_matches(['.', '/']);
    if dir.is_empty() || item.is_empty() {
        return None;
    }
    let own = format!("ux0:app/{titleid}");
    if !titleid.is_empty() && dir.starts_with(&own) {
        return None;
    }
    Some((item.to_owned(), dir.to_owned()))
}

pub fn check_line(line: &str, titleid: &str) -> ReqStatus {
    let Some((item, dir)) = parse_line(line, titleid) else {
        return ReqStatus::Unknown;
    };
    let found = if !item.contains(' ') && item.contains('.') {
        vita_fs::exists(&format!("{dir}/{item}"))
    } else {
        vita_fs::list_dir(&dir).is_ok_and(|names| !names.is_empty())
    };
    if !found {
        return ReqStatus::Missing;
    }
    let is_tai = dir.starts_with("ur0:tai") || dir.starts_with("ux0:tai");
    let is_plugin = item.ends_with(".skprx") || item.ends_with(".suprx");
    if is_tai && is_plugin {
        if let Ok(config) = super::tai::TaiConfig::load_active() {
            if let Some((_sec, tai_item)) = config.find_plugin(&item) {
                match tai_item {
                    super::tai::TaiItem::Plugin { enabled: true, .. } => ReqStatus::Found,
                    super::tai::TaiItem::Plugin { enabled: false, .. } => ReqStatus::Disabled,
                    _ => ReqStatus::Found,
                }
            } else {
                ReqStatus::Unconfigured
            }
        } else {
            ReqStatus::Found
        }
    } else {
        ReqStatus::Found
    }
}

#[derive(Default)]
struct Cache {
    done: HashMap<String, Arc<Vec<ReqItem>>>,
    pending: HashSet<String>,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

fn unchecked(requirements: &str) -> Vec<ReqItem> {
    split_lines(requirements)
        .into_iter()
        .map(|text| ReqItem { text, status: ReqStatus::Unknown })
        .collect()
}

/// Requirement lines for the detail page. Disk checks run on a blocking worker;
/// until they land every line reads `Unknown`, so the paint path never touches I/O.
pub fn items(ctx: &egui::Context, app_id: &str, titleid: &str, requirements: &str) -> Arc<Vec<ReqItem>> {
    {
        let mut cache = cache().lock().unwrap();
        if let Some(done) = cache.done.get(app_id) {
            return done.clone();
        }
        if !cache.pending.insert(app_id.to_owned()) {
            return Arc::new(unchecked(requirements));
        }
    }
    let placeholder = Arc::new(unchecked(requirements));
    let (app_id, titleid, requirements) = (app_id.to_owned(), titleid.to_owned(), requirements.to_owned());
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || {
        let checked: Vec<ReqItem> = split_lines(&requirements)
            .into_iter()
            .map(|text| {
                let status = check_line(&text, &titleid);
                ReqItem { text, status }
            })
            .collect();
        let mut cache = cache().lock().unwrap();
        cache.pending.remove(&app_id);
        cache.done.insert(app_id, Arc::new(checked));
        ctx.request_repaint();
    });
    placeholder
}

/// Anything on disk may have changed (an install, a plugin, a delete), so every
/// cached check is dropped and redone the next time its page is open.
pub fn invalidate_all() {
    cache().lock().unwrap().done.clear();
}

fn plain_file_name(line: &str) -> Option<String> {
    let at = line.rfind(" in ")?;
    let item = line[..at].trim();
    (!item.contains(' ') && item.contains('.')).then(|| item.to_lowercase())
}

fn url_file(url: &str) -> String {
    url.split('?')
        .next()
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or("")
        .to_lowercase()
}

/// A catalog plugin that provides a missing file, and where the file belongs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub name: String,
    pub dir: String,
    pub file: String,
}

/// The catalog plugin that provides a missing file such as `kubridge.skprx`.
/// Only offered for files that belong in a taiHEN folder, the one place the
/// installer can also register them.
pub fn dependency(apps: &[crate::data::AppEntry], line: &str) -> Option<Dependency> {
    static FOUND: OnceLock<Mutex<HashMap<String, (String, String)>>> = OnceLock::new();
    let wanted = plain_file_name(line)?;
    let (file, dir) = parse_line(line, "")?;
    if !(dir.starts_with("ur0:tai") || dir.starts_with("ux0:tai")) {
        return None;
    }
    let found = FOUND.get_or_init(Default::default);
    let cached = found.lock().unwrap().get(&wanted).cloned();
    let (id, name) = match cached {
        Some(hit) => hit,
        None => {
            let hit = apps.iter().find(|app| {
                app.platform == crate::data::Platform::Plugin
                    && (url_file(&app.download_url) == wanted
                        || app
                            .plugin_config_line
                            .as_deref()
                            .is_some_and(|config| config.trim().to_lowercase().ends_with(&wanted)))
            })?;
            let pair = (hit.id.clone(), hit.name.clone());
            found.lock().unwrap().insert(wanted, pair.clone());
            pair
        }
    };
    Some(Dependency { id, name, dir, file })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ready,
    NeedsFiles,
    Plain,
}

pub fn verdict(items: &[ReqItem], installed: bool) -> Verdict {
    if !installed {
        return Verdict::Plain;
    }
    if items.iter().any(|item| {
        item.status == ReqStatus::Missing
            || item.status == ReqStatus::Disabled
            || item.status == ReqStatus::Unconfigured
    }) {
        Verdict::NeedsFiles
    } else if items.iter().any(|item| item.status == ReqStatus::Found) {
        Verdict::Ready
    } else {
        Verdict::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_bullets() {
        assert_eq!(
            split_lines("- a in ux0:data\n- b in ur0:tai\n\n"),
            vec!["a in ux0:data", "b in ur0:tai"]
        );
    }

    #[test]
    fn parses_file_and_folder_lines() {
        assert_eq!(
            parse_line("kubridge.skprx in ur0:tai", "X"),
            Some(("kubridge.skprx".into(), "ur0:tai".into()))
        );
        assert_eq!(
            parse_line("Game Data Files: PC Steam in ux0:data/Mania.", "X"),
            Some(("Game Data Files: PC Steam".into(), "ux0:data/Mania".into()))
        );
    }

    #[test]
    fn unchecked_lines_are_unknown() {
        assert_eq!(parse_line("libshacccg.suprx", "X"), None);
        assert_eq!(parse_line("data in somewhere", "X"), None);
        assert_eq!(parse_line("cfg in ux0:app/ABCD12345/data", "ABCD12345"), None);
    }

    #[test]
    fn plain_files_only() {
        assert_eq!(plain_file_name("kubridge.skprx in ur0:tai"), Some("kubridge.skprx".into()));
        assert_eq!(plain_file_name("Game Data Files: PC in ux0:data"), None);
        assert_eq!(url_file("https://x/y/KuBridge.skprx?dl=1"), "kubridge.skprx");
    }

    #[test]
    fn verdicts() {
        let found = ReqItem { text: "a".into(), status: ReqStatus::Found };
        let missing = ReqItem { text: "b".into(), status: ReqStatus::Missing };
        assert!(verdict(&[found.clone()], true) == Verdict::Ready);
        assert!(verdict(&[found.clone(), missing], true) == Verdict::NeedsFiles);
        assert!(verdict(&[found], false) == Verdict::Plain);
    }
}
