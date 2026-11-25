//! Find in page (Ctrl+F): a small bar over the top right of the page.
//! Typing searches as you go, case-insensitive, wrapping around; Return
//! goes to the next match, Shift+Return to the previous, Esc closes the bar
//! and clears the highlights. The bar says how many matches there are.

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gdk, glib, gtk, FindOptions, WebView};

use crate::browser::Browser;

const MAX_MATCHES: u32 = 1000;

fn options() -> u32 {
    (FindOptions::CASE_INSENSITIVE | FindOptions::WRAP_AROUND).bits()
}

impl Browser {
    pub(crate) fn find_bar() -> (gtk::Box, gtk::Entry, gtk::Label) {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bar.add_css_class("find");
        bar.set_halign(gtk::Align::End);
        bar.set_valign(gtk::Align::Start);
        bar.set_margin_top(10);
        bar.set_margin_end(14);
        let entry = gtk::Entry::builder().placeholder_text("Find in page").width_chars(22).build();
        let count = gtk::Label::new(None);
        count.add_css_class("dim");
        bar.append(&entry);
        bar.append(&count);
        bar.set_visible(false);
        (bar, entry, count)
    }

    pub(crate) fn wire_find(&self) {
        let weak = self.weak();
        self.find_entry.connect_changed(move |entry| {
            let Some(b) = weak.upgrade() else { return };
            b.find(&entry.text());
        });
        let keys = gtk::EventControllerKey::new();
        let weak = self.weak();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            let Some(finder) = b.front_view().and_then(|v| v.find_controller()) else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                        finder.search_previous();
                    } else {
                        finder.search_next();
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    b.close_find();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.find_entry.add_controller(keys);
    }

    pub(crate) fn open_find(&self) {
        if self.front_view().is_none() {
            return;
        }
        self.find_bar.set_visible(true);
        self.find_entry.grab_focus();
        self.find_entry.select_region(0, -1);
        let text = self.find_entry.text();
        if !text.is_empty() {
            self.find(&text);
        }
    }

    pub(crate) fn close_find(&self) {
        self.find_bar.set_visible(false);
        if let Some(view) = self.front_view() {
            if let Some(finder) = view.find_controller() {
                finder.search_finish();
            }
            view.grab_focus();
        }
    }

    /// Searches the front page for `text`, highlighting and counting.
    pub(crate) fn find(&self, text: &str) {
        let Some(finder) = self.front_view().and_then(|v| v.find_controller()) else { return };
        if text.is_empty() {
            finder.search_finish();
            self.find_count.set_label("");
            return;
        }
        finder.search(text, options(), MAX_MATCHES);
        finder.count_matches(text, options(), MAX_MATCHES);
    }

    /// The count and "no matches" come back from the page asynchronously.
    pub(crate) fn wire_find_for(&self, id: TabId, view: &WebView) {
        let Some(finder) = view.find_controller() else { return };
        let weak = self.weak();
        finder.connect_counted_matches(move |_, n| {
            let Some(b) = weak.upgrade() else { return };
            if b.front() == Some(id) {
                let label = match n {
                    0 => "No matches".to_string(),
                    1 => "1 match".to_string(),
                    n if n >= MAX_MATCHES => format!("{MAX_MATCHES}+ matches"),
                    n => format!("{n} matches"),
                };
                b.find_count.set_label(&label);
                b.found.set(n);
            }
        });
        let weak = self.weak();
        finder.connect_failed_to_find_text(move |_| {
            let Some(b) = weak.upgrade() else { return };
            if b.front() == Some(id) {
                b.find_count.set_label("No matches");
                b.found.set(0);
            }
        });
    }
}
