use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub section: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn errors(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error).collect()
    }

    pub fn warnings(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Warning).collect()
    }

    pub fn add_error(&mut self, section: Option<String>, path: Option<String>, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Error,
            section,
            path,
            message: msg.into(),
        });
    }

    pub fn add_warning(&mut self, section: Option<String>, path: Option<String>, msg: impl Into<String>) {
        self.issues.push(ValidationIssue {
            severity: Severity::Warning,
            section,
            path,
            message: msg.into(),
        });
    }
}

/// A line item inside a taiHEN section or the preamble.
/// Preserves comments, blanks, and unknown directives so nothing is lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum TaiItem {
    Plugin {
        path: String,
        enabled: bool,
        inline_comment: Option<String>,
    },
    Comment(String),
    Unknown(String),
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaiSection {
    pub name: String,
    pub items: Vec<TaiItem>,
}

impl TaiSection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    pub fn is_kernel(&self) -> bool {
        self.name.eq_ignore_ascii_case("*KERNEL")
    }

    pub fn is_main(&self) -> bool {
        self.name.eq_ignore_ascii_case("*main")
    }

    pub fn is_all(&self) -> bool {
        self.name.eq_ignore_ascii_case("*ALL")
    }

    pub fn plugins(&self) -> impl Iterator<Item = (&str, bool, Option<&str>)> {
        self.items.iter().filter_map(|item| match item {
            TaiItem::Plugin { path, enabled, inline_comment } => {
                Some((path.as_str(), *enabled, inline_comment.as_deref()))
            }
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaiConfig {
    pub path: PathBuf,
    pub preamble: Vec<TaiItem>,
    pub sections: Vec<TaiSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitOutcome {
    pub reboot_required: bool,
    pub backup_created: bool,
    pub changed: bool,
}

impl TaiConfig {
    /// Standard taiHEN search path: ux0:tai/config.txt has priority over ur0:tai/config.txt.
    pub fn active_path() -> PathBuf {
        let ux0 = PathBuf::from("ux0:tai/config.txt");
        if ux0.exists() { ux0 } else { PathBuf::from("ur0:tai/config.txt") }
    }

    /// Load the active config from disk or create an empty standard structure.
    pub fn load_active() -> Result<Self> {
        let path = Self::active_path();
        Self::load_or_default(path)
    }

    pub fn load_or_default(path: PathBuf) -> Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("couldn't read {}", path.display()))?;
            Self::parse(&text, path)
        } else {
            Ok(Self {
                path,
                preamble: vec![
                    TaiItem::Comment("# taiHEN configuration file".to_string()),
                    TaiItem::Blank,
                ],
                sections: vec![
                    TaiSection::new("*KERNEL"),
                    TaiSection::new("*main"),
                    TaiSection::new("*ALL"),
                ],
            })
        }
    }

    /// Parse config.txt without losing comments, unknown lines, or empty lines.
    pub fn parse(text: &str, path: PathBuf) -> Result<Self> {
        let clean_text = text.trim_start_matches('\u{feff}');
        let mut preamble = Vec::new();
        let mut sections: Vec<TaiSection> = Vec::new();
        let mut current_section: Option<TaiSection> = None;

        for raw_line in clean_text.lines() {
            let line = raw_line.trim_end_matches(['\r', '\n']);
            let trimmed = line.trim();

            if trimmed.is_empty() {
                if let Some(sec) = current_section.as_mut() {
                    sec.items.push(TaiItem::Blank);
                } else {
                    preamble.push(TaiItem::Blank);
                }
                continue;
            }

            // Section header starts with '*'
            if trimmed.starts_with('*') {
                if let Some(sec) = current_section.take() {
                    sections.push(sec);
                }
                current_section = Some(TaiSection::new(trimmed));
                continue;
            }

            // Non-section line: could be a comment, a plugin, or unknown
            let item = parse_line_item(line);
            if let Some(sec) = current_section.as_mut() {
                sec.items.push(item);
            } else {
                preamble.push(item);
            }
        }

        if let Some(sec) = current_section {
            sections.push(sec);
        }

        Ok(Self { path, preamble, sections })
    }

    /// Serialize back to standard taiHEN config.txt format with clean Unix line endings.
    pub fn to_config_txt(&self) -> String {
        let mut out = String::new();

        for item in &self.preamble {
            render_item(&mut out, item);
        }

        for (i, sec) in self.sections.iter().enumerate() {
            if !out.is_empty() && !out.ends_with("\n\n") && !out.ends_with("\n") {
                out.push('\n');
            }
            if i > 0 && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&sec.name);
            out.push('\n');

            for item in &sec.items {
                render_item(&mut out, item);
            }
        }

        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    /// Export to structured JSON.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("failed to serialize TaiConfig to JSON")
    }

    /// Import from structured JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).context("failed to deserialize TaiConfig from JSON")
    }

    pub fn get_section(&self, name: &str) -> Option<&TaiSection> {
        self.sections.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn get_section_mut(&mut self, name: &str) -> Option<&mut TaiSection> {
        self.sections.iter_mut().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn ensure_section(&mut self, name: &str) -> &mut TaiSection {
        if let Some(pos) = self.sections.iter().position(|s| s.name.eq_ignore_ascii_case(name)) {
            &mut self.sections[pos]
        } else {
            self.sections.push(TaiSection::new(name));
            self.sections.last_mut().unwrap()
        }
    }

    /// Find an entry by exact path or filename suffix.
    pub fn find_plugin(&self, needle: &str) -> Option<(&TaiSection, &TaiItem)> {
        let needle_lower = needle.to_lowercase();
        for sec in &self.sections {
            for item in &sec.items {
                if let TaiItem::Plugin { path, .. } = item {
                    let path_lower = path.to_lowercase();
                    if path_lower == needle_lower || path_lower.ends_with(&format!("/{needle_lower}")) {
                        return Some((sec, item));
                    }
                }
            }
        }
        None
    }

    /// Enable or disable a plugin in any section it appears in.
    pub fn set_plugin_enabled(&mut self, path_needle: &str, target_enabled: bool) -> bool {
        let needle_lower = path_needle.to_lowercase();
        let mut changed = false;
        for sec in &mut self.sections {
            for item in &mut sec.items {
                if let TaiItem::Plugin { path, enabled, .. } = item {
                    let path_lower = path.to_lowercase();
                    if (path_lower == needle_lower || path_lower.ends_with(&format!("/{needle_lower}")))
                        && *enabled != target_enabled
                    {
                        *enabled = target_enabled;
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Remove a plugin completely from all sections.
    pub fn remove_plugin(&mut self, path_needle: &str) -> bool {
        let needle_lower = path_needle.to_lowercase();
        let mut changed = false;
        for sec in &mut self.sections {
            let before = sec.items.len();
            sec.items.retain(|item| match item {
                TaiItem::Plugin { path, .. } => {
                    let path_lower = path.to_lowercase();
                    !(path_lower == needle_lower || path_lower.ends_with(&format!("/{needle_lower}")))
                }
                _ => true,
            });
            if sec.items.len() != before {
                changed = true;
            }
        }
        changed
    }

    /// Add or update a plugin entry in a specified section.
    pub fn add_or_update_plugin(&mut self, section_name: &str, path: &str, enabled: bool) -> bool {
        let path_clean = path.trim();
        let sec = self.ensure_section(section_name);

        for item in &mut sec.items {
            if let TaiItem::Plugin { path: existing_path, enabled: existing_enabled, .. } = item {
                if existing_path.eq_ignore_ascii_case(path_clean) {
                    if *existing_enabled != enabled {
                        *existing_enabled = enabled;
                        return true;
                    }
                    return false;
                }
            }
        }

        sec.items.push(TaiItem::Plugin {
            path: path_clean.to_string(),
            enabled,
            inline_comment: None,
        });
        true
    }

    /// Sorts plugins under *KERNEL according to PS Vita community best practices:
    /// Priority 1: Storage drivers (yamt, gamesd, storagemgr)
    /// Priority 2: Core patches / DRM decrypters (0syscall6, nonpdrm)
    /// Priority 3: Low-level syscall / hardware hooks (fd_fix, ioplus, kubridge)
    /// Priority 4: File patch redirection (repatch)
    /// Priority 5: General kernel plugins
    pub fn auto_sort_kernel(&mut self) -> bool {
        let Some(sec) = self.get_section_mut("*KERNEL") else {
            return false;
        };

        let before = sec.items.clone();
        sec.items.sort_by_key(|item| match item {
            TaiItem::Plugin { path, .. } => kernel_plugin_priority(path),
            _ => 10,
        });
        sec.items != before
    }

    /// Comprehensive validation of syntax, module types, path safety, conflicts, and file existence.
    pub fn validate(&self) -> ValidationReport {
        let mut report = ValidationReport::default();

        // Check storage collision: ux0 vs ur0
        let ux0 = Path::new("ux0:tai/config.txt");
        let ur0 = Path::new("ur0:tai/config.txt");
        if ux0.exists() && ur0.exists() {
            report.add_warning(
                None,
                None,
                "Both ux0:tai/config.txt and ur0:tai/config.txt exist. taiHEN ignores ur0 when ux0 is present.",
            );
        }

        let mut seen_plugins = std::collections::HashSet::new();
        let mut has_0syscall6 = false;
        let mut has_refood = false;
        let mut has_yamt = false;
        let mut has_storagemgr = false;
        let mut has_fdfix = false;
        let mut has_repatch = false;

        for sec in &self.sections {
            let sec_name = &sec.name;

            // Validate section header syntax
            if !sec_name.starts_with('*') || sec_name.len() < 2 || sec_name.contains(|c: char| c.is_whitespace()) {
                report.add_error(
                    Some(sec_name.clone()),
                    None,
                    format!("Invalid section name format '{sec_name}'. Must start with '*' with no spaces."),
                );
            }

            let is_kernel = sec.is_kernel();
            let is_user = sec.is_main() || sec.is_all() || (!is_kernel && sec_name.starts_with('*'));
            let mut seen_non_storage_driver = false;

            for item in &sec.items {
                if let TaiItem::Plugin { path, enabled, .. } = item {
                    // Check buffer length limit in taiHEN parser
                    if path.len() > 255 {
                        report.add_error(
                            Some(sec_name.clone()),
                            Some(path.clone()),
                            format!("Plugin path exceeds taiHEN 255-character buffer limit ({} chars)", path.len()),
                        );
                    }

                    // Check path traversal or illegal control characters
                    if path.contains(['\r', '\n', '\0']) || path.split(['/', '\\']).any(|p| p == "..") {
                        report.add_error(
                            Some(sec_name.clone()),
                            Some(path.clone()),
                            "Plugin path contains invalid characters or directory traversal '..'",
                        );
                    }

                    // Check allowed Vita prefixes
                    let allowed_prefix = path.starts_with("ur0:")
                        || path.starts_with("ux0:")
                        || path.starts_with("gro0:")
                        || path.starts_with("uma0:");
                    if !allowed_prefix {
                        report.add_warning(
                            Some(sec_name.clone()),
                            Some(path.clone()),
                            "Plugin path does not start with standard Vita storage prefix (ur0:, ux0:)",
                        );
                    }

                    let path_lower = path.to_lowercase();

                    // Rule 1: .skprx in Kernel only, .suprx in userland only
                    if is_kernel {
                        if path_lower.ends_with(".suprx") {
                            report.add_error(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                "Userland plugin (.suprx) placed inside *KERNEL section! This will crash the system.",
                            );
                        } else if !path_lower.ends_with(".skprx") {
                            report.add_warning(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                "Kernel plugin does not end with .skprx",
                            );
                        }

                        // Rule: storage on ux0: warning
                        if path_lower.starts_with("ux0:") {
                            report.add_warning(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                "Kernel plugin is located on ux0:. If SD2Vita fails to mount, system won't boot. Recommended: move to ur0:tai/.",
                            );
                        }

                        // Rule: storage driver ordering (should be first)
                        let prio = kernel_plugin_priority(path);
                        if prio == 1 && seen_non_storage_driver {
                            report.add_warning(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                format!("Storage driver ({path}) should load at the very top of *KERNEL to mount storage before other plugins."),
                            );
                        } else if prio > 1 {
                            seen_non_storage_driver = true;
                        }
                    } else if is_user {
                        if path_lower.ends_with(".skprx") {
                            report.add_error(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                "Kernel plugin (.skprx) placed inside user section! Kernel plugins only work under *KERNEL.",
                            );
                        }
                    }

                    // Rule 2: Duplicates
                    let key = (sec_name.clone(), path_lower.clone());
                    if !seen_plugins.insert(key) {
                        report.add_warning(
                            Some(sec_name.clone()),
                            Some(path.clone()),
                            "Duplicate plugin entry in the same section",
                        );
                    }

                    // Rule 3: Known conflicts tracking
                    if *enabled {
                        if path_lower.contains("0syscall6") { has_0syscall6 = true; }
                        if path_lower.contains("refood") { has_refood = true; }
                        if path_lower.contains("yamt") { has_yamt = true; }
                        if path_lower.contains("storagemgr") { has_storagemgr = true; }
                        if path_lower.contains("fd_fix") { has_fdfix = true; }
                        if path_lower.contains("repatch") { has_repatch = true; }
                    }

                    // Rule 4: File existence check on disk (if enabled)
                    if *enabled && (path.starts_with("ur0:") || path.starts_with("ux0:")) {
                        if !plugin_file_exists(path) {
                            report.add_warning(
                                Some(sec_name.clone()),
                                Some(path.clone()),
                                "Referenced plugin file was not found on disk",
                            );
                        }
                    }
                }
            }
        }

        if has_0syscall6 && has_refood {
            report.add_warning(
                None,
                None,
                "Both 0syscall6.skprx and refood.skprx are enabled. They conflict and may cause freeze during boot.",
            );
        }
        if has_yamt && has_storagemgr {
            report.add_warning(
                None,
                None,
                "Both yamt.skprx and storagemgr.skprx are enabled. Two storage drivers active simultaneously will corrupt storage.",
            );
        }
        if has_fdfix && has_repatch {
            report.add_warning(
                None,
                None,
                "Both fd_fix.skprx and repatch are enabled. Ensure repatch is updated to version 3.0+ (rePatch reLoaded) to avoid file descriptor conflicts in Android ports.",
            );
        }

        report
    }

    /// Transactional Commit following the exact fail-safe flowchart:
    /// Snapshot -> In-memory Mutate -> Validate -> Write .tmp -> Re-parse & Verify -> Backup -> Atomic Replace -> Reboot check.
    pub fn commit_transaction(&self) -> Result<CommitOutcome> {
        // 1. Pre-validation in memory
        let initial_report = self.validate();
        if initial_report.has_errors() {
            let err_msgs: Vec<_> = initial_report.errors().iter().map(|e| e.message.as_str()).collect();
            bail!("taiHEN config validation failed: {}", err_msgs.join("; "));
        }

        // Compare against existing to check what changed
        let existing_raw = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).context("failed to read existing taiHEN config"),
        };

        let new_raw = self.to_config_txt();
        if existing_raw == new_raw {
            return Ok(CommitOutcome {
                reboot_required: false,
                backup_created: false,
                changed: false,
            });
        }

        // Determine if kernel section was modified
        let reboot_required = if let Ok(old_cfg) = Self::parse(&existing_raw, self.path.clone()) {
            let old_kernel = old_cfg.get_section("*KERNEL").map(|s| s.items.clone()).unwrap_or_default();
            let new_kernel = self.get_section("*KERNEL").map(|s| s.items.clone()).unwrap_or_default();
            old_kernel != new_kernel
        } else {
            true
        };

        // Ensure parent directory exists (e.g. ur0:tai or ux0:tai)
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // 2. Write to isolated temporary file
        let tmp_path = PathBuf::from(format!("{}.vitaforge.tmp", self.path.display()));
        std::fs::write(&tmp_path, &new_raw)
            .with_context(|| format!("couldn't write temporary file {}", tmp_path.display()))?;

        // 3. Re-parse and verify temporary file
        let recheck_text = std::fs::read_to_string(&tmp_path)
            .with_context(|| format!("couldn't re-read {}", tmp_path.display()))?;
        let recheck_cfg = match Self::parse(&recheck_text, self.path.clone()) {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                bail!("re-verification failed on temporary config: {e:#}");
            }
        };

        let recheck_report = recheck_cfg.validate();
        if recheck_report.has_errors() {
            let _ = std::fs::remove_file(&tmp_path);
            let err_msgs: Vec<_> = recheck_report.errors().iter().map(|e| e.message.as_str()).collect();
            bail!("re-verification validation error: {}", err_msgs.join("; "));
        }

        // 4. Backup existing config.txt before overwriting
        let mut backup_created = false;
        if self.path.exists() {
            let bak_path = PathBuf::from(format!("{}.bak", self.path.display()));
            // Keep original working backup if not already present
            if !bak_path.exists() {
                let _ = std::fs::copy(&self.path, &bak_path);
                backup_created = true;
            }
            // Always keep latest previous version as .vf_prev
            let prev_path = PathBuf::from(format!("{}.vf_prev", self.path.display()));
            let _ = std::fs::copy(&self.path, &prev_path);
        }

        // 5. Atomic Replace
        std::fs::rename(&tmp_path, &self.path)
            .with_context(|| format!("failed to replace {} with updated config", self.path.display()))?;

        // 6. Post-commit runtime actions
        if !reboot_required {
            let _ = tai_reload_config();
        }

        Ok(CommitOutcome {
            reboot_required,
            backup_created,
            changed: true,
        })
    }

    /// Migrate plugins from ux0:tai/config.txt into ur0:tai/config.txt and disable ux0:tai/config.txt.
    pub fn consolidate_ux0_into_ur0() -> Result<usize> {
        let ux0_path = PathBuf::from("ux0:tai/config.txt");
        let ur0_path = PathBuf::from("ur0:tai/config.txt");
        if !ux0_path.exists() {
            return Ok(0);
        }

        let ux0_cfg = Self::load_or_default(ux0_path.clone())?;
        let mut ur0_cfg = Self::load_or_default(ur0_path.clone())?;

        let mut consolidated_count = 0;
        for sec in &ux0_cfg.sections {
            for item in &sec.items {
                if let TaiItem::Plugin { path, enabled, .. } = item {
                    if ur0_cfg.add_or_update_plugin(&sec.name, path, *enabled) {
                        consolidated_count += 1;
                    }
                }
            }
        }

        ur0_cfg.commit_transaction()?;

        // Disable ux0:tai/config.txt by renaming it
        let disabled_path = PathBuf::from("ux0:tai/config.txt.disabled_by_vitaforge");
        let _ = std::fs::rename(&ux0_path, &disabled_path);

        Ok(consolidated_count)
    }
}

fn parse_line_item(line: &str) -> TaiItem {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return TaiItem::Blank;
    }

    let is_comment = trimmed.starts_with('#');
    let content = if is_comment {
        trimmed.trim_start_matches('#').trim()
    } else {
        trimmed
    };

    // Check if content looks like a Vita plugin path (ends with .skprx/.suprx/.sprx or contains ':')
    let is_plugin = (content.ends_with(".skprx") || content.ends_with(".suprx") || content.ends_with(".sprx"))
        || (content.starts_with("ur0:") || content.starts_with("ux0:") || content.starts_with("gro0:") || content.starts_with("uma0:"));

    if is_plugin {
        // Look for trailing inline comment
        let (path_part, comment_part) = if let Some(hash_pos) = content.find('#') {
            (content[..hash_pos].trim(), Some(content[hash_pos + 1..].trim().to_string()))
        } else {
            (content, None)
        };

        TaiItem::Plugin {
            path: path_part.to_string(),
            enabled: !is_comment,
            inline_comment: comment_part,
        }
    } else if is_comment {
        TaiItem::Comment(line.to_string())
    } else {
        TaiItem::Unknown(line.to_string())
    }
}

fn render_item(out: &mut String, item: &TaiItem) {
    match item {
        TaiItem::Plugin { path, enabled, inline_comment } => {
            if !*enabled {
                out.push('#');
            }
            out.push_str(path);
            if let Some(c) = inline_comment {
                out.push_str(" # ");
                out.push_str(c);
            }
            out.push('\n');
        }
        TaiItem::Comment(text) => {
            out.push_str(text);
            out.push('\n');
        }
        TaiItem::Unknown(text) => {
            out.push_str(text);
            out.push('\n');
        }
        TaiItem::Blank => {
            out.push('\n');
        }
    }
}

fn plugin_file_exists(path: &str) -> bool {
    crate::install::installed::vita_fs::exists(path)
}

pub fn kernel_plugin_priority(path: &str) -> u32 {
    let lower = path.to_lowercase();
    if lower.contains("yamt") || lower.contains("gamesd") || lower.contains("storagemgr") {
        1
    } else if lower.contains("0syscall6") || lower.contains("nonpdrm") {
        2
    } else if lower.contains("fd_fix") || lower.contains("ioplus") || lower.contains("kubridge") {
        3
    } else if lower.contains("repatch") {
        4
    } else {
        5
    }
}

/// Call taiReloadConfig() on Vita without rebooting.
#[cfg(target_os = "vita")]
pub fn tai_reload_config() -> Result<i32> {
    #[link(name = "taihen_stub")]
    extern "C" {
        fn taiReloadConfig() -> i32;
    }
    let res = unsafe { taiReloadConfig() };
    if res < 0 {
        bail!("taiReloadConfig failed: {res:#010x}");
    }
    Ok(res)
}

#[cfg(not(target_os = "vita"))]
pub fn tai_reload_config() -> Result<i32> {
    Ok(0)
}

/// Safe system reboot: validates config.txt first, preventing soft-brick if errors exist.
pub fn safe_reboot() -> Result<()> {
    let cfg = TaiConfig::load_active()?;
    let report = cfg.validate();
    if report.has_errors() {
        let errs: Vec<_> = report.errors().iter().map(|e| e.message.as_str()).collect();
        bail!("Reboot blocked: taiHEN config.txt has fatal errors: {}", errs.join("; "));
    }

    #[cfg(target_os = "vita")]
    unsafe {
        vitasdk_sys::scePowerRequestColdReset();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r#"# taiHEN configuration file
# This is a comment in preamble

*KERNEL
ur0:tai/nonpdrm.skprx
#ur0:tai/0syscall6.skprx
ur0:tai/kubridge.skprx

*main
ur0:tai/henkaku.suprx
#ur0:tai/shellbat.suprx

*ALL
ur0:tai/itls.suprx

*PCSE00001
ux0:tai/custom_game_patch.suprx
"#;

    #[test]
    fn parses_and_serializes_cleanly() {
        let cfg = TaiConfig::parse(SAMPLE_CONFIG, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg.sections.len(), 4);
        assert_eq!(cfg.sections[0].name, "*KERNEL");
        assert_eq!(cfg.sections[1].name, "*main");
        assert_eq!(cfg.sections[2].name, "*ALL");
        assert_eq!(cfg.sections[3].name, "*PCSE00001");

        let serialized = cfg.to_config_txt();
        let reparsed = TaiConfig::parse(&serialized, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn json_roundtrip() {
        let cfg = TaiConfig::parse(SAMPLE_CONFIG, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let json = cfg.to_json().unwrap();
        assert!(json.contains("*KERNEL"));
        assert!(json.contains("kubridge.skprx"));

        let from_json = TaiConfig::from_json(&json).unwrap();
        assert_eq!(cfg, from_json);
    }

    #[test]
    fn toggles_plugin_enabled() {
        let mut cfg = TaiConfig::parse(SAMPLE_CONFIG, PathBuf::from("ur0:tai/config.txt")).unwrap();
        // 0syscall6 is initially disabled (#)
        assert!(cfg.set_plugin_enabled("0syscall6.skprx", true));
        let (_, item) = cfg.find_plugin("0syscall6.skprx").unwrap();
        match item {
            TaiItem::Plugin { enabled, .. } => assert!(*enabled),
            _ => panic!("expected plugin item"),
        }

        // Toggle back to disabled
        assert!(cfg.set_plugin_enabled("0syscall6.skprx", false));
        let (_, item) = cfg.find_plugin("0syscall6.skprx").unwrap();
        match item {
            TaiItem::Plugin { enabled, .. } => assert!(!*enabled),
            _ => panic!("expected plugin item"),
        }
    }

    #[test]
    fn rejects_suprx_in_kernel() {
        let bad_config = "*KERNEL\nur0:tai/bad_user_plugin.suprx\n";
        let cfg = TaiConfig::parse(bad_config, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.has_errors());
        assert!(report.errors()[0].message.contains("Userland plugin (.suprx) placed inside *KERNEL"));
    }

    #[test]
    fn rejects_skprx_in_userland() {
        let bad_config = "*ALL\nur0:tai/bad_kernel_plugin.skprx\n";
        let cfg = TaiConfig::parse(bad_config, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.has_errors());
        assert!(report.errors()[0].message.contains("Kernel plugin (.skprx) placed inside user section"));
    }

    #[test]
    fn detects_conflicting_plugins() {
        let conflict_config = "*KERNEL\nur0:tai/0syscall6.skprx\nur0:tai/refood.skprx\n";
        let cfg = TaiConfig::parse(conflict_config, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(!report.has_errors()); // Warnings, not fatal
        assert!(report.warnings().iter().any(|w| w.message.contains("Both 0syscall6.skprx and refood.skprx are enabled")));
    }

    #[test]
    fn commit_transaction_re_parses_and_creates_backup() {
        let temp_dir = std::env::temp_dir().join(format!("tai_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let config_file = temp_dir.join("config.txt");

        std::fs::write(&config_file, "*KERNEL\nur0:tai/nonpdrm.skprx\n").unwrap();

        let mut cfg = TaiConfig::load_or_default(config_file.clone()).unwrap();
        cfg.add_or_update_plugin("*KERNEL", "ur0:tai/kubridge.skprx", true);

        let outcome = cfg.commit_transaction().unwrap();
        assert!(outcome.changed);
        assert!(outcome.reboot_required); // Kernel section modified

        let written = std::fs::read_to_string(&config_file).unwrap();
        assert!(written.contains("kubridge.skprx"));
        assert!(written.contains("nonpdrm.skprx"));

        // Verify backup was created
        let backup_file = temp_dir.join("config.txt.bak");
        assert!(backup_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn autoplugin_messy_syntax_recovery() {
        // Typical chaotic output generated by AutoPlugin 2 after several installs/uninstalls
        let messy = "\r\n# AutoPlugin II generated\r\n# Do not edit\r\n\r\n*KERNEL\r\n   ur0:tai/nonpdrm.skprx   \r\n#   ur0:tai/0syscall6.skprx\r\n\r\n\r\n*main\r\nux0:tai/shellbat.suprx\r\n\r\n*ALL\r\nur0:tai/itls.suprx\r\n\r\n";
        let cfg = TaiConfig::parse(messy, PathBuf::from("ur0:tai/config.txt")).unwrap();

        assert_eq!(cfg.sections.len(), 3);
        assert_eq!(cfg.sections[0].name, "*KERNEL");
        assert_eq!(cfg.sections[1].name, "*main");
        assert_eq!(cfg.sections[2].name, "*ALL");

        let serialized = cfg.to_config_txt();
        assert!(serialized.contains("ur0:tai/nonpdrm.skprx"));
        assert!(serialized.contains("#ur0:tai/0syscall6.skprx"));
        assert!(serialized.contains("ux0:tai/shellbat.suprx"));
        assert!(serialized.contains("ur0:tai/itls.suprx"));
    }

    #[test]
    fn case_insensitive_sections_and_extensions() {
        let text = "*kernel\r\nur0:tai/plugin.SKPRX\r\n*MAIN\r\nur0:tai/plugin.SuPrX\r\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert!(cfg.sections[0].is_kernel());
        assert!(cfg.sections[1].is_main());

        let report = cfg.validate();
        assert!(!report.has_errors());
    }

    #[test]
    fn inline_comments_preservation() {
        let text = "*KERNEL\nur0:tai/kubridge.skprx # Needed for Android ports\n*main\nur0:tai/vsub.suprx # Subtitle hook\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let serialized = cfg.to_config_txt();

        assert!(serialized.contains("ur0:tai/kubridge.skprx # Needed for Android ports"));
        assert!(serialized.contains("ur0:tai/vsub.suprx # Subtitle hook"));
    }

    #[test]
    fn unknown_directives_preservation() {
        let text = "*KERNEL\nur0:tai/custom.skprx\ncustom_option_flag=1\nenable_extra_ram=true\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let serialized = cfg.to_config_txt();

        assert!(serialized.contains("custom_option_flag=1"));
        assert!(serialized.contains("enable_extra_ram=true"));
    }

    #[test]
    fn path_traversal_and_malformed_chars_rejected() {
        let traversal = "*KERNEL\nur0:tai/../../eboot.bin\n";
        let cfg = TaiConfig::parse(traversal, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.has_errors());
        assert!(report.errors()[0].message.contains("directory traversal"));

        let null_char = "*KERNEL\nur0:tai/plugin\0.skprx\n";
        let cfg2 = TaiConfig::parse(null_char, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report2 = cfg2.validate();
        assert!(report2.has_errors());
    }

    #[test]
    fn kernel_vs_userland_mismatch_strictness() {
        // .suprx in *KERNEL
        let bad1 = "*KERNEL\nur0:tai/foo.suprx\n";
        let r1 = TaiConfig::parse(bad1, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r1.has_errors());

        // .skprx in *main
        let bad2 = "*main\nur0:tai/foo.skprx\n";
        let r2 = TaiConfig::parse(bad2, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r2.has_errors());

        // .skprx in *ALL
        let bad3 = "*ALL\nur0:tai/foo.skprx\n";
        let r3 = TaiConfig::parse(bad3, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r3.has_errors());

        // .skprx in *TITLEID
        let bad4 = "*PCSE00001\nur0:tai/foo.skprx\n";
        let r4 = TaiConfig::parse(bad4, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r4.has_errors());
    }

    #[test]
    fn all_conflicting_plugin_combinations() {
        let text1 = "*KERNEL\nur0:tai/0syscall6.skprx\nur0:tai/refood.skprx\n";
        let r1 = TaiConfig::parse(text1, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r1.warnings().iter().any(|w| w.message.contains("Both 0syscall6.skprx and refood.skprx are enabled")));

        let text2 = "*KERNEL\nur0:tai/yamt.skprx\nur0:tai/storagemgr.skprx\n";
        let r2 = TaiConfig::parse(text2, PathBuf::from("ur0:tai/config.txt")).unwrap().validate();
        assert!(r2.warnings().iter().any(|w| w.message.contains("Both yamt.skprx and storagemgr.skprx are enabled")));
    }

    #[test]
    fn empty_and_whitespace_only_sections() {
        let text = "*KERNEL\n\n\n*main\n   \n*ALL\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg.sections.len(), 3);
        let serialized = cfg.to_config_txt();
        assert!(serialized.contains("*KERNEL"));
        assert!(serialized.contains("*main"));
        assert!(serialized.contains("*ALL"));
    }

    #[test]
    fn duplicate_entries_warning() {
        let text = "*KERNEL\nur0:tai/kubridge.skprx\nur0:tai/kubridge.skprx\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(!report.has_errors());
        assert!(report.warnings().iter().any(|w| w.message.contains("Duplicate plugin entry")));
    }

    #[test]
    fn json_lossless_roundtrip_complex() {
        let complex = r#"# taiHEN complex configuration file
# Managed by vitaForge

*KERNEL
ur0:tai/nonpdrm.skprx
ur0:tai/0syscall6.skprx
#ur0:tai/kubridge.skprx # Disabled for test
ur0:tai/ioplus.skprx

*main
ur0:tai/henkaku.suprx
ux0:tai/shellbat.suprx

*ALL
ur0:tai/itls.suprx

*PCSE00001
ux0:tai/patch_game.suprx

*PCSB00002
ux0:tai/resolution_hack.suprx
"#;
        let cfg = TaiConfig::parse(complex, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let json = cfg.to_json().unwrap();
        let from_json = TaiConfig::from_json(&json).unwrap();
        assert_eq!(cfg, from_json);

        let serialized = from_json.to_config_txt();
        let reparsed = TaiConfig::parse(&serialized, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn transactional_commit_fail_safe_rollback() {
        let temp_dir = std::env::temp_dir().join(format!("tai_test_fail_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let config_file = temp_dir.join("config.txt");

        let original_content = "*KERNEL\nur0:tai/valid.skprx\n";
        std::fs::write(&config_file, original_content).unwrap();

        let mut cfg = TaiConfig::load_or_default(config_file.clone()).unwrap();
        // Insert an invalid plugin (.suprx in *KERNEL)
        cfg.add_or_update_plugin("*KERNEL", "ur0:tai/bad.suprx", true);

        let result = cfg.commit_transaction();
        assert!(result.is_err(), "Commit must fail on invalid configuration");

        // Verify original file was completely untouched!
        let content_after = std::fs::read_to_string(&config_file).unwrap();
        assert_eq!(content_after, original_content);

        // Verify temporary file was deleted
        let tmp_file = temp_dir.join("config.txt.vitaforge.tmp");
        assert!(!tmp_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn add_or_update_plugin_idempotency() {
        let mut cfg = TaiConfig::load_or_default(PathBuf::from("ur0:tai/config.txt")).unwrap();
        // First add: true
        assert!(cfg.add_or_update_plugin("*KERNEL", "ur0:tai/kubridge.skprx", true));
        // Second add identical: false (already present and enabled)
        assert!(!cfg.add_or_update_plugin("*KERNEL", "ur0:tai/kubridge.skprx", true));
        // Third add with enabled: false -> updates without duplicating line
        assert!(cfg.add_or_update_plugin("*KERNEL", "ur0:tai/kubridge.skprx", false));

        let sec = cfg.get_section("*KERNEL").unwrap();
        let matching: Vec<_> = sec.plugins().filter(|(p, _, _)| *p == "ur0:tai/kubridge.skprx").collect();
        assert_eq!(matching.len(), 1);
        assert!(!matching[0].1); // disabled
    }

    #[test]
    fn remove_plugin_cleanly() {
        let mut cfg = TaiConfig::load_or_default(PathBuf::from("ur0:tai/config.txt")).unwrap();
        cfg.add_or_update_plugin("*KERNEL", "ur0:tai/to_remove.skprx", true);
        cfg.add_or_update_plugin("*main", "ur0:tai/to_remove.suprx", true);

        assert!(cfg.remove_plugin("to_remove.skprx"));
        assert!(cfg.find_plugin("to_remove.skprx").is_none());

        assert!(cfg.remove_plugin("to_remove.suprx"));
        assert!(cfg.find_plugin("to_remove.suprx").is_none());
    }

    #[test]
    fn preamble_comments_and_blanks() {
        let text = "# Comment line 1\n# Comment line 2\n\n*KERNEL\nur0:tai/foo.skprx\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg.preamble.len(), 3);
        let serialized = cfg.to_config_txt();
        assert!(serialized.starts_with("# Comment line 1\n# Comment line 2\n\n*KERNEL"));
    }

    #[test]
    fn consolidate_ux0_into_ur0_test() {
        let temp_dir = std::env::temp_dir().join(format!("tai_consolidate_{}", std::process::id()));
        let ux0_dir = temp_dir.join("ux0").join("tai");
        let ur0_dir = temp_dir.join("ur0").join("tai");
        let _ = std::fs::create_dir_all(&ux0_dir);
        let _ = std::fs::create_dir_all(&ur0_dir);

        let ux0_file = ux0_dir.join("config.txt");
        let ur0_file = ur0_dir.join("config.txt");

        std::fs::write(&ux0_file, "*KERNEL\nur0:tai/from_ux0.skprx\n").unwrap();
        std::fs::write(&ur0_file, "*KERNEL\nur0:tai/existing_ur0.skprx\n").unwrap();

        let ux0_cfg = TaiConfig::load_or_default(ux0_file.clone()).unwrap();
        let mut ur0_cfg = TaiConfig::load_or_default(ur0_file.clone()).unwrap();

        for sec in &ux0_cfg.sections {
            for item in &sec.items {
                if let TaiItem::Plugin { path, enabled, .. } = item {
                    ur0_cfg.add_or_update_plugin(&sec.name, path, *enabled);
                }
            }
        }
        ur0_cfg.commit_transaction().unwrap();

        let final_ur0 = std::fs::read_to_string(&ur0_file).unwrap();
        assert!(final_ur0.contains("from_ux0.skprx"));
        assert!(final_ur0.contains("existing_ur0.skprx"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn strips_utf8_bom() {
        let bom_text = "\u{feff}*KERNEL\nur0:tai/nonpdrm.skprx\n";
        let cfg = TaiConfig::parse(bom_text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert_eq!(cfg.sections.len(), 1);
        assert_eq!(cfg.sections[0].name, "*KERNEL");
        assert!(cfg.sections[0].is_kernel());
        let serialized = cfg.to_config_txt();
        assert!(!serialized.starts_with('\u{feff}'));
    }

    #[test]
    fn storage_driver_ordering_warning() {
        let text = "*KERNEL\nur0:tai/kubridge.skprx\nur0:tai/yamt.skprx\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.warnings().iter().any(|w| w.message.contains("Storage driver") && w.message.contains("top of *KERNEL")));
    }

    #[test]
    fn auto_sort_kernel_places_storage_first() {
        let text = "*KERNEL\nur0:tai/repatch.skprx\nur0:tai/kubridge.skprx\nur0:tai/yamt.skprx\nur0:tai/0syscall6.skprx\n";
        let mut cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        assert!(cfg.auto_sort_kernel());

        let sec = cfg.get_section("*KERNEL").unwrap();
        let paths: Vec<&str> = sec.plugins().map(|(p, _, _)| p).collect();
        assert_eq!(paths, vec![
            "ur0:tai/yamt.skprx",
            "ur0:tai/0syscall6.skprx",
            "ur0:tai/kubridge.skprx",
            "ur0:tai/repatch.skprx",
        ]);

        let report = cfg.validate();
        assert!(!report.warnings().iter().any(|w| w.message.contains("Storage driver")));
    }

    #[test]
    fn rejects_path_exceeding_255_chars() {
        let long_name = "a".repeat(260);
        let text = format!("*KERNEL\nur0:tai/{long_name}.skprx\n");
        let cfg = TaiConfig::parse(&text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.has_errors());
        assert!(report.errors()[0].message.contains("255-character buffer limit"));
    }

    #[test]
    fn kernel_on_ux0_warning() {
        let text = "*KERNEL\nux0:tai/custom.skprx\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.warnings().iter().any(|w| w.message.contains("Kernel plugin is located on ux0:")));
    }

    #[test]
    fn fd_fix_and_repatch_advisory() {
        let text = "*KERNEL\nur0:tai/fd_fix.skprx\nur0:tai/repatch.skprx\n";
        let cfg = TaiConfig::parse(text, PathBuf::from("ur0:tai/config.txt")).unwrap();
        let report = cfg.validate();
        assert!(report.warnings().iter().any(|w| w.message.contains("Both fd_fix.skprx and repatch are enabled")));
    }
}
