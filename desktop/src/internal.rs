//! rippl's own pages (`rippl://history`, `bookmarks`, `downloads`,
//! `settings`, `archive`, `welcome`), built by `rippl_core::pages` exactly
//! as in the GTK build. A button is a visit to `rippl://<page>?do=…` and
//! acts only with this launch's token.

use rippl_core::import;
use rippl_core::pages::{self, Setting, SettingsView};
use rippl_core::places::now;

use crate::browser::Browser;
use crate::commands;
use crate::platform;

fn other_browsers() -> Vec<import::Source> {
    import::sources(&rippl_core::dirs::home())
}

impl Browser {
    pub fn serve(&self, uri: &str) -> String {
        let url = platform::from_engine(uri);
        let page = pages::page_name(&url);
        let act = pages::param(&url, "do");
        let arg = pages::param(&url, "arg").unwrap_or_default();
        let token_ok = pages::param(&url, "token").as_deref() == Some(self.token.as_str());
        let mut note = String::new();
        if let Some(act) = act.as_deref() {
            if token_ok {
                note = self.act(&page, act, &arg);
            } else {
                eprintln!("rippl: refused an action without this launch's token: {act}");
            }
        }
        let q = pages::param(&url, "q").unwrap_or_default();
        let html = match page.as_str() {
            "history" => {
                let places =
                    if q.trim().is_empty() { self.places.recent(300).unwrap_or_default() } else { self.places.search(&q, 300).unwrap_or_default() };
                pages::history(&places, &q, now(), &self.token)
            }
            "downloads" => pages::downloads(&self.downloads.running(), &self.downloads.kept(), now(), &self.token),
            "welcome" => pages::welcome(platform::is_default_browser(), &self.token),
            "archive" => {
                let closing = self.config.borrow().close_after.is_some();
                pages::archive(&self.places.archived(&q).unwrap_or_default(), &q, closing, now(), &self.token)
            }
            "bookmarks" => {
                let labels: Vec<(String, bool)> = other_browsers()
                    .iter()
                    .map(|s| (format!("{} · {}", s.browser, s.profile), s.kind == import::Kind::Chromium))
                    .collect();
                pages::bookmarks(&self.places.bookmarks().unwrap_or_default(), &labels, &note, now(), &self.token)
            }
            "passwords" => {
                let accounts = crate::passwords::accounts();
                let never = self.places.never_save_list().unwrap_or_default();
                pages::passwords(&accounts, &never, crate::passwords::STORE, &self.token)
            }
            _ => {
                let permissions = self.places.permissions().unwrap_or_default();
                let shield_off = self.shield.allowed_list();
                let hidden = self.places.hidden().unwrap_or_default();
                let config = self.config.borrow().rows();
                let shortcuts = self.shortcut_rows();
                let about = vec![
                    ("Version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
                    ("Engine".to_string(), platform::engine_name()),
                    ("Your data".to_string(), rippl_core::dirs::data().display().to_string()),
                    ("Source".to_string(), "github.com/amxnn05/rippl".to_string()),
                ];
                let path = rippl_core::config::path();
                pages::settings(
                    &SettingsView {
                        permissions: &permissions,
                        shield_off: &shield_off,
                        hidden: &hidden,
                        config: &config,
                        shortcuts: &shortcuts,
                        about: &about,
                        default_browser: platform::is_default_browser(),
                        rail_in_column: false,
                        config_path: &path.to_string_lossy(),
                        note: &note,
                    },
                    &self.token,
                )
            }
        };
        platform::internal_html(html)
    }

    fn shortcut_rows(&self) -> Vec<Setting> {
        let config = self.config.borrow();
        let mut names: Vec<(String, Vec<String>)> =
            commands::COMMANDS.iter().map(|(n, k)| (n.to_string(), k.iter().map(|s| s.to_string()).collect())).collect();
        names.retain(|(n, _)| n != "escape");
        names
            .into_iter()
            .map(|(name, own)| {
                let mine = config.shortcuts.iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone());
                let value = mine.clone().unwrap_or_else(|| own.join(", "));
                let shown: Vec<String> = value.split(',').map(|k| commands::label(k.trim())).collect();
                Setting {
                    key: format!("shortcut.{name}"),
                    label: label(&name),
                    value: if value == "none" { "none".into() } else { value.clone() },
                    choices: Vec::new(),
                    note: if mine.is_some() {
                        format!("Yours ({}); its own: {}", shown.join(", "), own.join(", "))
                    } else {
                        shown.join(", ")
                    },
                }
            })
            .collect()
    }

