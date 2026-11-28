//! Ctrl+Tab (macOS and Windows), as the GTK build's switcher.rs: the tabs
//! of the space on screen as cards, the one you're on first, then the ones
//! you last left, latest first. Tab walks on, Shift+Tab back; letting go of
//! Ctrl picks. A quick Ctrl+Tab is the tab you were on before.
//! `tab_switcher = off`: Ctrl+Tab is the next tab.
//!
//! The cards are drawn in the overlay: a transparent view over the whole
//! window, raised above the pages while it shows.

use rippl_core::address;
use rippl_core::tabs::TabId;
use serde_json::{Value, json};
use wry::WebViewBuilder;
use wry::dpi::{LogicalPosition, LogicalSize};

use crate::browser::{Browser, Ev, post};
use crate::platform;

impl Browser {
    /// Remembers the order tabs were looked at.
    pub fn looked_at(&self, id: TabId) {
        let mut r = self.recent.borrow_mut();
        r.retain(|t| *t != id);
        r.insert(0, id);
        r.truncate(200);
    }

    fn walk_order(&self) -> Vec<TabId> {
        let tabs = self.tabs.borrow();
        let here: Vec<TabId> = tabs.here().map(|t| t.id).collect();
        let mut order: Vec<TabId> = self.recent.borrow().iter().copied().filter(|t| here.contains(t)).collect();
        for t in here {
            if !order.contains(&t) {
                order.push(t);
            }
        }
        order
    }

    /// Ctrl+Tab (or Ctrl+Shift+Tab), first or again while Ctrl is held.
    pub fn walk(&self, back: bool) {
        if !self.config.borrow().tab_switcher {
            let next = self.tabs.borrow().neighbour(if back { -1 } else { 1 });
            if let Some(n) = next {
                self.select(n);
            }
            return;
        }
        let started = self.walking.borrow().is_some();
        if !started {
            let order = self.walk_order();
            if order.len() < 2 {
                return;
            }
            let at = if back { order.len() - 1 } else { 1 };
            *self.walking.borrow_mut() = Some((order, at));
        } else if let Some((order, at)) = self.walking.borrow_mut().as_mut() {
            let n = order.len();
            *at = if back { (*at + n - 1) % n } else { (*at + 1) % n };
        }
        self.show_switcher();
    }

    /// Ctrl let go: the chosen tab comes to front.
    pub fn end_walk(&self, go: bool) {
        let Some((order, at)) = self.walking.borrow_mut().take() else { return };
        self.hide_overlay();
        if go && let Some(id) = order.get(at) {
            self.select(*id);
        }
    }

    pub fn pick_from_switcher(&self, id: TabId) {
        self.walking.borrow_mut().take();
        self.hide_overlay();
        self.select(id);
    }

    fn show_switcher(&self) {
        let Some((order, at)) = self.walking.borrow().clone() else { return };
        let cards: Vec<Value> = {
            let tabs = self.tabs.borrow();
            let extra = self.extra.borrow();
            order
                .iter()
                .filter_map(|id| tabs.get(*id))
                .take(24)
                .map(|t| {
                    json!({
                        "id": t.id,
                        "title": if t.title.is_empty() { address::pretty(&t.url) } else { t.title.clone() },
                        "site": crate::shield::host_of(&t.url).unwrap_or_else(|| if t.url.is_empty() { "New tab".into() } else { t.url.clone() }),
                        "letter": address::monogram(&t.url, &t.title),
                        "icon": extra.get(&t.id).map(|e| e.icon.clone()).unwrap_or_default(),
                    })
                })
                .collect()
        };
        let state = json!({ "cards": cards, "chosen": at, "dark": self.dark.get() });
        self.overlay_eval(&format!("rippl.switcher({state})"));
    }

    // ---- the overlay ----------------------------------------------------

    fn overlay_eval(&self, js: &str) {
        self.ensure_overlay();
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        if let Some(o) = self.overlay.borrow().as_ref() {
            let _ = o.set_bounds(wry::Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(size.width, size.height).into(),
            });
            if self.overlay_ready.get() {
                let _ = o.evaluate_script(js);
            } else {
                *self.overlay_queued.borrow_mut() = Some(js.to_string());
            }
            let _ = o.set_visible(true);
            platform::raise(o, &self.window, &self.host);
        }
    }

    pub fn hide_overlay(&self) {
        if let Some(o) = self.overlay.borrow().as_ref() {
            let _ = o.set_visible(false);
        }
    }

    fn ensure_overlay(&self) {
        if self.overlay.borrow().is_some() {
            return;
        }
        let proxy = self.proxy.clone();
        let builder = WebViewBuilder::new()
            .with_html(include_str!("../ui/overlay.html"))
            .with_transparent(true)
            .with_visible(false)
            .with_focused(false)
            .with_ipc_handler(move |req| post(&proxy, Ev::Overlay(req.body().clone())));
        match platform::build(builder, &self.window, &self.host) {
            Ok(v) => *self.overlay.borrow_mut() = Some(v),
            Err(e) => eprintln!("rippl: the overlay: {e}"),
        }
    }

    pub fn on_overlay(&self, text: &str) {
        let Ok(m) = serde_json::from_str::<Value>(text) else { return };
        match m.get("t").and_then(Value::as_str).unwrap_or("") {
            "ready" => {
                self.overlay_ready.set(true);
                let queued = self.overlay_queued.borrow_mut().take();
                if let Some(js) = queued
                    && let Some(o) = self.overlay.borrow().as_ref()
                {
                    let _ = o.evaluate_script(&js);
                }
            }
            "pick" => self.pick_from_switcher(m.get("id").and_then(Value::as_u64).unwrap_or(0) as TabId),
            "walk" => self.walk(m.get("back").and_then(Value::as_bool).unwrap_or(false)),
            "mod-up" => self.end_walk(true),
            "cancel" => self.end_walk(false),
            _ => {}
        }
    }
}
