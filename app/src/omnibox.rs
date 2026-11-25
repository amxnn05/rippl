//! The one field, while you type in it.
//!
//! The column's tab list gives way to what the field offers: open tabs that
//! match ("Switch to tab"), then bookmarks and history, ranked by
//! `rippl_core::suggest`. An address you have been to completes inline:
//! "you" becomes "you|tube.com" with the rest selected, so typing on replaces
//! it, Backspace removes it and Return takes it.
//!
//! Focusing the field with nothing typed offers your bookmarks and recent
//! pages. Down and Up move through the offers; Return opens the one picked,
//! or what is in the field; Esc puts the page's address back.
//!
//! Everything offered comes from this machine. Nothing typed is sent
//! anywhere until Return.

use rippl_core::places::Place;
use rippl_core::suggest;
use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gdk, glib, gtk};

use crate::browser::Browser;

/// One row under the field.
#[derive(Debug, Clone)]
pub enum Offer {
    Tab(TabId),
    Page(String),
    /// Put this in the field and keep typing ("r " for Search Reddit).
    Prefix(String),
    /// Search for this (an engine suggestion).
    Search(String),
}

/// rippl's own pages, offered by name (the address bar commands, but
/// only ever as rows under what was typed: Return still does what it did).
const PAGES: &[(&str, &str)] =
    &[("history", "History"), ("bookmarks", "Bookmarks"), ("passwords", "Passwords"), ("settings", "Settings")];

const SHOWN: usize = 8;