    /// A button on one of the pages. Returns a note to show, if any.
    fn act(&self, page: &str, act: &str, arg: &str) -> String {
        let result: Result<String, String> = match (page, act) {
            ("downloads", "clear") => {
                self.downloads.clear_kept();
                Ok(String::new())
            }
            ("downloads", "forget") => {
                self.downloads.forget_kept(arg);
                Ok(String::new())
            }
            ("downloads", act) => match self.downloads.find_kept(arg) {
                None => Err("That download isn't on the list any more".into()),
                Some(e) => match act {
                    "open" => platform::open(std::path::Path::new(&e.path)),
                    "folder" => platform::show_in_folder(std::path::Path::new(&e.path)),
                    "again" => {
                        self.open(&e.url, false, true);
                        Ok(())
                    }
                    _ => Err(format!("unknown action {act}")),
                }
                .map(|_| String::new()),
            },
            ("history", "forget") => self.places.forget(arg).map(|_| String::new()).map_err(|e| e.to_string()),
            ("history", "clear") => self.places.clear_history(None).map(|_| String::new()).map_err(|e| e.to_string()),
            ("archive", "forget") => self.places.unarchive(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string()),
            ("archive", "clear") => self.places.unarchive(None).map(|_| String::new()).map_err(|e| e.to_string()),
            ("bookmarks", "remove") => {
                self.render_due.set(true);
                self.places.remove_bookmark(arg).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("bookmarks", "move") => {
                let (url, folder) = arg.split_once('\n').unwrap_or((arg, ""));
                self.places.move_bookmark(url, folder).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("bookmarks", "import") => self.import_from(arg.parse().unwrap_or(usize::MAX)),
            ("bookmarks", "import-tabs") => self.import_tabs(arg.parse().unwrap_or(usize::MAX)),
            ("bookmarks", "import-passwords") => Err("Bringing passwords over comes in a later update on this system".into()),
            ("passwords", "delete") => {
                let (origin, username) = arg.split_once('\n').unwrap_or((arg, ""));
                crate::passwords::delete(origin, username).map(|_| String::new())
            }
            ("passwords", "allow-saving") => {
                self.places.set_never_save(arg, false).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("settings", "set") => {
                let (key, value) = arg.split_once('=').unwrap_or((arg, ""));
                self.change_setting(key, value)
            }
            ("settings", "record-start") | ("settings", "record-stop") => Ok(String::new()),
            ("settings", "make-default") | ("welcome", "make-default") => platform::make_default_browser(),
            ("welcome", "start") => {
                self.after(|b| {
                    if let Some(id) = b.front() {
                        b.close(id);
                    }
                });
                Ok(String::new())
            }
            ("settings", "clear-history") => {
                self.places.clear_history(None).map(|_| "History cleared.".to_string()).map_err(|e| e.to_string())
            }
            ("settings", "forget-permissions") => {
                self.places.forget_permissions(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("settings", "shield-on") => {
                self.after({
                    let host = arg.to_string();
                    move |b| b.set_shield_for(&host, false)
                });
                Ok(String::new())
            }
            ("settings", "unhide") => {
                let r = self.places.unhide(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string());
                self.after(|b| b.apply_site_styles());
                r
            }
            _ => Err(format!("unknown action {act}")),
        };
        result.unwrap_or_else(|e| {
            eprintln!("rippl: {page} {act}: {e}");
            format!("That didn't work: {e}")
        })
    }

    pub fn change_setting(&self, key: &str, value: &str) -> Result<String, String> {
        self.config.borrow_mut().set(key, value)?;
        let config = self.config.borrow().clone();
        match key {
            "theme" => self.after(|b| b.apply_theme()),
            "history" => self.downloads.keep_list.set(config.history),
            k if k.starts_with("shortcut.") => self.after(|b| b.apply_shortcuts()),
            "shield" | "ublock" => return Ok("Blocking changes after rippl restarts.".into()),
            "peek" | "video_keys" | "autoscroll" | "elastic" | "autoplay" => {
                return Ok("Pages opened from now on have it; reload a page to change it there.".into());
            }
            _ => {}
        }
        self.render_due.set(true);
        Ok(String::new())
    }

    fn import_from(&self, index: usize) -> Result<String, String> {
        let sources = other_browsers();
        let source = sources.get(index).ok_or("that browser isn't there any more")?;
        let scratch = rippl_core::dirs::cache().join("import");
        let list = import::read(source, &scratch)?;
        let added = self.places.import_bookmarks(&list).map_err(|e| e.to_string())?;
        let history = if self.config.borrow().history {
            match rippl_core::import_more::history(source, &scratch, 20_000) {
                Ok(pages) => {
                    let new = self.places.import_history(&pages).map_err(|e| e.to_string())?;
                    format!(" and {new} new page{} of history", if new == 1 { "" } else { "s" })
                }
                Err(e) => format!(" (history: {e})"),
            }
        } else {
            String::new()
        };
        self.render_due.set(true);
        Ok(format!(
            "Brought over {added} new bookmark{}{history} from {} ({} bookmarks found)",
            if added == 1 { "" } else { "s" },
            source.browser,
            list.len()
        ))
    }

    fn import_tabs(&self, index: usize) -> Result<String, String> {
        let sources = other_browsers();
        let source = sources.get(index).ok_or("that browser isn't there any more")?;
        let found = rippl_core::import_more::open_tabs(source)?;
        if found.is_empty() {
            return Ok(format!("{} has no open tabs to bring over", source.browser));
        }
        {
            let mut tabs = self.tabs.borrow_mut();
            let space = tabs.add_space(&format!("From {}", source.browser));
            let here = tabs.space;
            tabs.space = space;
            for t in &found {
                let id = tabs.push(&t.url);
                if let Some(tab) = tabs.get_mut(id) {
                    tab.title = if t.title.trim().is_empty() { rippl_core::address::pretty(&t.url) } else { t.title.clone() };
                    tab.space = space;
                }
                if t.pinned {
                    tabs.set_pinned(id, true);
                }
            }
            tabs.space = here;
        }
        self.dirty.set(true);
        self.render_due.set(true);
        Ok(format!("Brought over {} tab{} from {} into the space “From {}”", found.len(), if found.len() == 1 { "" } else { "s" }, source.browser, source.browser))
    }
}

/// "zoom-in" → "Zoom in".
fn label(name: &str) -> String {
    match name {
        "grab" => return "Download a video (paste its link, Return)".into(),
        "group-site" => return "Group this site's tabs into one row".into(),
        _ => {}
    }
    let words = name.replace('-', " ");
    let mut c = words.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
