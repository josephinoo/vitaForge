//! Lossless model of taiHEN's config.txt.
//!
//! Every line keeps its original text, so parsing and printing an untouched file
//! gives back the same bytes. Plugins are switched off by commenting them out
//! (`# ur0:tai/foo.suprx`): taiHEN skips `#` lines and the plugin keeps its place.

use anyhow::{bail, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    /// `*KERNEL`, `*main`, `*ALL` or `*TITLEID`.
    Header(String),
    Entry { path: String, enabled: bool },
    /// Comments and blank lines.
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    kind: Kind,
    raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaiConfig {
    lines: Vec<Line>,
    crlf: bool,
    trailing_newline: bool,
}

/// One plugin line as the manager shows it. `line` identifies it for the edit calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryView {
    pub line: usize,
    pub section: Option<String>,
    pub path: String,
    pub enabled: bool,
    pub protected: bool,
}

impl EntryView {
    pub fn file_name(&self) -> &str {
        file_name(&self.path)
    }
}

pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\', ':']).next().unwrap_or(path)
}

/// HENkaku's own module: switching it off loses the HENkaku settings and the
/// version spoof, so the manager never touches it.
pub fn is_protected(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    name == "henkaku.suprx" || name == "henkaku.skprx"
}

fn is_module(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let device = lower.split_once(':').is_some_and(|(dev, _)| !dev.is_empty() && dev.len() <= 6 && !dev.contains(['/', ' ']));
    device && (lower.ends_with(".suprx") || lower.ends_with(".skprx")) && !lower.contains(char::is_whitespace)
}

fn classify(raw: &str) -> Kind {
    let text = raw.trim();
    if let Some(rest) = text.strip_prefix('#') {
        let rest = rest.trim();
        if is_module(rest) {
            return Kind::Entry { path: rest.to_owned(), enabled: false };
        }
        return Kind::Other;
    }
    if text.starts_with('*') {
        return Kind::Header(text.to_owned());
    }
    if text.is_empty() {
        return Kind::Other;
    }
    Kind::Entry { path: text.to_owned(), enabled: true }
}

