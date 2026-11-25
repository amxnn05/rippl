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

fn other_browsers() -> Vec<import::Source> {
    import::sources(&glib::home_dir())
}

impl Browser {
    pub(crate) fn wire_internal_pages(&self) {
        let context = &self.engine.context;
        if let Some(security) = context.security_manager() {
            security.register_uri_scheme_as_local("rippl");
            security.register_uri_scheme_as_display_isolated("rippl");
            security.register_uri_scheme_as_secure("rippl");
        }
        let weak = self.weak();
        context.register_uri_scheme("rippl", move |request| {
            let Some(b) = weak.upgrade() else { return };
            let html = b.internal_page(request);
            let bytes = glib::Bytes::from_owned(html.into_bytes());
            let len = bytes.len() as i64;
            let stream = gio::MemoryInputStream::from_bytes(&bytes);
            request.finish(&stream, len, Some("text/html"));
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
            "bookmarks" => {
                let labels: Vec<String> =
                    other_browsers().iter().map(|s| format!("{} · {}", s.browser, s.profile)).collect();
                pages::bookmarks(&self.places.bookmarks().unwrap_or_default(), &labels, &note, now(), &self.token)
            }
            _ => {
                let permissions = self.places.permissions().unwrap_or_default();
                let shield_off = self.shield.allowed_list();
                let hidden = self.places.hidden().unwrap_or_default();
                let config = self.config.describe();
                let path = glib::user_config_dir().join("rippl").join("config");
                pages::settings(
                    &SettingsView {
                        permissions: &permissions,
                        shield_off: &shield_off,
                        hidden: &hidden,
                        config: &config,
                        config_path: &path.to_string_lossy(),
                    },
                    &self.token,
                )
            }
        }
    }

    /// A button on one of the pages. Returns a note to show, if any.
    fn act(&self, page: &str, act: &str, arg: &str) -> String {
        let result: Result<String, String> = match (page, act) {
            ("history", "forget") => self.places.forget(arg).map(|_| String::new()).map_err(|e| e.to_string()),
            ("history", "clear") => self.places.clear_history(None).map(|_| String::new()).map_err(|e| e.to_string()),
            ("bookmarks", "remove") => {
                let r = self.places.remove_bookmark(arg).map(|_| String::new()).map_err(|e| e.to_string());
                self.show_front();
                r
            }
            ("bookmarks", "import") => self.import_from(arg.parse().unwrap_or(usize::MAX)),
            ("settings", "forget-permissions") => {
                self.places.forget_permissions(Some(arg)).map(|_| String::new()).map_err(|e| e.to_string())
            }
            ("settings", "shield-on") => {
                if self.shield.is_allowed(arg) {
                    self.shield.toggle(arg, |_| {});
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

    /// Imports bookmarks from the `index`th browser found.
    pub(crate) fn import_from(&self, index: usize) -> Result<String, String> {
        let sources = other_browsers();
        let source = sources.get(index).ok_or("that browser isn't there any more")?;
        let scratch: PathBuf = crate::engine::cache_dir().join("import");
        let list = import::read(source, &scratch)?;
        let added = self.places.import_bookmarks(&list).map_err(|e| e.to_string())?;
        Ok(format!(
            "Brought over {added} new bookmark{} from {} ({} found)",
            if added == 1 { "" } else { "s" },
            source.browser,
            list.len()
        ))
    }
}
