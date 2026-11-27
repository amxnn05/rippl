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
//! # uBlock Origin's lists as well as EasyList's (on, default) or not (off)
//! ublock = on
//! # the column as frosted glass the compositor blurs
//! glass = off
//! # darken pages that have no dark look, in the dark theme (off, default)
//! dusk = off
//! # sites' icons on tabs (on, default) or their letters
//! icons = on
//! # the new tab shows the sites of the bookmark folder "Home" (off, default)
//! home_grid = off
//! # Space and arrow keys on a video with the browser's controls (off, default)
//! video_keys = off
//! # Shift+click a link: its page in a panel over this one (on, default)
//! peek = on
//! # what plays in another tab, at the foot of the column (off, default)
//! player = off
//! # videos start by themselves without sound (muted, default) or wait for
//! # a click (never); a site's card can let it play with sound
//! autoplay = muted
//! # a click of the middle button on a page scrolls it (off, default)
//! autoscroll = off
//! # tabs left unseen this long close themselves, into rippl://archive
//! # (never, default; or 12h, 1d, 7d, 30d)
//! close_after = never
//! # a row of bookmarks above the page (off, default)
//! bookmarks_bar = off
//! # the column with titles (full, default) or as icons only
//! column = full
//! # Ctrl+Tab shows the tabs as pictures, latest first (on, default) or
//! # simply goes to the next tab (off)
//! tab_switcher = on
//! # tabs down the side (side, default) or across the top
//! tabs = side
//! # behind the new tab's field: off (default), a painted wall (open-road,
//! # mountain-lake, northern-lights), solid, gradient (of home_colour), or
//! # image (home_image)
//! home_theme = off
//! home_colour = #4285f5
//! home_image = /home/you/Pictures/wall.jpg
//! # email links open in a webmail: none (default: your mail app), gmail,
//! # outlook, fastmail, proton
//! webmail = none
//! # let a coding agent on this computer use rippl, through `rippl --mcp`
//! # (off, default); with agents_see_tabs, it may read your tabs too
//! agents = off
//! agents_see_tabs = off
//! # the AI add-on (off, default): ask about a page, summarize it, translate
//! # it, with a model running on this computer (Ollama, llama.cpp's
//! # llama-server, LM Studio), through its OpenAI-style API
//! ai = off
//! ai_endpoint = http://127.0.0.1:11434/v1
//! ai_model = llama3.2
//! translate_to = English
//! # sync bookmarks, pins, spaces and settings through a folder that
//! # something else keeps the same on your devices (Syncthing, Nextcloud…);
//! # empty (default): off
//! sync_folder =
//! # where pages and rippl's own requests go out: system (default: the
//! # desktop's proxy settings), direct, or a proxy, e.g. socks5://127.0.0.1:9050
//! # (Tor) or http://proxy.example:3128; this machine and .local stay direct
//! proxy = system
//! # motion: springs, fades, the column's slide, tabs sliding in (full,
//! # default) or everything immediate (reduced)
//! motion = full
//! # pages stretch a little at their top and bottom and spring back (on, default)
//! elastic = on
//! # pages, their animations and smooth scrolling at the screen's own rate
//! # (display, default: 144 on a 144 Hz screen) or near 60 (60, saves power)
//! frame_rate = display
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
//! # suggestions from the search engine while typing (off, default: it
//! # sends each keystroke to the engine)
//! suggestions = off
//! # your own keys for a command (rippl://settings lists them all), or none
//! shortcut.new-tab = <Ctrl>t, <Ctrl>n
//! # your own site keywords: "keyword.<word> = <address with %s>"
//! keyword.npm = https://www.npmjs.com/search?q=%s
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
    pub ublock: bool,
    pub glass: bool,
    pub dusk: bool,
    pub icons: bool,
    pub home_grid: bool,
    pub video_keys: bool,
    /// Ctrl+Shift+Y downloads a video with yt-dlp (grab.rs), and as what.
    pub grab: bool,
    pub grab_format: rippl_core::ytdlp::Format,
    pub peek: bool,
    pub player: bool,
    /// Videos may start by themselves, without sound (true) or not at all.
    pub autoplay: bool,
    pub autoscroll: bool,
    /// Tabs left unseen this long close themselves; None: never.
    pub close_after: Option<Duration>,
    pub bookmarks_bar: bool,
    /// The column as icons only.
    pub icon_column: bool,
    /// Tabs across the top instead of down the side.
    pub top_tabs: bool,
    pub tab_switcher: bool,
    /// Pages at the screen's refresh rate rather than near 60.
    pub full_rate: bool,
    /// Animations (motion.rs), and the elastic edges of pages.
    pub motion_full: bool,
    pub elastic: bool,
    pub proxy: rippl_core::address::Proxy,
    pub webmail: String,
    pub sync_folder: String,
    pub ai: bool,
    pub agents: bool,
    pub agents_see_tabs: bool,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub translate_to: String,
    pub home_theme: String,
    pub home_colour: String,
    pub home_image: String,
    pub sleep_after: Duration,
    pub history: bool,
    pub theme: Theme,
    pub https_only: bool,
    pub gpc: bool,
    pub suggestions: bool,
    /// Your keys for commands: (command, value), `shortcut.<command>`.
    pub shortcuts: Vec<(String, String)>,
    /// Site keywords: the built-in ones, then yours (which win).
    pub keywords: Vec<(String, String)>,
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

