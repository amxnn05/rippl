//! What rippl does to how pages look, and taking pictures of them.
//!
//! - **Dark pages**: with `dusk = on` and rippl in its dark
//!   look, a page that has no dark look of its own is darkened. When a page
//!   finishes loading, a check in rippl's isolated world looks at what the
//!   page says (`color-scheme` dark) and at its background; only a light
//!   page with no dark scheme is marked, and one style sheet darkens
//!   marked pages, turning pictures, video and frames back to their own
//!   colours. Off for a site from the site card.
//! - **Your style for a site**: CSS you
//!   write in the site card, kept per site, applied with the site's hidden
//!   elements as one user style sheet before the page draws.
//! - **Capture**: the whole page as a
//!   PNG in `~/Pictures/rippl/`, and on the clipboard.
//! - **Save as a web archive** (Ctrl+Alt+A): the page and everything it
//!   shows in one `.mhtml` file in Downloads, which rippl opens again as
//!   it was.
//! - **Site icons**: WebKit's favicon store is on, and a tab shows its
//!   site's icon instead of its letter once there is one (letters while
//!   none is known, or with `icons = off`).

use std::path::PathBuf;

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gdk, gio, glib, gtk, SnapshotOptions, SnapshotRegion, UserContentInjectedFrames, UserStyleLevel, UserStyleSheet, WebView};

use crate::browser::Browser;
use crate::shield::host_of;

pub const NO_DUSK: &str = "nodusk";

