//! Split view: two tabs side by side, a divider between them.
//!
//! - Ctrl+Alt+S splits the tab in front with the tab you were on before
//!   it, or separates a split. Alt-click on a tab in the column splits it
//!   with the one in front.
//! - The pane you click into becomes the tab in front: shortcuts, the
//!   field, find and reading mode act on it. Ctrl+Alt+←/→ move between
//!   the panes.
//! - Both pages are on screen, so neither sleeps while shown.
//! - Selecting a tab of the pair shows the pair; selecting any other tab
//!   shows it alone, and the pair comes back when you return to either.
//!
//! A view lives in the stage (one page at a time) or in the paned (two),
//! never both: `place` and `detach` are the only ways views move.

use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{gtk, WebView};

use crate::browser::Browser;

impl Browser {
    /// Shows tab `id` (already awake, or blank): alone, or beside its
    /// partner if it has one.
    pub(crate) fn place(&self, id: TabId) {
        let view = self.views.borrow().get(&id).cloned();
        let partner = self.partner.borrow().get(&id).copied();
        let Some(view) = view else {
            self.unsplit_layout();
            self.stage.set_visible_child(&self.blank);
            self.field.grab_focus();
            return;
        };
        if let Some(p) = partner {
            let asleep = self.tabs.borrow().get(p).is_some_and(|t| t.life != Life::Awake && !t.url.is_empty());
            if asleep {
                self.wake_for_split(p);
            }
            if let Some(other) = self.views.borrow().get(&p).cloned() {
                self.show_pair(id, &view, p, &other);
                view.grab_focus();
                return;
            }
        }
        self.unsplit_layout();
        self.stage.set_visible_child(&view);
        view.grab_focus();
    }

    fn show_pair(&self, a: TabId, va: &WebView, b: TabId, vb: &WebView) {
        // Left is whichever comes first in the column.
        let (ia, ib) = {
            let tabs = self.tabs.borrow();
            (tabs.index(a).unwrap_or(0), tabs.index(b).unwrap_or(0))
        };
        let (left, right) = if ia <= ib { ((a, va), (b, vb)) } else { ((b, vb), (a, va)) };
        let shown = *self.shown_split.borrow();
        if shown != Some((left.0, right.0)) {
            self.unsplit_layout();
            self.detach(left.1);
            self.detach(right.1);
            self.paned.set_start_child(Some(left.1));
            self.paned.set_end_child(Some(right.1));
            let width = self.paned.width();
            if width > 0 {
                self.paned.set_position(width / 2);
            }
            *self.shown_split.borrow_mut() = Some((left.0, right.0));
        }
        self.stage.set_visible_child(&self.paned);
        self.mark_pane(a);
    }

    /// Outlines the pane that has the keyboard.
    pub(crate) fn mark_pane(&self, front: TabId) {
        let Some((l, r)) = *self.shown_split.borrow() else { return };
        for id in [l, r] {
            if let Some(v) = self.views.borrow().get(&id) {
                if id == front { v.add_css_class("pane-front") } else { v.remove_css_class("pane-front") }
            }
        }
    }

    /// Puts a shown pair back into the stage, one page each.
    pub(crate) fn unsplit_layout(&self) {
        let Some((l, r)) = self.shown_split.borrow_mut().take() else { return };
        self.paned.set_start_child(None::<&gtk::Widget>);
        self.paned.set_end_child(None::<&gtk::Widget>);
        for id in [l, r] {
            if let Some(v) = self.views.borrow().get(&id).cloned() {
                v.remove_css_class("pane-front");
                if v.parent().is_none() {
                    self.stage.add_named(&v, Some(&format!("tab-{id}")));
                }
            }
        }
    }

    /// Takes a view off wherever it is shown.
    pub(crate) fn detach(&self, view: &WebView) {
        let Some(parent) = view.parent() else { return };
        if parent == *self.paned.upcast_ref::<gtk::Widget>() {
            if self.paned.start_child().as_ref() == Some(view.upcast_ref()) {
                self.paned.set_start_child(None::<&gtk::Widget>);
            } else {
                self.paned.set_end_child(None::<&gtk::Widget>);
            }
        } else {
            self.stage.remove(view);
        }
    }

    /// The tabs on screen: the one in front, and its partner when shown.
    pub(crate) fn on_screen(&self) -> Vec<TabId> {
        let mut ids: Vec<TabId> = self.front().into_iter().collect();
        if let Some((l, r)) = *self.shown_split.borrow() {
            ids.extend([l, r]);
        }
        ids
    }

    /// Splits the tab in front with `other`.
    pub(crate) fn split_with(&self, other: TabId) {
        let Some(front) = self.front() else { return };
        if front == other || self.tabs.borrow().get(other).is_none_or(|t| t.url.is_empty()) {
            return;
        }
        self.separate(front);
        self.separate(other);
        let mut partner = self.partner.borrow_mut();
        partner.insert(front, other);
        partner.insert(other, front);
        drop(partner);
        self.select(front);
    }

    /// Ctrl+Alt+S: separate the split in front, or split the front tab
    /// with the tab used before it.
    pub(crate) fn toggle_split(&self) {
        let Some(front) = self.front() else { return };
        if self.partner.borrow().contains_key(&front) {
            self.separate(front);
            self.select(front);
            return;
        }
        let previous = {
            let tabs = self.tabs.borrow();
            tabs.list
                .iter()
                .filter(|t| t.id != front && !t.url.is_empty())
                .max_by_key(|t| t.touched)
                .map(|t| t.id)
        };
        match previous {
            Some(p) => self.split_with(p),
            None => self.notice("Open another page to put beside this one"),
        }
    }

    /// Ends `id`'s pair, if it is in one.
    pub(crate) fn separate(&self, id: TabId) {
        let other = self.partner.borrow_mut().remove(&id);
        if let Some(o) = other {
            self.partner.borrow_mut().remove(&o);
            let shown = *self.shown_split.borrow();
            if shown.is_some_and(|(l, r)| [l, r].contains(&id)) {
                self.unsplit_layout();
            }
        }
    }

    /// Ctrl+Alt+←/→: the keyboard to the left or right pane.
    pub(crate) fn focus_pane(&self, right: bool) {
        let Some((l, r)) = *self.shown_split.borrow() else { return };
        let id = if right { r } else { l };
        if let Some(v) = self.views.borrow().get(&id).cloned() {
            v.grab_focus();
        }
    }

    /// A pane got the keyboard (a click in it): its tab is now in front,
    /// without moving anything.
    pub(crate) fn pane_focused(&self, id: TabId) {
        let shown = *self.shown_split.borrow();
        if !shown.is_some_and(|(l, r)| [l, r].contains(&id)) || self.front() == Some(id) {
            return;
        }
        self.tabs.borrow_mut().select(id);
        self.mark_pane(id);
        self.show_front();
        self.show_question();
        self.show_save_question();
        self.render();
    }
}