fn same_path(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl TaiConfig {
    pub fn parse(text: &str) -> Self {
        let crlf = text.contains("\r\n");
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n')
                .map(|raw| {
                    let raw = raw.strip_suffix('\r').unwrap_or(raw).to_owned();
                    Line { kind: classify(&raw), raw }
                })
                .collect()
        };
        Self { lines, crlf, trailing_newline }
    }

    pub fn render(&self) -> String {
        let ending = if self.crlf { "\r\n" } else { "\n" };
        let mut out = self.lines.iter().map(|l| l.raw.as_str()).collect::<Vec<_>>().join(ending);
        if self.trailing_newline && !self.lines.is_empty() {
            out.push_str(ending);
        }
        out
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Plain text of each line, for the read-only viewer.
    pub fn raw_lines(&self) -> impl Iterator<Item = (&str, LineStyle)> {
        self.lines.iter().map(|l| {
            let style = match &l.kind {
                Kind::Header(_) => LineStyle::Header,
                Kind::Entry { enabled: true, .. } => LineStyle::Entry,
                Kind::Entry { enabled: false, .. } => LineStyle::Disabled,
                Kind::Other => LineStyle::Comment,
            };
            (l.raw.as_str(), style)
        })
    }

    pub fn entries(&self) -> Vec<EntryView> {
        let mut section = None;
        let mut out = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            match &line.kind {
                Kind::Header(name) => section = Some(name.clone()),
                Kind::Entry { path, enabled } => out.push(EntryView {
                    line: index,
                    section: section.clone(),
                    path: path.clone(),
                    enabled: *enabled,
                    protected: is_protected(path),
                }),
                Kind::Other => {}
            }
        }
        out
    }

    pub fn entry(&self, line: usize) -> Option<EntryView> {
        self.entries().into_iter().find(|e| e.line == line)
    }

    pub fn find(&self, section: &str, path: &str) -> Option<EntryView> {
        self.entries().into_iter().find(|e| e.section.as_deref() == Some(section) && same_path(&e.path, path))
    }

    /// Every section that lists `path`, enabled or not.
    pub fn sections_of(&self, path: &str) -> Vec<EntryView> {
        self.entries().into_iter().filter(|e| same_path(&e.path, path)).collect()
    }

    fn editable(&self, line: usize) -> Result<EntryView> {
        let Some(entry) = self.entry(line) else { bail!("that line is not a plugin") };
        if entry.protected {
            bail!("{} belongs to HENkaku and can't be changed", entry.file_name());
        }
        Ok(entry)
    }

    pub fn set_enabled(&mut self, line: usize, enabled: bool) -> Result<bool> {
        let entry = self.editable(line)?;
        if entry.enabled == enabled {
            return Ok(false);
        }
        let raw = if enabled { entry.path.clone() } else { format!("# {}", entry.path) };
        self.lines[line] = Line { kind: Kind::Entry { path: entry.path, enabled }, raw };
        Ok(true)
    }

    pub fn remove(&mut self, line: usize) -> Result<()> {
        self.editable(line)?;
        self.lines.remove(line);
        Ok(())
    }

    /// Swaps the entry with its neighbour in the same section. Returns its new
    /// line, or `None` when it is already first/last. Load order inside a section
    /// is the order taiHEN starts the modules in.
    pub fn move_entry(&mut self, line: usize, up: bool) -> Result<Option<usize>> {
        let entry = self.editable(line)?;
        let neighbour = self
            .entries()
            .into_iter()
            .filter(|e| e.section == entry.section)
            .filter(|e| if up { e.line < line } else { e.line > line })
            .map(|e| e.line);
        let neighbour = if up { neighbour.max() } else { neighbour.min() };
        let Some(other) = neighbour else { return Ok(None) };
        self.lines.swap(line, other);
        Ok(Some(other))
    }

    /// Adds `path` to `section` (created at the end of the file if missing). A
    /// disabled copy is switched back on instead of duplicated. The new line goes
    /// before the first entry `insert_before` accepts, else after the section's
    /// last line. Returns whether anything changed.
    pub fn add(&mut self, section: &str, path: &str, insert_before: impl Fn(&str) -> bool) -> bool {
        if let Some(existing) = self.find(section, path) {
            if existing.enabled {
                return false;
            }
            let path = existing.path.clone();
            self.lines[existing.line] = Line { raw: path.clone(), kind: Kind::Entry { path, enabled: true } };
            return true;
        }
        let new_line = Line { kind: Kind::Entry { path: path.to_owned(), enabled: true }, raw: path.to_owned() };
        let Some(header) = self.lines.iter().position(|l| l.kind == Kind::Header(section.to_owned())) else {
            self.trailing_newline = true;
            self.lines.push(Line { kind: Kind::Header(section.to_owned()), raw: section.to_owned() });
            self.lines.push(new_line);
            return true;
        };
        let end = self.lines[header + 1..]
            .iter()
            .position(|l| matches!(l.kind, Kind::Header(_)))
            .map_or(self.lines.len(), |offset| header + 1 + offset);
        let before = (header + 1..end).find(|&i| match &self.lines[i].kind {
            Kind::Entry { path: other, .. } => !is_protected(other) && insert_before(other),
            _ => false,
        });
        let at = before.unwrap_or_else(|| {
            // After the last non-blank line, so blank separators stay between sections.
            (header + 1..end).rev().find(|&i| !self.lines[i].raw.trim().is_empty()).map_or(header + 1, |i| i + 1)
        });
        if at == self.lines.len() {
            self.trailing_newline = true;
        }
        self.lines.insert(at, new_line);
        true
    }

    /// Comments out every plugin HENkaku doesn't need. Returns the lines it
    /// switched off, so they (and only they) can be restored later.
    pub fn disable_all(&mut self) -> Vec<(Option<String>, String)> {
        let mut disabled = Vec::new();
        for entry in self.entries() {
            if entry.enabled && !entry.protected && self.set_enabled(entry.line, false).unwrap_or(false) {
                disabled.push((entry.section, entry.path));
            }
        }
        disabled
    }

    /// Switches back on the lines `disable_all` returned that are still there.
    pub fn restore_enabled(&mut self, lines: &[(Option<String>, String)]) -> usize {
        let mut count = 0;
        for entry in self.entries() {
            let listed = lines.iter().any(|(s, p)| *s == entry.section && same_path(p, &entry.path));
            if listed && !entry.enabled && self.set_enabled(entry.line, true).unwrap_or(false) {
                count += 1;
            }
        }
        count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Header,
    Entry,
    Disabled,
    Comment,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENSO: &str = "# For users plugins, you must refresh taiHEN from HENkaku Settings for\n\
# changes to take place.\n\
# For kernel plugins, you must reboot for changes to take place.\n\
*KERNEL\n\
# henkaku.skprx is hard-coded to load and is not listed here\n\
ur0:tai/nonpdrm.skprx\n\
*main\n\
# main is a special titleid for SceShell\n\
ur0:tai/henkaku.suprx\n\
*NPXS10015\n\
# this is for modifying the version string\n\
ur0:tai/henkaku.suprx\n\
*NPXS10016\n\
# this is for modifying the version string in settings widget\n\
ur0:tai/henkaku.suprx\n";

    fn never(_: &str) -> bool {
        false
    }

    #[test]
    fn untouched_files_round_trip() {
        for text in [ENSO, "", "*KERNEL", "*KERNEL\r\nur0:tai/a.skprx\r\n", "\n\n*main\n# ur0:tai/x.suprx\n\n"] {
            assert_eq!(TaiConfig::parse(text).render(), text);
        }
    }

    #[test]
    fn sees_sections_and_disabled_lines() {
        let config = TaiConfig::parse("*KERNEL\nur0:tai/a.skprx\n#ur0:tai/b.skprx\n# a note\n*main\nux0:tai/c.suprx\n");
        let entries = config.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].path, "ur0:tai/b.skprx");
        assert!(!entries[1].enabled);
        assert_eq!(entries[2].section.as_deref(), Some("*main"));
    }

    #[test]
    fn toggling_keeps_the_place_and_round_trips() {
        let mut config = TaiConfig::parse(ENSO);
        let line = config.find("*KERNEL", "ur0:tai/nonpdrm.skprx").unwrap().line;
        assert!(config.set_enabled(line, false).unwrap());
        assert!(config.render().contains("*KERNEL\n# henkaku.skprx is hard-coded to load and is not listed here\n# ur0:tai/nonpdrm.skprx\n*main"));
        assert!(config.set_enabled(line, true).unwrap());
        assert_eq!(config.render(), ENSO);
    }

    #[test]
    fn henkaku_lines_are_protected() {
        let mut config = TaiConfig::parse(ENSO);
        let line = config.find("*main", "ur0:tai/henkaku.suprx").unwrap().line;
        assert!(config.set_enabled(line, false).is_err());
        assert!(config.remove(line).is_err());
        assert!(config.move_entry(line, true).is_err());
        assert_eq!(config.render(), ENSO);
    }

    #[test]
    fn moves_inside_its_section_only() {
        let mut config = TaiConfig::parse("*KERNEL\nur0:tai/a.skprx\n# note\nur0:tai/b.skprx\n*main\nur0:tai/c.suprx\n");
        let b = config.find("*KERNEL", "ur0:tai/b.skprx").unwrap().line;
        assert_eq!(config.move_entry(b, true).unwrap(), Some(1));
        assert_eq!(config.render(), "*KERNEL\nur0:tai/b.skprx\n# note\nur0:tai/a.skprx\n*main\nur0:tai/c.suprx\n");
        assert_eq!(config.move_entry(1, true).unwrap(), None);
        let a = config.find("*KERNEL", "ur0:tai/a.skprx").unwrap().line;
        assert_eq!(config.move_entry(a, false).unwrap(), None);
    }

    #[test]
    fn removes_a_line() {
        let mut config = TaiConfig::parse("*main\nur0:tai/a.suprx\nur0:tai/b.suprx\n");
        config.remove(0).unwrap_err();
        config.remove(1).unwrap();
        assert_eq!(config.render(), "*main\nur0:tai/b.suprx\n");
    }

    #[test]
    fn adds_before_the_next_section_and_after_blank_separators() {
        let mut config = TaiConfig::parse("*KERNEL\nur0:tai/old.skprx\n\n*ALL\nur0:tai/old.suprx\n");
        assert!(config.add("*KERNEL", "ur0:tai/new.skprx", never));
        assert_eq!(config.render(), "*KERNEL\nur0:tai/old.skprx\nur0:tai/new.skprx\n\n*ALL\nur0:tai/old.suprx\n");
        assert!(!config.add("*KERNEL", "UR0:tai/NEW.skprx", never));
    }

    #[test]
    fn adding_a_disabled_line_switches_it_on() {
        let mut config = TaiConfig::parse("*main\n# ur0:tai/a.suprx\n");
        assert!(config.add("*main", "ur0:tai/a.suprx", never));
        assert_eq!(config.render(), "*main\nur0:tai/a.suprx\n");
    }

    #[test]
    fn creates_missing_sections_and_keeps_crlf() {
        let mut config = TaiConfig::parse("*KERNEL\r\nur0:tai/a.skprx");
        assert!(config.add("*PCSE00000", "ux0:tai/game.suprx", never));
        assert_eq!(config.render(), "*KERNEL\r\nur0:tai/a.skprx\r\n*PCSE00000\r\nux0:tai/game.suprx\r\n");
        let mut empty = TaiConfig::parse("");
        assert!(empty.add("*KERNEL", "ur0:tai/a.skprx", never));
        assert_eq!(empty.render(), "*KERNEL\nur0:tai/a.skprx\n");
    }

    #[test]
    fn insert_before_puts_the_line_in_load_order() {
        let mut config = TaiConfig::parse("*KERNEL\nur0:tai/first.skprx\nur0:tai/late.skprx\n");
        assert!(config.add("*KERNEL", "ur0:tai/early.skprx", |p| p.ends_with("late.skprx")));
        assert_eq!(config.render(), "*KERNEL\nur0:tai/first.skprx\nur0:tai/early.skprx\nur0:tai/late.skprx\n");
    }

    #[test]
    fn safe_mode_restores_exactly_what_it_switched_off() {
        let original = "*KERNEL\nur0:tai/a.skprx\n# ur0:tai/off.skprx\n*main\nur0:tai/henkaku.suprx\nur0:tai/b.suprx\n";
        let mut config = TaiConfig::parse(original);
        let disabled = config.disable_all();
        assert_eq!(disabled.len(), 2);
        assert!(config.entries().iter().all(|e| !e.enabled || e.protected));
        assert_eq!(config.restore_enabled(&disabled), 2);
        assert_eq!(config.render(), original);
    }
}
