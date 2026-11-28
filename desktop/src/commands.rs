//! rippl's commands and their own keys: the same names and keys as the GTK
//! build (`shortcut.<name>` in the config moves them on every system). On
//! macOS, Ctrl in a key becomes ⌘, as Mac apps expect, except with Tab
//! (Ctrl+Tab is the Mac's tab switch too).

use rippl_core::keys::{self, Key};

/// (command, its own keys). The order is Settings › Keyboard's.
pub const COMMANDS: &[(&str, &[&str])] = &[
    ("focus-field", &["<Ctrl>l", "F6", "<Alt>d"]),
    ("new-tab", &["<Ctrl>t"]),
    ("private-tab", &["<Ctrl><Shift>n"]),
    ("new-window", &["<Ctrl>n"]),
    ("close-tab", &["<Ctrl>w", "<Ctrl>F4"]),
    ("reopen", &["<Ctrl><Shift>t"]),
    ("next-tab", &["<Ctrl>Page_Down", "<Ctrl>Tab"]),
    ("prev-tab", &["<Ctrl>Page_Up", "<Ctrl><Shift>ISO_Left_Tab"]),
    ("switch-tab", &["<Ctrl>k"]),
    ("back", &["<Alt>Left", "<Ctrl>bracketleft"]),
    ("forward", &["<Alt>Right", "<Ctrl>bracketright"]),
    ("reload", &["<Ctrl>r", "F5"]),
    ("reload-hard", &["<Ctrl><Shift>r", "<Shift>F5"]),
    ("find", &["<Ctrl>f"]),
    ("find-next", &["<Ctrl>g", "F3"]),
    ("find-previous", &["<Ctrl><Shift>g", "<Shift>F3"]),
    ("bookmark", &["<Ctrl>d"]),
    ("history", &["<Ctrl>y", "<Ctrl>h"]),
    ("downloads", &["<Ctrl>j"]),
    ("bookmarks", &["<Ctrl><Shift>o"]),
    ("settings", &["<Ctrl>comma"]),
    ("passwords", &["<Ctrl><Alt>p"]),
    ("pin", &["<Ctrl><Shift>p"]),
    ("duplicate", &["<Ctrl><Shift>d"]),
    ("split", &["<Ctrl><Alt>s"]),
    ("pane-left", &["<Ctrl><Alt>Left"]),
    ("pane-right", &["<Ctrl><Alt>Right"]),
    ("group-site", &["<Ctrl><Alt>g"]),
    ("shield-toggle", &["<Ctrl><Shift>b"]),
    ("site-card", &["<Ctrl>i"]),
    ("reader", &["F9", "<Ctrl><Alt>r"]),
    ("hide-element", &["<Ctrl><Shift>h"]),
    ("unhide", &["<Ctrl><Shift>u"]),
    ("print", &["<Ctrl>p"]),
    ("capture", &["<Ctrl><Shift>s"]),
    ("copy-address", &["<Ctrl><Shift>c"]),
    ("paste-go", &["<Ctrl><Shift>v"]),
    ("grab", &["<Ctrl><Shift>y"]),
    ("ask-page", &["<Ctrl><Shift>a"]),
    ("translate", &["<Ctrl><Shift>l"]),
    ("toggle-column", &["<Ctrl>s"]),
    ("inspector", &["<Ctrl><Shift>i", "F12"]),
    ("zoom-in", &["<Ctrl>plus", "<Ctrl>equal"]),
    ("zoom-out", &["<Ctrl>minus"]),
    ("zoom-reset", &["<Ctrl>0"]),
    ("new-space", &["<Ctrl><Alt>t"]),
    ("fullscreen", &["F11"]),
    ("escape", &["Escape"]),
    ("quit", &["<Ctrl>q"]),
];

/// The keys of the numbered commands: tab-1…9 (Ctrl+n), space-1…9 (Alt+n).
pub fn numbered() -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for n in 1..=9 {
        out.push((format!("tab-{n}"), vec![format!("<Ctrl>{n}")]));
        out.push((format!("space-{n}"), vec![format!("<Alt>{n}")]));
    }
    out
}

/// Every command's keys, yours where set.
pub fn keymap(mine: &[(String, String)]) -> Vec<(String, Key)> {
    let mut all: Vec<(String, Vec<String>)> =
        COMMANDS.iter().map(|(n, k)| (n.to_string(), k.iter().map(|s| s.to_string()).collect())).collect();
    all.extend(numbered());
    let mut out = Vec::new();
    for (name, own) in all {
        let chosen = match mine.iter().find(|(n, _)| *n == name) {
            Some((_, value)) => keys::parse(value).unwrap_or(own),
            None => own,
        };
        for k in chosen {
            if let Some(key) = keys::parse_one(&k) {
                out.push((name.clone(), for_platform(key)));
            }
        }
    }
    out
}

/// Ctrl is ⌘ on macOS (not with Tab).
pub fn for_platform(mut k: Key) -> Key {
    if cfg!(target_os = "macos") && k.ctrl && !matches!(k.name.as_str(), "Tab" | "ISO_Left_Tab") {
        k.ctrl = false;
        k.meta = true;
    }
    k
}

/// The keymap as page.js and chrome.js read it.
pub fn keymap_json(map: &[(String, Key)]) -> serde_json::Value {
    serde_json::Value::Array(
        map.iter()
            .map(|(name, k)| {
                serde_json::json!({
                    "name": name, "key": k.name, "ctrl": k.ctrl, "shift": k.shift, "alt": k.alt, "meta": k.meta,
                })
            })
            .collect(),
    )
}

/// "<Ctrl><Shift>t" as a person reads it on this system.
pub fn label(accel: &str) -> String {
    let Some(k) = keys::parse_one(accel).map(for_platform) else { return accel.to_string() };
    let mac = cfg!(target_os = "macos");
    let mut parts: Vec<String> = Vec::new();
    if k.ctrl {
        parts.push(if mac { "⌃" } else { "Ctrl" }.into());
    }
    if k.alt {
        parts.push(if mac { "⌥" } else { "Alt" }.into());
    }
    if k.shift {
        parts.push(if mac { "⇧" } else { "Shift" }.into());
    }
    if k.meta {
        parts.push(if mac { "⌘" } else { "Win" }.into());
    }
    let name = match k.name.as_str() {
        "comma" => ",".to_string(),
        "plus" => "+".to_string(),
        "minus" => "-".to_string(),
        "equal" => "=".to_string(),
        "bracketleft" => "[".to_string(),
        "bracketright" => "]".to_string(),
        "Page_Down" => "Page Down".to_string(),
        "Page_Up" => "Page Up".to_string(),
        "ISO_Left_Tab" => "Tab".to_string(),
        n if n.chars().count() == 1 => n.to_uppercase(),
        n => n.to_string(),
    };
    parts.push(name);
    parts.join(if mac { "" } else { "+" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_own_key_parses() {
        for (name, own) in COMMANDS {
            for k in *own {
                assert!(keys::parse_one(k).is_some(), "{name}: {k}");
            }
        }
    }

    #[test]
    fn yours_win() {
        let map = keymap(&[("new-tab".into(), "F2".into())]);
        assert!(map.iter().any(|(n, k)| n == "new-tab" && k.name == "F2"));
        assert!(!map.iter().any(|(n, k)| n == "new-tab" && k.name == "t"));
    }
}
