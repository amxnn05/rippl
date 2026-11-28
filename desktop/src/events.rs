//! What the column, the pages and the engine said, acted on. Each arrives
//! here from the event loop, with nothing borrowed.

use rippl_core::address;
use rippl_core::tabs::{SiteGrouping, TabId};
use serde_json::Value;

use crate::browser::{Browser, Ev};
use crate::platform;

impl Browser {
    pub fn handle(&self, ev: Ev) {
        match ev {
            Ev::Chrome(text) => {
                if let Ok(m) = serde_json::from_str::<Value>(&text) {
                    self.from_chrome(&m);
                }
            }
            Ev::Page(id, text) => {
                if let Ok(m) = serde_json::from_str::<Value>(&text) {
                    self.from_page(id, &m);
                }
            }
            Ev::Title(id, title) => {
                let url = {
                    let mut tabs = self.tabs.borrow_mut();
                    let Some(t) = tabs.get_mut(id) else { return };
                    t.title = title.clone();
                    t.url.clone()
                };
                if !self.is_private(id) && self.config.borrow().history && url.starts_with("http") {
                    let _ = self.places.retitle(&url, &title);
                }
                if Some(id) == self.front() {
                    self.window.set_title(if title.is_empty() { "rippl" } else { &title });
                }
                self.dirty.set(true);
                self.render_due.set(true);
            }
            Ev::Load(id, started, url) => self.loaded(id, started, &platform::from_engine(&url)),
            Ev::Navigate(id, url) => {
                let url = platform::from_engine(&url);
                // An http address with HTTPS-only on: its https one instead.
                if self.config.borrow().https_only
                    && let Some(https) = address::https_upgrade(&url)
                    && !crate::shield::host_of(&url).is_some_and(|h| self.places.site_flag(&h, "http").unwrap_or(false))
                {
                    if let Some(v) = self.views.borrow().get(&id) {
                        let _ = v.load_url(&https);
                    }
                    return;
                }
                if let Some(v) = self.views.borrow().get(&id) {
                    self.shield.follow(v, &url);
                }
            }
            Ev::Popup(id, url) => {
                // A pop-up is a tab next to its opener.
                let private = self.is_private(id);
                if self.front() != Some(id) {
                    self.select(id);
                }
                self.open(&platform::from_engine(&url), private, false);
            }
            Ev::Serve(uri, responder) => {
                let html = self.serve(&uri);
                let response = wry::http::Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .header("Cache-Control", "no-store")
                    .header("Content-Security-Policy", "default-src 'none'; img-src data: https:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; form-action rippl: https://rippl.localhost")
                    .body(std::borrow::Cow::Owned(html.into_bytes()))
                    .unwrap_or_default();
                responder.respond(response);
            }
            Ev::DownloadStarted(url, path) => {
                self.downloads.started(url, path);
                self.render_due.set(true);
            }
            Ev::DownloadDone(url, path, ok) => {
                let private = self.is_private(self.front().unwrap_or(0));
                if let Some(name) = self.downloads.finished(&url, path, ok, private) {
                    self.notice(&if ok { format!("Downloaded {name}") } else { format!("{name} didn't finish downloading") });
                }
                self.render_due.set(true);
            }
            Ev::Suggest(typed, words) => self.suggestions_came(&typed, words),
            Ev::ShieldLists(lists) => self.shield.lists_ready(lists, self.proxy.clone()),
            Ev::ShieldApplied => self.reapply_shield(),
            Ev::Menu(id) => {
                let action = self.menu_actions.borrow_mut().remove(&id);
                self.menu_actions.borrow_mut().clear();
                if let Some(a) = action {
                    a(self);
                }
            }
            Ev::Control(line, reply) => {
                let answer = self.control(&line);
                let _ = reply.send(answer);
            }
            Ev::Tick => {
                if self.downloads.poll() {
                    self.render_due.set(true);
                }
                self.tick();
            }
        }
    }

    pub fn is_private(&self, id: TabId) -> bool {
        self.tabs.borrow().get(id).is_some_and(|t| t.private)
    }

