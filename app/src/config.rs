//! `~/.config/rippl/config`: the few things there are to set, as
//! `key = value` lines. Missing file or key: the default. rippl://settings
//! changes them in place (`set`), keeping your comments and other lines.
//!
//! ```text
//! # duckduckgo (default), brave, startpage, mojeek, qwant,
//! # or your own address with {} where the words go, e.g. a SearXNG:
//! # search = https://searx.example.org/search?q={}
//! search = duckduckgo
//! # on (default) or off
//! shield = on
//! # minutes a tab sits unseen before it sleeps (default 30)
//! sleep = 30
//! # keep history (on, default) or not (off); bookmarks are kept either way
//! history = on
//! # system (default), light or dark: the window, and pages that offer both
//! theme = system
//! # upgrade every http request to https (on, default) or not (off)
//! https_only = on
//! # send Global Privacy Control, "do not sell or share" (on, default)
//! gpc = on
//! ```

use std::path::PathBuf;
use std::time::Duration;

use rippl_core::address;
use rippl_core::pages::Setting;
use webkit6::glib;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Clone)]
pub struct Config {
    /// The search template, `{}` where the words go.
    pub search: String,
    pub shield: bool,
    pub sleep_after: Duration,
    pub history: bool,
    pub theme: Theme,
    pub https_only: bool,
    pub gpc: bool,
}

pub fn path() -> PathBuf {
    glib::user_config_dir().join("rippl").join("config")
}

fn on(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            search: address::DEFAULT_SEARCH.to_string(),
            shield: true,
            sleep_after: Duration::from_secs(30 * 60),
            history: true,
            theme: Theme::System,
            https_only: true,
            gpc: true,
        }
    }
}

impl Config {
    pub fn load() -> Config {
        let text = std::fs::read_to_string(path()).unwrap_or_default();
        let mut config = Config::default();
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            let Some((key, value)) = line.split_once('=') else { continue };
            if let Err(e) = config.apply(key.trim(), value.trim()) {
                eprintln!("rippl: config: {e}");
            }
        }
        // For measuring: seconds, not minutes.
        if let Some(secs) = std::env::var("RIPPL_SLEEP_AFTER").ok().and_then(|s| s.parse().ok()) {
            config.sleep_after = Duration::from_secs(secs);
        }
        config
    }

    /// Takes one setting, checked. The error says what was wrong.
    pub fn apply(&mut self, key: &str, value: &str) -> Result<(), String> {
        let bad = || format!("{value:?} isn't a value for {key}");
        match key {
            "search" => self.search = address::engine(value).ok_or_else(bad)?,
            "shield" => self.shield = on(value).ok_or_else(bad)?,
            "history" => self.history = on(value).ok_or_else(bad)?,
            "https_only" => self.https_only = on(value).ok_or_else(bad)?,
            "gpc" => self.gpc = on(value).ok_or_else(bad)?,
            "sleep" => {
                let minutes: u64 = value.parse().ok().filter(|m| (1..=24 * 60).contains(m)).ok_or_else(bad)?;
                self.sleep_after = Duration::from_secs(minutes * 60);
            }
            "theme" => {
                self.theme = match value {
                    "system" => Theme::System,
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => return Err(bad()),
                }
            }
            other => return Err(format!("unknown setting {other:?}")),
        }
        Ok(())
    }

    /// Changes one setting and writes it to the file.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        self.apply(key, value)?;
        let file = path();
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let new = rippl_core::settings_file::set(&text, key, value);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = file.with_extension("tmp");
        std::fs::write(&tmp, new).and_then(|_| std::fs::rename(&tmp, &file)).map_err(|e| e.to_string())
    }

    fn search_name(&self) -> String {
        address::ENGINES.iter().find(|(_, t)| *t == self.search).map_or(self.search.clone(), |(n, _)| n.to_string())
    }

    /// The settings as rippl://settings shows them, each with its choices.
    pub fn rows(&self) -> Vec<Setting> {
        let onoff = |b: bool| if b { "on" } else { "off" }.to_string();
        let choice = |key: &str, label: &str, value: String, choices: &[&str], note: &str| Setting {
            key: key.into(),
            label: label.into(),
            value,
            choices: choices.iter().map(|c| c.to_string()).collect(),
            note: note.into(),
        };
        let mut engines: Vec<&str> = address::ENGINES.iter().map(|(n, _)| *n).collect();
        engines.sort();
        vec![
            choice("search", "Search engine", self.search_name(), &engines, ""),
            choice("https_only", "HTTPS-only", onoff(self.https_only), &["on", "off"], "For new pages"),
            choice("gpc", "Global Privacy Control", onoff(self.gpc), &["on", "off"], "For new pages"),
            choice("shield", "Ad and tracker blocking", onoff(self.shield), &["on", "off"], "Applies after a restart"),
            choice("history", "Keep history", onoff(self.history), &["on", "off"], ""),
            choice("theme", "Theme", format!("{:?}", self.theme).to_lowercase(), &["system", "light", "dark"], ""),
            choice("sleep", "Tabs sleep after (minutes)", (self.sleep_after.as_secs() / 60).to_string(), &[], ""),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked() {
        let mut c = Config::default();
        assert!(c.apply("search", "brave").is_ok());
        assert!(c.apply("search", "http://evil/?q={}").is_err(), "https only");
        assert!(c.apply("shield", "maybe").is_err());
        assert!(c.apply("sleep", "0").is_err());
        assert!(c.apply("sleep", "15").is_ok());
        assert_eq!(c.sleep_after, Duration::from_secs(900));
        assert!(c.apply("nonsense", "1").is_err());
        assert!(c.apply("https_only", "off").is_ok() && !c.https_only);
    }
}
