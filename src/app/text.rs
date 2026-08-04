
fn is_renderable(c: char) -> bool {
    match c {
        '\n' => true,
        ' '..='~' => true,
        '\u{00a1}'..='\u{00ff}' => true,
        '\u{0100}'..='\u{017f}' => true,
        '\u{2018}' | '\u{2019}' | '\u{201c}' | '\u{201d}' => true, 
        '\u{2013}' | '\u{2014}' => true,                           
        '\u{2026}' => true,                                        
        '\u{2022}' => true,                                        
        _ => false,
    }
}
fn is_clean(text: &str) -> bool {
    let mut previous_space = false;
    for c in text.chars() {
        if !is_renderable(c) || c == '&' {
            return false;
        }
        let space = c == ' ';
        if space && previous_space {
            return false;
        }
        previous_space = space;
    }
    text.len() == text.trim().len()
}
fn decode_entity(rest: &str) -> Option<(char, usize)> {
    let end = rest[1..].find(';')? + 1;
    let body = &rest[1..end];
    let decoded = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" | "#39" | "#x27" | "#X27" => '\'',
        "nbsp" | "#160" => ' ',
        numeric if numeric.starts_with('#') => {
            let (digits, radix) = match numeric.as_bytes().get(1) {
                Some(b'x') | Some(b'X') => (&numeric[2..], 16),
                _ => (&numeric[1..], 10),
            };
            char::from_u32(u32::from_str_radix(digits, radix).ok()?)?
        }
        _ => return None,
    };
    Some((decoded, end + 1))
}
pub fn sanitize(text: &mut String) {
    if is_clean(text) {
        return;
    }
    *text = sanitized(text);
}
pub fn sanitized(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let mut width = c.len_utf8();
        let replacement = match c {
            '&' => match decode_entity(rest) {
                Some((decoded, consumed)) => {
                    width = consumed;
                    Some(decoded)
                }
                None => Some('&'),
            },
            '\t' | '\r' | '\u{00a0}' => Some(' '),
            '\u{00ad}' | '\u{200b}'..='\u{200d}' | '\u{feff}' | '\u{fe0e}' | '\u{fe0f}'
            | '\u{1f3fb}'..='\u{1f3ff}' => None,
            c if is_renderable(c) => Some(c),
            _ => None,
        };
        rest = &rest[width..];
        let Some(c) = replacement else { continue };
        if c == ' ' && matches!(out.chars().next_back(), None | Some(' ') | Some('\n')) {
            continue;
        }
        out.push(c);
    }
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out
}
pub fn display_name<'a>(name: &'a str, original: Option<&'a str>, title_id: &'a str) -> &'a str {
    for candidate in [name, original.unwrap_or(""), title_id] {
        if !candidate.trim().is_empty() {
            return candidate;
        }
    }
    "Unknown"
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drops_the_variation_selector_that_drew_a_box() {
        // "⚠️" is U+26A0 U+FE0F: the fonts have the sign but not the selector.
        assert_eq!(sanitized("⚠️ Requiere NoNpDrm"), "Requiere NoNpDrm");
    }
    #[test]
    fn keeps_the_line_breaks_in_a_requirements_list() {
        let requirements = "- libshacccg.suprx\n- kubridge.skprx\n- Game Data Files: Android";
        assert_eq!(sanitized(requirements), requirements);
    }
    #[test]
    fn a_cjk_title_sanitizes_away_and_falls_back() {
        let name = sanitized("シェルノサージュ");
        assert!(name.is_empty(), "got {name:?}");
        assert_eq!(display_name(&name, None, "PCSG00284"), "PCSG00284");
        // A mixed title keeps its readable half.
        assert_eq!(sanitized("サムライ Vita"), "Vita");
    }
    #[test]
    fn is_idempotent_and_leaves_clean_text_alone() {
        let mut text = "Port of Grand Theft Auto: San Andreas for PSVITA.".to_owned();
        let before = text.as_ptr();
        sanitize(&mut text);
        assert_eq!(text.as_ptr(), before, "clean text should not be reallocated");
        let mut dirty = "Rinnegatamante &amp; TheFloW 🚀".to_owned();
        sanitize(&mut dirty);
        assert_eq!(dirty, "Rinnegatamante & TheFloW");
        let once = dirty.clone();
        sanitize(&mut dirty);
        assert_eq!(dirty, once);
    }
    #[test]
    fn expands_entities_and_keeps_accents() {
        assert_eq!(sanitized("Pok&eacute;mon"), "Pok&eacute;mon"); // unknown entity is left alone
        assert_eq!(sanitized("A &lt;b&gt; C &#39;quoted&#39;"), "A <b> C 'quoted'");
        assert_eq!(sanitized("Emulación española"), "Emulación española");
        assert_eq!(sanitized("día&nbsp;a&nbsp;día"), "día a día");
    }
    #[test]
    fn collapses_the_gaps_left_by_removed_characters() {
        assert_eq!(sanitized("Metal 🎮 Slug 🎮 X"), "Metal Slug X");
        assert_eq!(sanitized("  padded 🚀  "), "padded");
    }
}
