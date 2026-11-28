//! Keyboard shortcuts written as GTK writes them (`<Ctrl><Shift>t`, `F5`,
//! `<Alt>Left`), read without GTK, so the macOS and Windows builds use the
//! same names in the config and on Settings. `<Primary>` is Ctrl, or ⌘ on
//! macOS.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// ⌘ on macOS, the Windows key elsewhere.
    pub meta: bool,
    /// The key, as GTK names it: a lowercase letter or digit, `F5`,
    /// `Return`, `Left`, `Page_Down`, `plus`, …
    pub name: String,
}

/// Keys with names, beyond letters, digits and F1–F24.
pub const NAMED: &[&str] = &[
    "Return", "Tab", "ISO_Left_Tab", "Escape", "BackSpace", "Delete", "Insert", "Home", "End", "Page_Up",
    "Page_Down", "Left", "Right", "Up", "Down", "space", "plus", "minus", "equal", "comma", "period", "slash",
    "backslash", "semicolon", "apostrophe", "grave", "bracketleft", "bracketright", "KP_Add", "KP_Subtract",
    "KP_0", "KP_Enter", "AudioPlay", "AudioPause", "AudioNext", "AudioPrev", "AudioStop", "Back", "Forward",
    "Refresh", "Find", "Menu",
];

/// One key, `None` if it isn't one.
pub fn parse_one(text: &str) -> Option<Key> {
    let mut k = Key { ctrl: false, shift: false, alt: false, meta: false, name: String::new() };
    let mut rest = text.trim();
    while let Some(r) = rest.strip_prefix('<') {
        let end = r.find('>')?;
        match r[..end].to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => k.ctrl = true,
            "shift" => k.shift = true,
            "alt" | "mod1" => k.alt = true,
            "super" | "meta" | "cmd" => k.meta = true,
            _ => return None,
        }
        rest = &r[end + 1..];
    }
    let name = rest.trim();
    let mut chars = name.chars();
    let ok = match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_alphanumeric(),
        _ => {
            NAMED.contains(&name)
                || name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=24).contains(&n))
        }
    };
    if !ok {
        return None;
    }
    k.name = if name.chars().count() == 1 { name.to_ascii_lowercase() } else { name.to_string() };
    Some(k)
}

/// A setting's keys: comma-separated, or `none` for no keys at all.
pub fn parse(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if value == "none" {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| match parse_one(k) {
            Some(_) => Ok(k.to_string()),
            None => Err(format!("{k:?} isn't a key rippl knows (write it as <Ctrl><Shift>x, F2, …)")),
        })
        .collect()
}

/// A browser's `KeyboardEvent.key` as GTK names it: "ArrowLeft" → "Left",
/// "Enter" → "Return", "a" → "a", "+" → "plus".
pub fn from_dom(key: &str) -> String {
    match key {
        "Enter" => "Return",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "PageUp" => "Page_Up",
        "PageDown" => "Page_Down",
        "Backspace" => "BackSpace",
        "Esc" => "Escape",
        " " => "space",
        "+" => "plus",
        "-" => "minus",
        "=" => "equal",
        "," => "comma",
        "." => "period",
        "/" => "slash",
        "\\" => "backslash",
        ";" => "semicolon",
        "'" => "apostrophe",
        "`" => "grave",
        "[" => "bracketleft",
        "]" => "bracketright",
        "MediaPlayPause" => "AudioPlay",
        "MediaTrackNext" => "AudioNext",
        "MediaTrackPrevious" => "AudioPrev",
        "BrowserBack" => "Back",
        "BrowserForward" => "Forward",
        k if k.chars().count() == 1 => return k.to_lowercase(),
        k => k,
    }
    .to_string()
}

impl Key {
    /// Does a key press match? `shift` is ignored for keys whose shifted
    /// form has its own name (`plus` is Shift+= on most layouts).
    pub fn matches(&self, ctrl: bool, shift: bool, alt: bool, meta: bool, name: &str) -> bool {
        let name_eq = self.name.eq_ignore_ascii_case(name)
            || (self.name == "ISO_Left_Tab" && name == "Tab" && shift);
        let shift_ok = self.shift == shift || matches!(self.name.as_str(), "plus" | "ISO_Left_Tab");
        name_eq && self.ctrl == ctrl && self.alt == alt && self.meta == meta && shift_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gtk_names() {
        let k = parse_one("<Ctrl><Shift>t").unwrap();
        assert!(k.ctrl && k.shift && !k.alt && k.name == "t");
        assert_eq!(parse_one("<Primary>L").unwrap().name, "l");
        assert_eq!(parse_one("F5").unwrap().name, "F5");
        assert!(parse_one("<Alt>Left").unwrap().alt);
        assert!(parse_one("<Hyper>x").is_none());
        assert!(parse_one("F25").is_none());
        assert!(parse_one("Wobble").is_none());
        assert_eq!(parse("none").unwrap(), Vec::<String>::new());
        assert!(parse("<Ctrl>j, F2").is_ok());
        assert!(parse("<Ctrl>j, nope").is_err());
    }

    #[test]
    fn matches_a_dom_key() {
        let k = parse_one("<Ctrl>plus").unwrap();
        assert!(k.matches(true, true, false, false, &from_dom("+")));
        let back = parse_one("<Alt>Left").unwrap();
        assert!(back.matches(false, false, true, false, &from_dom("ArrowLeft")));
        assert!(!back.matches(true, false, true, false, "Left"));
        let t = parse_one("<Ctrl>t").unwrap();
        assert!(t.matches(true, false, false, false, &from_dom("T")));
    }
}