/// `close_after`'s choices Settings › Tabs offers them.
pub const CLOSE_CHOICES: [&str; 5] = ["never", "12h", "1d", "7d", "30d"];

fn close_after(value: &str) -> Option<Option<Duration>> {
    let hours = match value {
        "never" => return Some(None),
        "12h" => 12,
        "1d" => 24,
        "7d" => 7 * 24,
        "30d" => 30 * 24,
        _ => return None,
    };
    Some(Some(Duration::from_secs(hours * 3600)))
}

fn close_name(after: Option<Duration>) -> String {
    let Some(d) = after else { return "never".into() };
    CLOSE_CHOICES
        .iter()
        .find(|c| close_after(c) == Some(Some(d)))
        .map_or_else(|| format!("{}s", d.as_secs()), |c| c.to_string())
}

impl Default for Config {
    fn default() -> Config {
        Config {
            search: address::DEFAULT_SEARCH.to_string(),
            shield: true,
            ublock: true,
            glass: false,
            dusk: false,
            icons: true,
            home_grid: false,
            video_keys: false,
            peek: true,
            player: false,
            autoplay: true,
            autoscroll: false,
            close_after: None,
            bookmarks_bar: false,
            icon_column: false,
            top_tabs: false,
            tab_switcher: true,
            full_rate: true,
            motion_full: true,
            elastic: true,
            proxy: rippl_core::address::Proxy::System,
            webmail: "none".into(),
            sync_folder: String::new(),
            ai: false,
            agents: false,
            agents_see_tabs: false,
            ai_endpoint: "http://127.0.0.1:11434/v1".into(),
            ai_model: "llama3.2".into(),
            translate_to: "English".into(),
            home_theme: "off".into(),
            home_colour: "#4285f5".into(),
            home_image: String::new(),
            sleep_after: Duration::from_secs(30 * 60),
            history: true,
            theme: Theme::System,
            https_only: true,
            gpc: true,
            grab: true,
            grab_format: rippl_core::ytdlp::Format::Video,
            suggestions: false,
            keywords: address::default_keywords(),
            shortcuts: Vec::new(),
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
        // For measuring: seconds, not hours.
        if let Some(secs) = std::env::var("RIPPL_CLOSE_AFTER").ok().and_then(|s| s.parse().ok()) {
            config.close_after = Some(Duration::from_secs(secs));
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
            "ublock" => self.ublock = on(value).ok_or_else(bad)?,
            "glass" => self.glass = on(value).ok_or_else(bad)?,
            "dusk" => self.dusk = on(value).ok_or_else(bad)?,
            "icons" => self.icons = on(value).ok_or_else(bad)?,
            "home_grid" => self.home_grid = on(value).ok_or_else(bad)?,
            "video_keys" => self.video_keys = on(value).ok_or_else(bad)?,
            "grab" => self.grab = on(value).ok_or_else(bad)?,
            "grab_format" => self.grab_format = rippl_core::ytdlp::Format::from_key(value).ok_or_else(bad)?,
            "peek" => self.peek = on(value).ok_or_else(bad)?,
            "player" => self.player = on(value).ok_or_else(bad)?,
            "ai" => self.ai = on(value).ok_or_else(bad)?,
            "agents" => self.agents = on(value).ok_or_else(bad)?,
            "agents_see_tabs" => self.agents_see_tabs = on(value).ok_or_else(bad)?,
            "ai_endpoint" => {
                if !(value.starts_with("http://") || value.starts_with("https://")) {
                    return Err(bad());
                }
                self.ai_endpoint = value.trim_end_matches('/').to_string();
            }
            "ai_model" => {
                if value.is_empty() || value.contains(char::is_whitespace) {
                    return Err(bad());
                }
                self.ai_model = value.to_string();
            }
            "translate_to" => {
                if value.is_empty() {
                    return Err(bad());
                }
                self.translate_to = value.to_string();
            }
            "sync_folder" => self.sync_folder = value.to_string(),
            "webmail" => {
                if !rippl_core::mail::WEBMAILS.contains(&value) {
                    return Err(bad());
                }
                self.webmail = value.to_string();
            }
            "proxy" => self.proxy = address::proxy(value).ok_or_else(bad)?,
            "home_theme" => {
                if !crate::home_theme::THEMES.contains(&value) {
                    return Err(bad());
                }
                self.home_theme = value.to_string();
            }
            "home_colour" => {
                if !rippl_core::tabs::colour_ok(value) {
                    return Err(bad());
                }
                self.home_colour = value.to_lowercase();
            }
            "home_image" => self.home_image = value.to_string(),
            "motion" => {
                self.motion_full = match value {
                    "full" => true,
                    "reduced" => false,
                    _ => return Err(bad()),
                }
            }
            "elastic" => self.elastic = on(value).ok_or_else(bad)?,
            "frame_rate" => {
                self.full_rate = match value {
                    "display" => true,
                    "60" => false,
                    _ => return Err(bad()),
                }
            }
            "tab_switcher" => self.tab_switcher = on(value).ok_or_else(bad)?,
            "autoscroll" => self.autoscroll = on(value).ok_or_else(bad)?,
            "bookmarks_bar" => self.bookmarks_bar = on(value).ok_or_else(bad)?,
            "autoplay" => {
                self.autoplay = match value {
                    "muted" => true,
                    "never" => false,
                    _ => return Err(bad()),
                }
            }
            "close_after" => self.close_after = close_after(value).ok_or_else(bad)?,
            "column" => {
                self.icon_column = match value {
                    "full" => false,
                    "icons" => true,
                    _ => return Err(bad()),
                }
            }
            "tabs" => {
                self.top_tabs = match value {
                    "side" => false,
                    "top" => true,
                    _ => return Err(bad()),
                }
            }
            "history" => self.history = on(value).ok_or_else(bad)?,
            "https_only" => self.https_only = on(value).ok_or_else(bad)?,
            "gpc" => self.gpc = on(value).ok_or_else(bad)?,
            "suggestions" => self.suggestions = on(value).ok_or_else(bad)?,
            k if k.starts_with("shortcut.") => {
                let name = k["shortcut.".len()..].trim().to_string();
                self.shortcuts.retain(|(n, _)| *n != name);
                if !value.is_empty() {
                    crate::shortcuts::parse(value)?;
                    self.shortcuts.push((name, value.to_string()));
                }
            }
            k if k.starts_with("keyword.") => {
                let word = k["keyword.".len()..].trim().to_lowercase();
                if word.is_empty() || word.contains(char::is_whitespace) || !address::keyword_template_ok(value) {
                    return Err(bad());
                }
                self.keywords.retain(|(w, _)| *w != word);
                self.keywords.insert(0, (word, value.to_string()));
            }
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
            choice(
                "ublock",
                "uBlock Origin's filter lists too",
                onoff(self.ublock),
                &["on", "off"],
                "Applies after a restart",
            ),
            choice("glass", "Glass column (the compositor blurs behind it)", onoff(self.glass), &["on", "off"], ""),
            choice("dusk", "Darken pages that have no dark look", onoff(self.dusk), &["on", "off"], "In the dark theme"),
            choice("icons", "Sites' icons on tabs", onoff(self.icons), &["on", "off"], "Off: their letters"),
            choice("peek", "Shift+click a link to peek at it", onoff(self.peek), &["on", "off"], "Esc puts it away; Ctrl+Return keeps it as a tab"),
            choice("player", "What plays in another tab, at the foot of the column", onoff(self.player), &["on", "off"], "With its pause"),
            choice("video_keys", "Space and ← → control videos", onoff(self.video_keys), &["on", "off"], "Videos with the browser's own controls, once clicked"),
            choice("grab", "Ctrl+Shift+Y downloads a video", onoff(self.grab), &["on", "off"], "Paste a link from YouTube, Vimeo and a thousand other sites; it goes to Downloads through yt-dlp, which must be installed"),
            choice("grab_format", "Return downloads", self.grab_format.key().into(), &["video", "audio"], "Video: the best quality, as .mp4. Audio: only the sound, as .mp3. Shift+Return gives the other"),
            choice("agents", "Let a coding agent use rippl", onoff(self.agents), &["on", "off"], "Claude Code, Codex…: add `rippl --mcp` as an MCP server. It works in tabs of its own, marked ✦; never on the network"),
            choice("agents_see_tabs", "Let it read my open tabs", onoff(self.agents_see_tabs), &["on", "off"], "Read only: it never clicks or types in your tabs, and never sees private ones"),
            choice("ai", "The AI add-on", onoff(self.ai), &["on", "off"], "Ask about a page (Ctrl+Shift+A), summarize it, translate it (Ctrl+Shift+L). Nothing is sent until you ask"),
            choice("ai_endpoint", "Where the model runs", self.ai_endpoint.clone(), &[], "Its OpenAI-style address. Ollama: http://127.0.0.1:11434/v1; llama-server: http://127.0.0.1:8080/v1. Another computer's address sends it the page you ask about"),
            choice("ai_model", "Model", self.ai_model.clone(), &[], "As the server names it: llama3.2, qwen2.5, gemma3…"),
            choice("translate_to", "Translate pages into", self.translate_to.clone(), &rippl_core::ai::LANGUAGES, ""),
            choice("sync_folder", "Sync through this folder", self.sync_folder.clone(), &[], "Bookmarks, pinned tabs, spaces and settings; never history, passwords or sign-ins. Any folder Syncthing, Nextcloud or a shared disk keeps the same on your devices; empty: off"),
            choice("webmail", "Email links open in", self.webmail.clone(), &rippl_core::mail::WEBMAILS, "none: your mail app"),
            choice("proxy", "Proxy", match &self.proxy {
                address::Proxy::System => "system".into(),
                address::Proxy::Direct => "direct".into(),
                address::Proxy::Through(u) => u.clone(),
            }, &[], "system, direct, or socks5://host:port, http://host:port. Everything goes through it: pages and rippl's own requests; this machine and .local stay direct"),
            choice("autoplay", "Videos start by themselves", if self.autoplay { "muted" } else { "never" }.into(), &["muted", "never"], "Muted: without sound, until you let a site play sound from its card. Never: they wait for a click"),
            choice("autoscroll", "Scroll with the middle button", onoff(self.autoscroll), &["on", "off"], "A click of the wheel on a page, then move the pointer"),
            choice("close_after", "Tabs left alone close themselves after", close_name(self.close_after), &CLOSE_CHOICES, "Not pinned tabs, tabs in a group, or kept-awake sites. rippl://archive keeps them a month"),
            choice("motion", "Animations", if self.motion_full { "full" } else { "reduced" }.into(), &["full", "reduced"], "Springs, fades, slides. Reduced: everything immediate"),
            choice("elastic", "Pages stretch at their edges", onoff(self.elastic), &["on", "off"], "Scrolled past the top or bottom, a page gives a little and springs back"),
            choice("frame_rate", "Pages and scrolling", if self.full_rate { "display" } else { "60" }.into(), &["display", "60"], "Display: at your screen's refresh rate (144 Hz gets 144). 60: less power on battery"),
            choice("tab_switcher", "Ctrl+Tab shows the tabs as pictures", onoff(self.tab_switcher), &["on", "off"], "Latest first. Off: the next tab in the column"),
            choice("bookmarks_bar", "Bookmarks bar above the page", onoff(self.bookmarks_bar), &["on", "off"], ""),
            choice("tabs", "Tabs", if self.top_tabs { "top" } else { "side" }.into(), &["side", "top"], "Down the side, or across the top"),
            choice("column", "The column", if self.icon_column { "icons" } else { "full" }.into(), &["full", "icons"], "With titles, or icons only"),
            choice("home_theme", "New tab background", self.home_theme.clone(), &crate::home_theme::THEMES, "Painted walls are the; solid and gradient use the colour below; image uses the file below"),
            choice("home_colour", "New tab colour", self.home_colour.clone(), &[], "#rrggbb, for solid and gradient"),
            choice("home_image", "New tab image", self.home_image.clone(), &[], "A picture's full path, for image"),
            choice("home_grid", "New tab: the sites in the bookmark folder Home", onoff(self.home_grid), &["on", "off"], "Four to a row, sixteen at most"),
            choice("history", "Keep history", onoff(self.history), &["on", "off"], ""),
            choice(
                "suggestions",
                "Search engine suggestions while typing",
                onoff(self.suggestions),
                &["on", "off"],
                "Sends what you type to the engine, letter by letter",
            ),
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
        assert!(c.apply("keyword.npm", "https://www.npmjs.com/search?q=%s").is_ok());
        assert_eq!(c.keywords[0].0, "npm");
        assert!(c.apply("close_after", "7d").is_ok());
        assert_eq!(c.close_after, Some(Duration::from_secs(7 * 86400)));
        assert_eq!(close_name(c.close_after), "7d");
        assert!(c.apply("close_after", "3d").is_err());
        assert!(c.apply("autoplay", "never").is_ok() && !c.autoplay);
        assert!(c.apply("tabs", "top").is_ok() && c.top_tabs);
        assert!(c.apply("keyword.evil", "https://%s.evil.com/").is_err());
    }
}
