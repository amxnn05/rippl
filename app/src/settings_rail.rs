//! Settings' sections in the column (the Settings: its sections down
//! the left): while rippl://settings is the tab in front, the column shows
//! them where the tabs are, under "‹ Tabs" to go back, "Settings" and Find
//! a setting, the chosen one on a white card. A click shows that section
//! in the page; the page's own rail stands aside. With tabs across the top
//! or the column as icons, the page keeps its own rail.

use rippl_core::pages::CATEGORIES;
use webkit6::prelude::*;
use webkit6::{gio, gtk};

use crate::browser::Browser;

/// The column's settings rail and its parts.
pub(crate) struct Rail {
    pub rail: gtk::Box,
    find: gtk::SearchEntry,
    rows: Vec<(String, gtk::Button)>,
    back: gtk::Button,
}

impl Rail {
    pub(crate) fn new() -> Rail {
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 2);
        rail.add_css_class("settings-rail");
        rail.set_visible(false);
        let back = gtk::Button::builder().focusable(false).build();
        let back_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        back_line.append(&gtk::Label::new(Some("‹")));
        back_line.append(&gtk::Label::new(Some("Tabs")));
        back.set_child(Some(&back_line));
        back.add_css_class("rail-back");
        back.set_tooltip_text(Some("Back to your tabs"));
        let title = gtk::Label::builder().label("Settings").xalign(0.0).build();
        title.add_css_class("rail-title");
        let find = gtk::SearchEntry::builder().placeholder_text("Find a setting").build();
        find.add_css_class("rail-find");
        rail.append(&back);
        rail.append(&title);
        rail.append(&find);
        let mut rows = Vec::new();
        for (n, (id, name, glyph)) in CATEGORIES.iter().enumerate() {
            // the three blocks, a hairline between them.
            if n == 6 || n == 10 {
                let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
                rule.add_css_class("rail-rule");
                rail.append(&rule);
            }
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            let g = gtk::Label::new(Some(glyph));
            g.add_css_class("rail-glyph");
            g.set_width_chars(2);
            line.append(&g);
            line.append(&gtk::Label::builder().label(*name).xalign(0.0).hexpand(true).build());
            let row = gtk::Button::builder().child(&line).focusable(false).build();
            row.add_css_class("rail-row");
            rail.append(&row);
            rows.push((id.to_string(), row));
        }
        Rail { rail, find, rows, back }
    }
}

impl Browser {
    pub(crate) fn wire_settings_rail(&self) {
        for (id, row) in &self.rail.rows {
            let weak = self.weak();
            let id = id.clone();
            row.connect_clicked(move |_| {
                let Some(b) = weak.upgrade() else { return };
                b.rail.find.set_text("");
                b.settings_page_js(&format!("location.hash = '#{id}'"));
                b.mark_rail(&id);
            });
        }
        let weak = self.weak();
        self.rail.find.connect_search_changed(move |e| {
            let Some(b) = weak.upgrade() else { return };
            let q = serde_json::to_string(&e.text().to_string()).unwrap_or_default();
            b.settings_page_js(&format!(
                "(() => {{ const f = document.getElementById('find'); f.value = {q}; f.dispatchEvent(new Event('input')); }})()"
            ));
            if e.text().is_empty() {
                b.place_settings_rail();
            } else {
                for (_, row) in &b.rail.rows {
                    row.remove_css_class("here");
                }
            }
        });
        let weak = self.weak();
        self.rail.back.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.rail_away.set(true);
                b.render();
            }
        });
    }

    /// Whether the column shows Settings' sections now.
    pub(crate) fn settings_in_column(&self) -> bool {
        let c = self.config.borrow();
        if c.top_tabs || c.icon_column || self.rail_away.get() {
            return false;
        }
        drop(c);
        self.settings_front_url().is_some()
    }

    /// The address of the settings page in front, if Settings is in front.
    fn settings_front_url(&self) -> Option<String> {
        let tabs = self.tabs.borrow();
        let url = tabs.front.and_then(|f| tabs.get(f)).map(|t| t.url.clone())?;
        url.starts_with("rippl://settings").then_some(url)
    }

    /// Shows the rail or the tabs, as the tab in front asks (from render).
    pub(crate) fn place_settings_rail(&self) {
        let on = self.settings_in_column();
        self.rail.rail.set_visible(on);
        self.pins.set_visible(!on && self.pins.first_child().is_some());
        self.list.set_visible(!on);
        self.new_tab.set_visible(!on);
        if on {
            let url = self.settings_front_url().unwrap_or_default();
            let id = url.split_once('#').map(|(_, h)| h.to_string()).unwrap_or_else(|| "general".into());
            if self.rail.find.text().is_empty() {
                self.mark_rail(&id);
            }
        }
        // The page's own rail stands aside while the column has it.
        self.settings_page_js(&format!("document.body.classList.toggle('rail-in-column', {on})"));
    }

    /// For the control socket: press a section's row, type in Find a
    /// setting (`find <words>`), or "‹ Tabs" (`back`); then what shows.
    pub(crate) fn rail_command(&self, what: &str) -> String {
        match what.split_once(' ').unwrap_or((what, "")) {
            ("back", _) => self.rail.back.emit_clicked(),
            ("find", words) => self.rail.find.set_text(words),
            (id, _) => {
                if let Some((_, row)) = self.rail.rows.iter().find(|(r, _)| r == id) {
                    row.emit_clicked();
                }
            }
        }
        let here = self.rail.rows.iter().find(|(_, r)| r.has_css_class("here")).map_or("-", |(id, _)| id.as_str());
        format!("rail={} tabs={} here={here}", self.rail.rail.is_visible(), self.list.is_visible())
    }

    fn mark_rail(&self, id: &str) {
        for (row_id, row) in &self.rail.rows {
            if row_id == id {
                row.add_css_class("here");
            } else {
                row.remove_css_class("here");
            }
        }
    }

    /// Runs `code` in the settings page in front (its own world: the page's
    /// script is there).
    fn settings_page_js(&self, code: &str) {
        if self.settings_front_url().is_none() {
            return;
        }
        if let Some(v) = self.front_view() {
            v.evaluate_javascript(code, None, None, None::<&gio::Cancellable>, |_| {});
        }
    }
}
