//! `~/.config/rippl/config`: the few things there are to set, as
//! `key = value` lines. Missing file or key: the default.
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
//! ```

use std::time::Duration;

use rippl_core::address;
use webkit6::glib;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

pub struct Config {
    /// The search template, `{}` where the words go.
    pub search: String,
    pub shield: bool,
    pub sleep_after: Duration,
    pub history: bool,
    pub theme: Theme,
}

impl Config {
    pub fn load() -> Config {
        let path = glib::user_config_dir().join("rippl").join("config");
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut config = Config {
            search: address::DEFAULT_SEARCH.to_string(),
            shield: true,
            sleep_after: Duration::from_secs(30 * 60),
            history: true,
            theme: Theme::System,
        };
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "search" => match address::engine(value) {
                    Some(template) => config.search = template,
                    None => eprintln!("rippl: config: unknown search engine {value:?}, using DuckDuckGo"),
                },
                "shield" => config.shield = value != "off",
                "sleep" => {
                    if let Ok(minutes) = value.parse::<u64>() {
                        config.sleep_after = Duration::from_secs(minutes.max(1) * 60);
                    }
                }
                "history" => config.history = value != "off",
                "theme" => {
                    config.theme = match value {
                        "dark" => Theme::Dark,
                        "light" => Theme::Light,
                        _ => Theme::System,
                    }
                }
                other => eprintln!("rippl: config: unknown setting {other:?}"),
            }
        }
        // For measuring: seconds, not minutes.
        if let Some(secs) = std::env::var("RIPPL_SLEEP_AFTER").ok().and_then(|s| s.parse().ok()) {
            config.sleep_after = Duration::from_secs(secs);
        }
        config
    }
}