impl Browser {
    pub(crate) fn wire_omnibox(&self) {
        let weak = self.weak();
        self.field.connect_activate(move |field| {
            let Some(b) = weak.upgrade() else { return };
            let picked = b.suggest_list.selected_row().map(|r| r.index());
            match picked.and_then(|i| b.offers.borrow().get(i as usize).cloned()) {
                Some(offer) => b.take(offer),
                None => {
                    let text = field.text().to_string();
                    b.hide_offers();
                    b.go(&text);
                }
            }
        });

        let weak = self.weak();
        self.field.connect_changed(move |field| {
            let Some(b) = weak.upgrade() else { return };
            if b.completing.get() || !field.has_focus() {
                return;
            }
            b.typed_changed(&field.text());
        });

        let weak = self.weak();
        self.suggest_list.connect_row_activated(move |_, row| {
            let Some(b) = weak.upgrade() else { return };
            let offer = b.offers.borrow().get(row.index() as usize).cloned();
            if let Some(offer) = offer {
                b.take(offer);
            }
        });

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Escape => {
                    b.hide_offers();
                    if let Some(view) = b.front_view() {
                        view.grab_focus();
                    }
                    b.show_front();
                    glib::Propagation::Stop
                }
                gdk::Key::Down | gdk::Key::Up if b.suggest_area.is_visible() => {
                    b.move_pick(if key == gdk::Key::Down { 1 } else { -1 });
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.field.add_controller(keys);

        let focus = gtk::EventControllerFocus::new();
        let weak = self.weak();
        focus.connect_enter(move |_| {
            let Some(b) = weak.upgrade() else { return };
            b.typed.borrow_mut().clear();
            b.offer(&[], "");
        });
        let weak = self.weak();
        focus.connect_leave(move |_| {
            if let Some(b) = weak.upgrade() {
                b.hide_offers();
            }
        });
        self.field.add_controller(focus);
    }

    /// The text in the field changed because of typing.
    fn typed_changed(&self, text: &str) {
        let grew = {
            let typed = self.typed.borrow();
            text.len() > typed.len() && text.starts_with(typed.as_str())
        } && self.field.position() as usize == text.chars().count();
        *self.typed.borrow_mut() = text.to_string();

        let mut places = if text.trim().is_empty() {
            Vec::new()
        } else {
            self.places.search(text, SHOWN).unwrap_or_default()
        };
        // A typo still finds the page, ranked below every exact match.
        if !text.trim().is_empty() && places.len() < SHOWN {
            let recent = self.places.recent(2000).unwrap_or_default();
            for p in suggest::rank_fuzzy(text, recent, rippl_core::places::now(), SHOWN) {
                if places.len() < SHOWN && !places.iter().any(|q| q.url == p.url) {
                    places.push(p);
                }
            }
        }
        // Completing only while typing forward: never after Backspace.
        if grew
            && let Some(full) = suggest::complete(text, &places) {
                self.completing.set(true);
                self.field.set_text(&full);
                self.field.select_region(text.chars().count() as i32, -1);
                self.completing.set(false);
            }
        self.offer(&places, text);
        self.ask_engine(text);
    }

    /// Suggestions from the search engine, only with `suggestions = on`:
    /// a moment after the typing stops, from a session of their own (no
    /// cookies), never for something that looks like an address.
    fn ask_engine(&self, text: &str) {
        let text = text.trim().to_string();
        let config = self.config.borrow();
        let Some(endpoint) = rippl_core::address::suggest_endpoint(&config.search).filter(|_| config.suggestions) else {
            return;
        };
        drop(config);
        if text.is_empty() || rippl_core::address::url_from(&text).is_some() || text.len() > 200 {
            return;
        }
        let generation = self.typing_gen.get().wrapping_add(1);
        self.typing_gen.set(generation);
        let weak = self.weak();
        glib::timeout_add_local_once(std::time::Duration::from_millis(180), move || {
            let Some(b) = weak.upgrade() else { return };
            if b.typing_gen.get() != generation {
                return; // more was typed since
            }
            let url = endpoint.replacen("{}", &rippl_core::address::encode_query(&text), 1);
            glib::MainContext::default().spawn_local(async move {
                let Ok(json) = crate::shield::fetch_text(&url).await else { return };
                let list = rippl_core::address::parse_suggestions(&json);
                if b.typing_gen.get() != generation || *b.typed.borrow() != text {
                    return;
                }
                *b.engine_says.borrow_mut() = (text.clone(), list);
                let places = b.places.search(&text, SHOWN).unwrap_or_default();
                b.offer(&places, &text);
            });
        });
    }

    /// Fills the list under the field. Nothing typed: bookmarks, then
    /// recent pages.
    pub(crate) fn offer(&self, places: &[Place], typed: &str) {
        let mut offers = Vec::new();
        let mut rows: Vec<(String, String, &'static str)> = Vec::new(); // title, detail, kind

        // The new tab's empty field offers nothing: the page
        // stays quiet, with only the Home grid under it.
        if typed.trim().is_empty() && self.field_at_home() {
            self.hide_offers();
            return;
        }
        if typed.trim().is_empty() {
            let mut seen = Vec::new();
            for p in self.places.bookmarks().unwrap_or_default().into_iter().take(SHOWN) {
                seen.push(p.url.clone());
                rows.push((title_of(&p), suggest::bare(&p.url).to_string(), "bookmark"));
                offers.push(Offer::Page(p.url));
            }
            for p in self.places.recent(SHOWN * 2).unwrap_or_default() {
                if offers.len() >= SHOWN * 2 || seen.contains(&p.url) {
                    continue;
                }
                rows.push((title_of(&p), suggest::bare(&p.url).to_string(), "history"));
                offers.push(Offer::Page(p.url));
            }
        } else {
            let words: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
            let front = self.front();
            for tab in self.tabs.borrow().list.iter() {
                if Some(tab.id) == front || tab.url.is_empty() || offers.len() >= 3 {
                    continue;
                }
                let hay = format!("{} {}", tab.title, tab.url).to_lowercase();
                if words.iter().all(|w| hay.contains(w.as_str())) {
                    rows.push((tab.title.clone(), "Switch to tab".into(), "tab"));
                    offers.push(Offer::Tab(tab.id));
                }
            }
            for p in places {
                if offers.iter().any(|o| matches!(o, Offer::Page(u) if *u == p.url)) {
                    continue;
                }
                let kind = if p.bookmarked { "bookmark" } else { "history" };
                rows.push((title_of(p), suggest::bare(&p.url).to_string(), kind));
                offers.push(Offer::Page(p.url.clone()));
            }
            // "red" → Search Reddit.
            for (name, word) in rippl_core::address::site_search(typed) {
                rows.push((format!("Search {name}"), format!("or type \"{word} \" and your words"), "site"));
                offers.push(Offer::Prefix(format!("{word} ")));
            }
            // rippl's own pages by name.
            let t = typed.trim().to_lowercase();
            if t.chars().count() >= 3 {
                for (page, name) in PAGES {
                    if page.starts_with(&t) {
                        rows.push((format!("Open {name}"), format!("rippl://{page}"), "command"));
                        offers.push(Offer::Page(format!("rippl://{page}")));
                    }
                }
            }
            // The engine's suggestions, when asked for and still current.
            let said = self.engine_says.borrow();
            if said.0 == typed.trim() {
                for q in &said.1 {
                    rows.push((q.clone(), "Search".into(), "suggest"));
                    offers.push(Offer::Search(q.clone()));
                }
            }
        }

        while let Some(child) = self.suggest_list.first_child() {
            self.suggest_list.remove(&child);
        }
        for (title, detail, kind) in &rows {
            let row = gtk::ListBoxRow::new();
            // Rows never take the focus from the field: clicking one must
            // not end the typing before the click lands.
            row.set_focusable(false);
            let lines = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let first = gtk::Label::builder()
                .label(match *kind {
                    "bookmark" => format!("★ {title}"),
                    "suggest" => format!("⌕ {title}"),
                    "site" | "command" => format!("→ {title}"),
                    _ => title.clone(),
                })
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .build();
            let second =
                gtk::Label::builder()
                    .label(detail)
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::Middle)
                    .max_width_chars(1)
                    .build();
            second.add_css_class("dim");
            lines.append(&first);
            lines.append(&second);
            row.set_child(Some(&lines));
            self.suggest_list.append(&row);
        }
        *self.offers.borrow_mut() = offers;
        let any = !rows.is_empty();
        self.suggest_area.set_visible(any);
        // On the new tab the offers hang under the field and the column
        // keeps its tabs; the Home grid makes way while you type.
        self.tabs_area.set_visible(!any || self.field_at_home());
        if self.field_at_home() {
            self.home_grid.set_visible(!any && self.home_grid.first_child().is_some());
        }
    }

    pub(crate) fn hide_offers(&self) {
        self.suggest_list.unselect_all();
        self.suggest_area.set_visible(false);
        self.tabs_area.set_visible(true);
        if self.field_at_home() {
            self.home_grid.set_visible(self.home_grid.first_child().is_some());
        }
        self.typed.borrow_mut().clear();
    }

    fn move_pick(&self, step: i32) {
        let n = self.offers.borrow().len() as i32;
        if n == 0 {
            return;
        }
        let now = self.suggest_list.selected_row().map_or(-1, |r| r.index());
        let next = now + step;
        if next < 0 {
            self.suggest_list.unselect_all();
            return;
        }
        if let Some(row) = self.suggest_list.row_at_index(next.min(n - 1)) {
            self.suggest_list.select_row(Some(&row));
        }
    }

    /// Opens what was offered.
    fn take(&self, offer: Offer) {
        self.hide_offers();
        match offer {
            Offer::Tab(id) => self.select(id),
            Offer::Page(url) => self.go(&url),
            Offer::Search(words) => {
                let search = self.config.borrow().search.clone();
                self.go(&search.replacen("{}", &rippl_core::address::encode_query(&words), 1));
            }
            Offer::Prefix(text) => {
                // Keep typing after it: the field keeps the keys.
                self.field.grab_focus();
                self.field.set_text(&text);
                self.field.set_position(-1);
                self.typed_changed(&text);
            }
        }
    }

    /// The offers as text, for the control socket's `type` command.
    pub(crate) fn offers_text(&self) -> String {
        let field = self.field.text();
        let mut out = vec![format!("field\t{field}")];
        for offer in self.offers.borrow().iter() {
            out.push(match offer {
                Offer::Tab(id) => format!("tab\t{id}"),
                Offer::Page(url) => format!("page\t{url}"),
                Offer::Prefix(t) => format!("prefix\t{t}"),
                Offer::Search(q) => format!("search\t{q}"),
            });
        }
        out.join("\n")
    }

    /// As if typed: for testing the field without a keyboard.
    pub(crate) fn type_text(&self, text: &str) {
        self.field.grab_focus();
        self.typed.borrow_mut().clear();
        // One character at a time, as typing does, so completion runs.
        let mut so_far = String::new();
        for c in text.chars() {
            so_far.push(c);
            self.completing.set(true);
            self.field.set_text(&so_far);
            self.field.set_position(-1);
            self.completing.set(false);
            self.typed_changed(&so_far);
        }
    }
}

fn title_of(p: &Place) -> String {
    if p.title.trim().is_empty() { suggest::bare(&p.url).trim_end_matches('/').to_string() } else { p.title.clone() }
}
