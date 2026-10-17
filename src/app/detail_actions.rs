use crate::data::{AppEntry, Platform};
use crate::install::installed::InstallState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailAction {
    Install,
    Update,
    Reinstall,
    Open,
    Download,
    Uninstall,
}

const PROTECTED_TITLES: [&str; 2] = ["NPXS", "VITAFORGE"];

fn valid_title(titleid: &str) -> bool {
    let id = titleid.trim();
    id.len() == 9 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn launchable(entry: &AppEntry) -> bool {
    matches!(entry.platform, Platform::Vita | Platform::NpsVita)
        && valid_title(&entry.titleid)
        && !crate::install::is_self(entry)
}

pub fn removable(entry: &AppEntry) -> bool {
    let id = entry.titleid.trim().to_uppercase();
    entry.platform == Platform::Vita
        && valid_title(&id)
        && !PROTECTED_TITLES.iter().any(|protected| id.starts_with(protected))
}

/// ✕: the main thing the page wants done.
pub fn primary(entry: &AppEntry, state: InstallState) -> DetailAction {
    match (entry.platform, state) {
        (Platform::Plugin, _) => DetailAction::Download,
        (_, InstallState::Absent) => DetailAction::Install,
        (_, InstallState::Outdated) => DetailAction::Update,
        (_, InstallState::Installed) if launchable(entry) => DetailAction::Open,
        (_, InstallState::Installed) => DetailAction::Reinstall,
    }
}

/// △: the other way to act on an app that is already on the Vita.
pub fn secondary(entry: &AppEntry, state: InstallState) -> Option<DetailAction> {
    match state {
        InstallState::Absent => None,
        InstallState::Installed if launchable(entry) && entry.platform != Platform::Plugin => {
            Some(DetailAction::Reinstall)
        }
        InstallState::Outdated if launchable(entry) => Some(DetailAction::Open),
        _ => None,
    }
}

/// □: delete it.
pub fn tertiary(entry: &AppEntry, state: InstallState) -> Option<DetailAction> {
    (state != InstallState::Absent && removable(entry)).then_some(DetailAction::Uninstall)
}

/// Apps that "update all" may touch: homebrew and plugins, never the store itself.
pub fn removable_or_plugin(entry: &AppEntry) -> bool {
    entry.platform == Platform::Plugin || removable(entry)
}
