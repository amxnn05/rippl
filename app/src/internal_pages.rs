//! Serving rippl's own pages (`rippl://history`, `rippl://bookmarks`,
//! `rippl://settings`) and acting on their buttons. The HTML is built in
//! `rippl_core::pages`.
//!
//! The scheme is registered with WebKit as:
//! - **local**: pages from the web can't link to, load or open it;
//! - **display-isolated**: only a `rippl://` page can show a `rippl://` page;
//! - **secure**: it never counts as mixed content.
//!
//! A button acts only when its request carries this launch's random
//! token, which appears nowhere but inside these pages.

use std::path::PathBuf;

use rippl_core::import;
use rippl_core::pages::{self, SettingsView};
use rippl_core::places::now;
use webkit6::{gio, glib, URISchemeRequest};

use crate::browser::Browser;

/// 128 random bits as hex, from the kernel.
pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut bytes);
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn finish(request: &URISchemeRequest, html: String) {
    let bytes = glib::Bytes::from_owned(html.into_bytes());
    let len = bytes.len() as i64;
    let stream = gio::MemoryInputStream::from_bytes(&bytes);
    request.finish(&stream, len, Some("text/html"));
}

fn other_browsers() -> Vec<import::Source> {
    import::sources(&glib::home_dir())
}

impl Browser {
    pub(crate) fn wire_internal_pages(&self) {
        // Once for the engine every window shares; a page is served by the
        // window its tab is in.
        if !self.primary {
            return;
        }
        let context = &self.engine.context;
        if let Some(security) = context.security_manager() {
            security.register_uri_scheme_as_local("rippl");
            security.register_uri_scheme_as_display_isolated("rippl");
            security.register_uri_scheme_as_secure("rippl");
        }
        context.register_uri_scheme("rippl", move |request| {
            let asker = request.web_view();
            let windows = crate::windows::all();
            let Some(b) = windows
                .iter()
                .find(|w| asker.as_ref().is_some_and(|v| w.views.borrow().values().any(|x| x == v)))
                .or_else(|| windows.first())
                .cloned()
            else {
                return;
            };
            let url = request.uri().map(|u| u.to_string()).unwrap_or_default();
            // Passwords come from the keyring, which answers later.
            if pages::page_name(&url) == "passwords" {
                let request = request.clone();
                glib::MainContext::default().spawn_local(async move {
                    let html = b.passwords_page(&url).await;
                    finish(&request, html);
                });
                return;
            }
            let html = b.internal_page(request);
            finish(request, html);
        });
    }