    fn loaded(&self, id: TabId, started: bool, url: &str) {
        let (title, private) = {
            let mut tabs = self.tabs.borrow_mut();
            let Some(t) = tabs.get_mut(id) else { return };
            t.loading = started;
            if !url.is_empty() && url != "about:blank" {
                t.url = url.to_string();
            }
            (t.title.clone(), t.private)
        };
        let (back, forward) = match self.views.borrow().get(&id) {
            Some(v) => (v.can_go_back().unwrap_or(false), v.can_go_forward().unwrap_or(false)),
            None => (false, false),
        };
        {
            let mut extra = self.extra.borrow_mut();
            let e = extra.entry(id).or_default();
            e.can_back = back;
            e.can_forward = forward;
            e.progress = if started { 0.2 } else { 1.0 };
            if started {
                e.icon.clear();
            }
        }
        if !started {
            if !private && self.config.borrow().history && url.starts_with("http") {
                let _ = self.places.visit(url, &title, false);
            }
            self.style_page(id, url);
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    fn from_page(&self, id: TabId, m: &Value) {
        let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        match m.get("t").and_then(Value::as_str).unwrap_or("") {
            "cmd" => self.command(&s("name"), m.get("repeat").and_then(Value::as_bool).unwrap_or(false)),
            "mod-up" => {}
            "icon" => {
                let href = s("href");
                if href.starts_with("https://") || href.starts_with("http://") || href.starts_with("data:image/") {
                    self.extra.borrow_mut().entry(id).or_default().icon = href;
                    self.render_due.set(true);
                }
            }
            "hover" => self.extra.borrow_mut().entry(id).or_default().hover = s("url"),
            "found" => {
                let count = m.get("count").and_then(Value::as_u64).unwrap_or(0);
                let text = s("text");
                let shown = if text.is_empty() { String::new() } else if count == 0 { "None".into() } else { count.to_string() };
                if let Some(f) = self.find.borrow_mut().as_mut() {
                    f.1 = shown.clone();
                }
                let js = serde_json::to_string(&shown).unwrap_or_default();
                self.eval_chrome_pub(&format!("rippl.findCount({js})"));
            }
            "open-behind" => {
                let private = self.is_private(id);
                self.open(&s("url"), private, true);
            }
            "peek" => {
                let private = self.is_private(id);
                self.open(&s("url"), private, false);
            }
            "audio" => {
                let on = m.get("on").and_then(Value::as_bool).unwrap_or(false);
                if let Some(t) = self.tabs.borrow_mut().get_mut(id) {
                    t.playing_audio = on;
                }
            }
            _ => {}
        }
    }

    fn from_chrome(&self, m: &Value) {
        let n = |k: &str| m.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let id = || m.get("id").and_then(Value::as_u64).unwrap_or(0) as TabId;
        let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| m.get(k).and_then(Value::as_bool).unwrap_or(false);
        match m.get("t").and_then(Value::as_str).unwrap_or("") {
            "ready" => {
                self.ready.set(true);
                self.render();
            }
            "page-rect" => {
                self.page_rect.set((n("x"), n("y"), n("w"), n("h")));
                self.place_views();
            }
            "cmd" => self.command(&s("name"), b("repeat")),
            "select" => {
                if b("meta") {
                    // Ctrl/⌘+click: a split with the front tab.
                    self.select(id());
                } else {
                    self.select(id());
                }
            }
            "close" => self.close(id()),
            "fold" => {
                let g = m.get("group").and_then(Value::as_u64).unwrap_or(0) as u32;
                self.tabs.borrow_mut().toggle_group(g);
                self.dirty.set(true);
                self.render_due.set(true);
            }
            "move" => {
                let onto = m.get("onto").and_then(Value::as_u64).unwrap_or(0) as TabId;
                let at = self.tabs.borrow().index(onto);
                if let Some(at) = at {
                    self.tabs.borrow_mut().move_to(id(), at);
                    self.dirty.set(true);
                    self.render_due.set(true);
                }
            }
            "space" => self.switch_space(m.get("id").and_then(Value::as_u64).unwrap_or(0) as u32),
            "typed" => self.suggest(&s("text")),
            "go" => self.go(&s("text"), b("new_tab"), b("background")),
            "escape" => {
                if let Some(id) = self.front()
                    && let Some(v) = self.views.borrow().get(&id)
                {
                    let _ = v.focus();
                }
            }
            "field-focus" => self.field_focused.set(b("on")),
            "find" => {
                let text = s("text");
                let dir = n("dir");
                let count = self.find.borrow().as_ref().map(|f| f.1.clone()).unwrap_or_default();
                *self.find.borrow_mut() = Some((text.clone(), count));
                let js = serde_json::to_string(&text).unwrap_or_default();
                self.front_view_eval(&format!("window.__rippl && __rippl.find({js}, {})", dir < 0.0));
            }
            "find-close" => {
                self.close_find();
                if let Some(id) = self.front()
                    && let Some(v) = self.views.borrow().get(&id)
                {
                    let _ = v.focus();
                }
            }
            "tab-menu" => self.tab_menu(id(), n("x"), n("y")),
            "group-menu" => self.group_menu(m.get("group").and_then(Value::as_u64).unwrap_or(0) as u32, n("x"), n("y")),
            "space-menu" => self.space_menu(m.get("id").and_then(Value::as_u64).unwrap_or(0) as u32, n("x"), n("y")),
            "site-card" => self.site_card(n("x"), n("y")),
            "download-open" => {
                let d = m.get("id").and_then(Value::as_u64).unwrap_or(0);
                match self.downloads.path_of(d) {
                    Some(p) => {
                        if let Err(e) = platform::open(&p) {
                            self.notice(&format!("That didn't open: {e}"));
                        }
                    }
                    None => self.command("downloads", false),
                }
            }
            "download-forget" => {
                self.downloads.forget_live(m.get("id").and_then(Value::as_u64).unwrap_or(0));
                self.render_due.set(true);
            }
            "column-width" => {
                let w = n("w").clamp(160.0, 420.0) as u32;
                let _ = std::fs::write(rippl_core::dirs::data().join("column-width"), w.to_string());
            }
            "drag-window" => {
                let _ = self.window.drag_window();
            }
            "zoom-window" => self.window.set_maximized(!self.window.is_maximized()),
            "bar" => {
                let answer = s("answer");
                self.answer_bar(&answer);
            }
            _ => {}
        }
    }

    pub fn eval_chrome_pub(&self, js: &str) {
        if let Some(c) = self.chrome.borrow().as_ref() {
            let _ = c.evaluate_script(js);
        }
    }

    pub fn switch_space(&self, space: u32) {
        let front = self.tabs.borrow_mut().switch_space(space);
        match front {
            Some(id) => self.select(id),
            None => {
                let id = self.tabs.borrow_mut().open("");
                self.select(id);
            }
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    pub fn group_site_front(&self) {
        let Some(id) = self.front() else { return };
        let r = self.tabs.borrow_mut().group_site(id);
        match r {
            SiteGrouping::Grouped(_, n) => self.notice(&format!("{n} tabs of this site, in one row")),
            SiteGrouping::Ungrouped(_) => self.notice("Ungrouped"),
            _ => self.notice("Only one tab of this site is open"),
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    pub fn answer_bar(&self, answer: &str) {
        let _ = answer;
        *self.bar.borrow_mut() = None;
        self.render_due.set(true);
    }
}
