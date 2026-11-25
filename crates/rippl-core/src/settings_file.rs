//! Changing one line of `~/.config/rippl/config` without disturbing the
//! rest: comments, blank lines and the other settings stay as they were.

/// `text` with `key` set to `value`: the first active `key = …` line is
/// replaced, or, if there is none, `key = value` is added at the end.
pub fn set(text: &str, key: &str, value: &str) -> String {
    let mut done = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|line| {
            let active = !line.trim_start().starts_with('#');
            let this_key = line.split_once('=').map(|(k, _)| k.trim());
            if !done && active && this_key == Some(key) {
                done = true;
                format!("{key} = {value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !done {
        out.push(format!("{key} = {value}"));
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_in_place_and_keeps_the_rest() {
        let text = "# my settings\nsearch = duckduckgo\n\n# search = brave\nshield = on\n";
        assert_eq!(set(text, "search", "brave"), "# my settings\nsearch = brave\n\n# search = brave\nshield = on\n");
    }

    #[test]
    fn appends_when_missing() {
        assert_eq!(set("", "theme", "dark"), "theme = dark\n");
        assert_eq!(set("shield = on", "theme", "dark"), "shield = on\ntheme = dark\n");
    }
}