/// Darkens a page rippl has marked; pictures, video and frames are turned
/// back so they keep their own colours.
const DUSK_CSS: &str = "
html[data-rippl-dusk] { filter: invert(0.9) hue-rotate(180deg) !important; background: #fff !important; }
html[data-rippl-dusk] :is(img, video, picture, canvas, iframe, embed, object, svg image) {
  filter: invert(1) hue-rotate(180deg) !important; }
";

/// Marks the page for darkening when it has no dark look of its own.
/// Answers what it found.
const DUSK_CHECK: &str = r#"
(() => {
  const root = document.documentElement;
  if (!root || !document.body) return "none";
  const meta = (document.querySelector('meta[name="color-scheme"]') || {}).content || "";
  if (/dark/.test(meta) || /dark/.test(getComputedStyle(root).colorScheme || "")) return "has-dark";
  const light = c => {
    const m = (c || "").match(/[\d.]+/g);
    if (!m) return null;
    const [r, g, b, a = 1] = m.map(Number);
    return a < 0.1 ? null : (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  };
  const l = light(getComputedStyle(document.body).backgroundColor) ?? light(getComputedStyle(root).backgroundColor) ?? 1;
  if (l > 0.6) { root.setAttribute("data-rippl-dusk", ""); return "darkened"; }
  return "already-dark";
})()
"#;

impl Browser {
    /// Every site's hidden elements and own CSS, and the darkening sheet.
    pub(crate) fn apply_site_styles(&self) {
        let content = &self.engine.content;
        content.remove_all_style_sheets();
        content.add_style_sheet(&UserStyleSheet::new(
            DUSK_CSS,
            UserContentInjectedFrames::TopFrame,
            UserStyleLevel::User,
            &[],
            &[],
        ));
        let mut by_host: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (host, selectors) in self.places.hidden().unwrap_or_default() {
            // One rule per selector: an invalid one only loses itself.
            let css: String = selectors.iter().map(|s| format!("{s} {{ display: none !important; }}\n")).collect();
            by_host.entry(host).or_default().push_str(&css);
        }
        for (host, css) in self.places.all_site_css().unwrap_or_default() {
            let entry = by_host.entry(host).or_default();
            entry.push_str(&css);
            entry.push('\n');
        }
        for (host, css) in by_host {
            let allow = [format!("*://{host}/*"), format!("*://*.{host}/*")];
            let allow: Vec<&str> = allow.iter().map(String::as_str).collect();
            let sheet = UserStyleSheet::new(&css, UserContentInjectedFrames::TopFrame, UserStyleLevel::User, &allow, &[]);
            content.add_style_sheet(&sheet);
        }
    }

    /// A page finished loading: darken it if it should be.
    pub(crate) fn dusk(&self, id: TabId, view: &WebView) {
        let dark = self.window.has_css_class("dark");
        if !self.config.borrow().dusk || !dark {
            return;
        }
        let Some(host) = view.uri().and_then(|u| host_of(&u)) else { return };
        if self.places.site_flag(&host, NO_DUSK).unwrap_or(false) {
            return;
        }
        let weak = self.weak();
        view.evaluate_javascript(DUSK_CHECK, Some("rippl"), None, None::<&gio::Cancellable>, move |r| {
            if let (Some(b), Ok(v)) = (weak.upgrade(), r)
                && std::env::var_os("RIPPL_DEBUG").is_some()
            {
                eprintln!("rippl: dusk {id}: {}", v.to_str());
                let _ = b;
            }
        });
    }

    /// Ctrl+Shift+S: the whole page as a picture, saved and copied.
    pub(crate) fn capture(&self) {
        let Some(view) = self.front_view() else { return };
        let host = view.uri().and_then(|u| host_of(&u)).unwrap_or_else(|| "page".into());
        let weak = self.weak();
        view.snapshot(SnapshotRegion::FullDocument, SnapshotOptions::NONE, None::<&gio::Cancellable>, move |result| {
            let Some(b) = weak.upgrade() else { return };
            let texture = match result {
                Ok(t) => t,
                Err(e) => {
                    b.notice(&format!("Couldn't capture this page: {e}"));
                    return;
                }
            };
            let dir = glib::user_special_dir(glib::UserDirectory::Pictures)
                .unwrap_or_else(|| glib::home_dir().join("Pictures"))
                .join("rippl");
            let _ = std::fs::create_dir_all(&dir);
            let stamp = glib::DateTime::now_local()
                .ok()
                .and_then(|d| d.format("%Y-%m-%d %H.%M.%S").ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "capture".into());
            let path: PathBuf = rippl_core::downloads::destination(&dir, &format!("{host} {stamp}.png"), |p| p.exists());
            let saved = texture.save_to_png(&path).is_ok();
            gdk::prelude::DisplayExt::clipboard(&WidgetExt::display(&b.window)).set_texture(&texture);
            b.notice(&if saved {
                format!("Captured: {} (and copied)", path.display())
            } else {
                "Captured and copied (couldn't save the file)".into()
            });
        });
    }

    /// Ctrl+Alt+A: the page as one web archive (MHTML) in Downloads.
    pub(crate) fn save_archive(&self) {
        let Some(view) = self.front_view() else { return };
        let title = view.title().map(|t| t.to_string()).filter(|t| !t.trim().is_empty());
        let name = title.or_else(|| view.uri().and_then(|u| host_of(&u))).unwrap_or_else(|| "page".into());
        let dir = glib::user_special_dir(glib::UserDirectory::Downloads).unwrap_or_else(|| glib::home_dir().join("Downloads"));
        let _ = std::fs::create_dir_all(&dir);
        let path = rippl_core::downloads::destination(&dir, &format!("{name}.mhtml"), |p| p.exists());
        let file = gio::File::for_path(&path);
        let weak = self.weak();
        view.save_to_file(&file, webkit6::SaveMode::Mhtml, None::<&gio::Cancellable>, move |r| {
            if let Some(b) = weak.upgrade() {
                b.notice(&match r {
                    Ok(()) => format!("Saved: {}", path.display()),
                    Err(e) => format!("Couldn't save this page: {e}"),
                });
            }
        });
    }

    /// The site's icon for a tab, if WebKit knows it: from its view, or for
    /// a tab with none (asleep, not opened yet) from the favicon store.
    pub(crate) fn icon_for(&self, id: TabId, url: &str) -> Option<gdk::Texture> {
        if !self.config.borrow().icons {
            return None;
        }
        if let Some(t) = self.views.borrow().get(&id).and_then(|v| v.favicon()) {
            return Some(t);
        }
        if let Some(t) = self.icons.borrow().get(url) {
            return Some(t.clone());
        }
        // Asked once per address; the answer redraws the column.
        if url.starts_with("http") && self.icons_asked.borrow_mut().insert(url.to_string()) {
            let db = self.engine.session.website_data_manager().and_then(|m| m.favicon_database());
            if let Some(db) = db {
                let weak = self.weak();
                let key = url.to_string();
                db.favicon(url, None::<&gio::Cancellable>, move |r| {
                    if let (Some(b), Ok(t)) = (weak.upgrade(), r) {
                        b.icons.borrow_mut().insert(key, t);
                        glib::idle_add_local_once(move || {
                            b.render();
                            if b.config.borrow().bookmarks_bar {
                                b.fill_bookmarks_bar();
                            }
                        });
                    }
                });
            }
        }
        None
    }

    /// A site's icon for something that isn't a tab (the bookmarks bar).
    pub(crate) fn icon_for_url(&self, url: &str) -> Option<gdk::Texture> {
        self.icon_for(TabId::MAX, url)
    }

    /// The site card's editor for the site's own CSS.
    pub(crate) fn edit_site_css(&self, host: &str) {
        let current = self.places.site_css(host).ok().flatten().unwrap_or_default();
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.set_margin_top(10);
        card.set_margin_bottom(10);
        card.set_margin_start(12);
        card.set_margin_end(12);
        let title = gtk::Label::builder().label(format!("Your style for {host}")).xalign(0.0).build();
        title.add_css_class("card-title");
        let text = gtk::TextView::builder().monospace(true).wrap_mode(gtk::WrapMode::WordChar).build();
        text.buffer().set_text(&current);
        let scroll =
            gtk::ScrolledWindow::builder().child(&text).min_content_height(160).min_content_width(320).build();
        scroll.add_css_class("css-edit");
        let hint = gtk::Label::builder()
            .label("CSS for this site's pages, applied before they draw. Empty removes it.")
            .xalign(0.0)
            .wrap(true)
            .build();
        hint.add_css_class("dim");
        let save = gtk::Button::with_label("Save");
        save.add_css_class("suggested-action");
        card.append(&title);
        card.append(&scroll);
        card.append(&hint);
        card.append(&save);
        let weak = self.weak();
        let host = host.to_string();
        save.connect_clicked(move |_| {
            let Some(b) = weak.upgrade() else { return };
            let buffer = text.buffer();
            let css = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            let _ = b.places.set_site_css(&host, &css);
            b.apply_site_styles();
            b.card.popdown();
            b.notice(if css.trim().is_empty() { "Your style for this site is gone" } else { "Your style for this site is on" });
        });
        self.card.set_child(Some(&card));
        self.card.popup();
    }
}