    fn internal_page(&self, request: &URISchemeRequest) -> String {
        let url = request.uri().map(|u| u.to_string()).unwrap_or_default();
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
        match page.as_str() {
            "history" => {
                let places = if q.trim().is_empty() {
                    self.places.recent(300).unwrap_or_default()
                } else {
                    self.places.search(&q, 300).unwrap_or_default()
                };
                pages::history(&places, &q, now(), &self.token)
            }
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
            _ => {
                let permissions = self.places.permissions().unwrap_or_default();
                let shield_off = self.shield.allowed_list();
                let hidden = self.places.hidden().unwrap_or_default();
                let config = self.config.borrow().rows();
                let shortcuts = self.shortcut_rows();
                let about = vec![
                    ("Version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
                    ("Engine".to_string(), format!("WebKitGTK {}.{}.{}", webkit6::functions::major_version(), webkit6::functions::minor_version(), webkit6::functions::micro_version())),
                    ("Profile".to_string(), crate::engine::profile().unwrap_or("default").to_string()),
                    ("Your data".to_string(), crate::engine::data_dir().display().to_string()),
                    ("Source".to_string(), "github.com/amxnn05/rippl".to_string()),
                ];
                let path = crate::config::path();
                pages::settings(
                    &SettingsView {
                        permissions: &permissions,
                        shield_off: &shield_off,
                        hidden: &hidden,
                        config: &config,
                        shortcuts: &shortcuts,
                        about: &about,
                        config_path: &path.to_string_lossy(),
                        note: &note,
                    },
                    &self.token,
                )
            }
        }
    }

    async fn passwords_page(&self, url: &str) -> String {
        let token_ok = pages::param(url, "token").as_deref() == Some(self.token.as_str());
        if let Some(act) = pages::param(url, "do") {
            let arg = pages::param(url, "arg").unwrap_or_default();
            if !token_ok {
                eprintln!("rippl: refused an action without this launch's token: {act}");
            } else if act == "delete" {
                let (origin, username) = arg.split_once('\n').unwrap_or((&arg, ""));
                if let Err(e) = self.keyring.delete(origin, username).await {
                    eprintln!("rippl: keyring: {e}");
                }
            } else if act == "allow-saving" {
                let _ = self.places.set_never_save(&arg, false);
            }
        }
        let accounts: Vec<(String, String)> = self
            .keyring
            .accounts(None)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.origin, a.username))
            .collect();
        let never = self.places.never_save_list().unwrap_or_default();
        let store = if self.keyring.is_memory() { "a test keyring in memory" } else { "your system keyring" };
        pages::passwords(&accounts, &never, store, &self.token)
    }

    /// A button on one of the pages. Returns a note to show, if any.
    fn act(&self, page: &str, act: &str, arg: &str) -> String {
        if page == "bookmarks" {
            // Whatever it was, the bar shows it after.
            let weak = self.weak();
            glib::idle_add_local_once(move || {
                if let Some(b) = weak.upgrade() {
                    b.fill_bookmarks_bar();
                }
            });
        }
        let result: Result<String, String> = match (page, act) {
            ("history", "forget") => self.places.forget(arg).map(|_| String::new()).map_err(|e| e.to_string()),
            ("archive", "forget") => self.places.unarchive(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string()),
            ("archive", "clear") => self.places.unarchive(None).map(|_| String::new()).map_err(|e| e.to_string()),
            ("history", "clear") => self.places.clear_history(None).map(|_| String::new()).map_err(|e| e.to_string()),
            ("bookmarks", "remove") => {
                let r = self.places.remove_bookmark(arg).map(|_| String::new()).map_err(|e| e.to_string());
                self.show_front();
                r
            }
            ("bookmarks", "import") => self.import_from(arg.parse().unwrap_or(usize::MAX)),
            ("bookmarks", "import-tabs") => self.import_tabs(arg.parse().unwrap_or(usize::MAX)),
            ("bookmarks", "import-passwords") => {
                // The keyring answers later: the note comes as a notice.
                self.import_passwords(arg.parse().unwrap_or(usize::MAX));
                Ok("Bringing passwords over…".into())
            }
            ("bookmarks", "move") => {
                let (url, folder) = arg.split_once('\n').unwrap_or((arg, ""));
                self.places.move_bookmark(url, folder).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("settings", "set") => {
                let (key, value) = arg.split_once('=').unwrap_or((arg, ""));
                let r = self.change_setting(key, value);
                // A key just recorded (or the recorder left): every
                // command has its keys back.
                self.restore_shortcuts();
                r
            }
            // The shortcut recorder listens: rippl's own keys step aside.
            ("settings", "record-start") => {
                self.suspend_shortcuts();
                Ok(String::new())
            }
            ("settings", "record-stop") => {
                self.restore_shortcuts();
                Ok(String::new())
            }
            ("settings", "clear-history") => {
                self.places.clear_history(None).map(|_| "History cleared.".to_string()).map_err(|e| e.to_string())
            }
            ("settings", "forget-permissions") => {
                self.places.forget_permissions(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("settings", "shield-on") => {
                if self.shield.is_allowed(arg) {
                    self.shield.toggle(arg, |_| {});
                    self.install_page_scripts();
                }
                Ok(String::new())
            }
            ("settings", "unhide") => {
                let r = self.places.unhide(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string());
                self.apply_hidden();
                r
            }
            _ => Err(format!("unknown action {act}")),
        };
        result.unwrap_or_else(|e| {
            eprintln!("rippl: {page} {act}: {e}");
            format!("That didn't work: {e}")
        })
    }

    /// One setting changed on rippl://settings: saved, and applied now
    /// where it can be.
    pub(crate) fn change_setting(&self, key: &str, value: &str) -> Result<String, String> {
        self.config.borrow_mut().set(key, value)?;
        let config = self.config.borrow().clone();
        match key {
            "theme" | "glass" => self.apply_look(),
            "icons" => self.render(),
            "home_grid" => self.fill_home_grid(),
            "bookmarks_bar" => self.fill_bookmarks_bar(),
            "tabs" | "column" => self.apply_chrome(),
            k if k.starts_with("shortcut.") => self.apply_shortcut(&k["shortcut.".len()..]),
            "home_theme" | "home_colour" | "home_image" => self.apply_home_theme(),
            "sync_folder" => return Ok(format!("Saved. {}", self.sync_now())),
            "agents" => {
                if config.agents
                    && let Some(me) = crate::windows::all().into_iter().next()
                {
                    crate::control::start(&me);
                }
            }
            "proxy" => {
                self.engine.set_proxy(config.proxy.clone());
                return Ok("Saved. New connections go this way now, open pages included; private tabs already open keep the old way.".into());
            }
            "frame_rate" => {
                self.engine.set_full_rate(config.full_rate);
                return Ok("Saved. Pages loaded from now on use it; reload a page to change it there.".into());
            }
            "video_keys" | "autoscroll" => self.install_page_scripts(),
            "player" => self.render_player(),
            "gpc" | "https_only" => self.engine.send_settings(config.gpc, config.https_only),
            "shield" | "ublock" => return Ok("Saved. Blocking changes after rippl restarts.".into()),
            _ => {}
        }
        Ok("Saved.".into())
    }

    /// Imports bookmarks and history from the `index`th browser found.
    pub(crate) fn import_from(&self, index: usize) -> Result<String, String> {
        let sources = other_browsers();
        let source = sources.get(index).ok_or("that browser isn't there any more")?;
        let scratch: PathBuf = crate::engine::cache_dir().join("import");
        let list = import::read(source, &scratch)?;
        let added = self.places.import_bookmarks(&list).map_err(|e| e.to_string())?;
        // History too, unless you keep none; the last 20,000 pages.
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
        Ok(format!(
            "Brought over {added} new bookmark{}{history} from {} ({} bookmarks found)",
            if added == 1 { "" } else { "s" },
            source.browser,
            list.len()
        ))
    }

    /// The tabs the `index`th browser has open, in a space of their own,
    /// asleep until each is first looked at.
    pub(crate) fn import_tabs(&self, index: usize) -> Result<String, String> {
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
        self.render_spaces();
        Ok(format!(
            "Brought over {} tab{} from {} into the space “From {}” (Alt+number, or the foot of the column)",
            found.len(),
            if found.len() == 1 { "" } else { "s" },
            source.browser,
            source.browser
        ))
    }

    /// The `index`th browser's saved passwords, into your keyring. Its key
    /// is asked of the keyring first; the result comes as a notice.
    pub(crate) fn import_passwords(&self, index: usize) {
        let sources = other_browsers();
        let Some(source) = sources.get(index).cloned() else { return };
        let scratch: PathBuf = crate::engine::cache_dir().join("import");
        let logins = match rippl_core::import_more::logins(&source, &scratch) {
            Ok(l) => l,
            Err(e) => return self.notice(&format!("Passwords: {e}")),
        };
        let weak = self.weak();
        glib::MainContext::default().spawn_local(async move {
            let Some(b) = weak.upgrade() else { return };
            let secret = crate::keyring::chromium_key(source.browser).await;
            let (mut saved, mut sealed) = (0, 0);
            for l in &logins {
                let Some(password) =
                    rippl_core::import_more::unseal(&l.sealed, secret.as_ref().map(|s| s.as_bytes())).map(zeroize::Zeroizing::new)
                else {
                    sealed += 1;
                    continue;
                };
                let Some(origin) = rippl_core::passwords::origin_of(&l.origin) else { continue };
                if b.keyring.save(&origin, &l.username, &password).await.is_ok() {
                    saved += 1;
                }
            }
            let mut note = format!("Brought over {saved} password{} from {}", if saved == 1 { "" } else { "s" }, source.browser);
            if sealed > 0 {
                note.push_str(&format!("; {sealed} couldn't be opened (its key isn't in your keyring)"));
            }
            b.notice(&note);
        });
    }
}
