//! Split view: up to four tabs on screen at once (the 
//! and ), dividers between them.
//!
//! - Ctrl+Alt+S splits the tab in front with the tab you were on before
//!   it, or takes it out of its split. Alt-click on a tab in the column
//!   (or "Put beside this tab" in its menu) adds it to the split in front,
//!   up to four.
//! - Two and three pages stand side by side; four make a two-by-two grid.
//!   The dividers drag.
//! - The pane you click into becomes the tab in front: shortcuts, the
//!   field, find and reading mode act on it. Ctrl+Alt+←/→ move between
//!   the panes.
//! - All of them are on screen, so none sleeps while shown.
//! - Selecting a tab of a split shows the split; selecting any other tab
//!   shows it alone, and the split comes back when you return to one of it.
//!
//! A view lives in the stage (one page at a time) or in the split area,
//! never both: `place` and `detach` are the only ways views move.

use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{glib, gtk, WebView};

use crate::browser::Browser;

/// The most pages one split shows.
pub(crate) const MOST: usize = 4;

impl Browser {
    /// The split `id` is in, in column order, if any.
    pub(crate) fn split_of(&self, id: TabId) -> Option<Vec<TabId>> {
        let found = self.splits.borrow().iter().find(|s| s.contains(&id)).cloned()?;
        let tabs = self.tabs.borrow();
        let mut ordered = found;
        ordered.sort_by_key(|t| tabs.index(*t).unwrap_or(usize::MAX));
        Some(ordered)
    }

    /// Whether `id` is in a split.
    pub(crate) fn in_split(&self, id: TabId) -> bool {
        self.splits.borrow().iter().any(|s| s.contains(&id))
    }

    /// Shows tab `id` (already awake, or blank): alone, or with its split.
    pub(crate) fn place(&self, id: TabId) {
        let view = self.views.borrow().get(&id).cloned();
        let Some(view) = view else {
            self.unsplit_layout();
            self.stage.set_visible_child(&self.blank);
            self.field.grab_focus();
            return;
        };
        if let Some(members) = self.split_of(id) {
            for m in &members {
                let asleep = self.tabs.borrow().get(*m).is_some_and(|t| t.life != Life::Awake && !t.url.is_empty());
                if asleep {
                    self.wake_for_split(*m);
                }
            }
            let views: Vec<(TabId, WebView)> =
                members.iter().filter_map(|m| self.views.borrow().get(m).cloned().map(|v| (*m, v))).collect();
            if views.len() >= 2 {
                self.show_split(&views);
                self.mark_pane(id);
                view.grab_focus();
                return;
            }
        }
        self.unsplit_layout();
        self.stage.set_visible_child(&view);
        view.grab_focus();
    }

    /// Lays `views` out in the split area: across, or two by two.
    fn show_split(&self, views: &[(TabId, WebView)]) {
        let ids: Vec<TabId> = views.iter().map(|(t, _)| *t).collect();
        if self.shown_split.borrow().as_ref() == Some(&ids) {
            self.stage.set_visible_child(&self.split_area);
            return;
        }
        self.unsplit_layout();
        for (_, v) in views {
            self.detach(v);
        }
        let pane = |o: gtk::Orientation, a: &gtk::Widget, b: &gtk::Widget| {
            let p = gtk::Paned::builder()
                .orientation(o)
                .wide_handle(true)
                .shrink_start_child(false)
                .shrink_end_child(false)
                .resize_start_child(true)
                .resize_end_child(true)
                .start_child(a)
                .end_child(b)
                .build();
            p.add_css_class("split");
            p
        };
        let w = |i: usize| views[i].1.clone().upcast::<gtk::Widget>();
        let h = gtk::Orientation::Horizontal;
        let root = match views.len() {
            2 => pane(h, &w(0), &w(1)),
            3 => {
                let right = pane(h, &w(1), &w(2));
                pane(h, &w(0), right.upcast_ref())
            }
            _ => {
                let top = pane(h, &w(0), &w(1));
                let bottom = pane(h, &w(2), &w(3));
                pane(gtk::Orientation::Vertical, top.upcast_ref(), bottom.upcast_ref())
            }
        };
        root.set_hexpand(true);
        root.set_vexpand(true);
        self.split_area.append(&root);
        *self.shown_split.borrow_mut() = Some(ids);
        self.stage.set_visible_child(&self.split_area);
        // Even shares, once there is a size to share.
        let root = root.clone();
        glib::idle_add_local_once(move || even(&root));
    }

    /// Outlines the pane that has the keyboard.
    pub(crate) fn mark_pane(&self, front: TabId) {
        let Some(shown) = self.shown_split.borrow().clone() else { return };
        for id in shown {
            if let Some(v) = self.views.borrow().get(&id) {
                if id == front { v.add_css_class("pane-front") } else { v.remove_css_class("pane-front") }
            }
        }
    }

    /// Puts a shown split back into the stage, one page each.
    pub(crate) fn unsplit_layout(&self) {
        let Some(shown) = self.shown_split.borrow_mut().take() else { return };
        for id in &shown {
            if let Some(v) = self.views.borrow().get(id).cloned() {
                self.detach(&v);
            }
        }
        while let Some(c) = self.split_area.first_child() {
            self.split_area.remove(&c);
        }
        for id in shown {
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
        if let Some(p) = parent.downcast_ref::<gtk::Paned>() {
            if p.start_child().as_ref() == Some(view.upcast_ref()) {
                p.set_start_child(None::<&gtk::Widget>);
            } else {
                p.set_end_child(None::<&gtk::Widget>);
            }
        } else if parent == *self.stage.upcast_ref::<gtk::Widget>() {
            self.stage.remove(view);
        } else {
            view.unparent();
        }
    }

    /// The tabs on screen: the one in front, and its split when shown.
    pub(crate) fn on_screen(&self) -> Vec<TabId> {
        let mut ids: Vec<TabId> = self.front().into_iter().collect();
        if let Some(shown) = self.shown_split.borrow().as_ref() {
            ids.extend(shown.iter().copied());
        }
        ids
    }

    /// Puts `other` beside the tab in front: into its split (up to four),
    /// or a new pair.
    pub(crate) fn split_with(&self, other: TabId) {
        let Some(front) = self.front() else { return };
        if front == other || self.tabs.borrow().get(other).is_none_or(|t| t.url.is_empty()) {
            return;
        }
        if self.split_of(front).is_some_and(|s| s.contains(&other)) {
            return;
        }
        let full = self.split_of(front).is_some_and(|s| s.len() >= MOST);
        if full {
            self.notice("A split shows four pages at most");
            return;
        }
        self.separate(other);
        let mut splits = self.splits.borrow_mut();
        match splits.iter_mut().find(|s| s.contains(&front)) {
            Some(s) => s.push(other),
            None => splits.push(vec![front, other]),
        }
        drop(splits);
        self.unsplit_layout();
        self.select(front);
    }

    /// Ctrl+Alt+S: take the tab in front out of its split, or split it with
    /// the tab used before it.
    pub(crate) fn toggle_split(&self) {
        let Some(front) = self.front() else { return };
        if self.in_split(front) {
            self.separate(front);
            self.select(front);
            return;
        }
        let previous = {
            let tabs = self.tabs.borrow();
            tabs.list
                .iter()
                .filter(|t| t.id != front && !t.url.is_empty() && t.space == tabs.space)
                .max_by_key(|t| t.touched)
                .map(|t| t.id)
        };
        match previous {
            Some(p) => self.split_with(p),
            None => self.notice("Open another page to put beside this one"),
        }
    }

    /// Takes `id` out of its split; a split left with one page ends.
    pub(crate) fn separate(&self, id: TabId) {
        if !self.in_split(id) {
            return;
        }
        if self.shown_split.borrow().as_ref().is_some_and(|s| s.contains(&id)) {
            self.unsplit_layout();
        }
        let mut splits = self.splits.borrow_mut();
        for s in splits.iter_mut() {
            s.retain(|t| *t != id);
        }
        splits.retain(|s| s.len() >= 2);
    }

    /// Ctrl+Alt+←/→: the keyboard to the pane before or after.
    pub(crate) fn focus_pane(&self, right: bool) {
        let Some(shown) = self.shown_split.borrow().clone() else { return };
        let at = self.front().and_then(|f| shown.iter().position(|t| *t == f)).unwrap_or(0);
        let n = shown.len();
        let next = if right { (at + 1) % n } else { (at + n - 1) % n };
        if let Some(v) = self.views.borrow().get(&shown[next]).cloned() {
            v.grab_focus();
        }
    }

    /// A pane got the keyboard (a click in it): its tab is now in front,
    /// without moving anything.
    pub(crate) fn pane_focused(&self, id: TabId) {
        let shown = self.shown_split.borrow().clone();
        if !shown.is_some_and(|s| s.contains(&id)) || self.front() == Some(id) {
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

/// Even shares for every divider under `root`: each pane of a row the
/// same width (three across: a third, then half of the rest).
fn even(root: &gtk::Paned) {
    let size = if root.orientation() == gtk::Orientation::Horizontal { root.width() } else { root.height() };
    if size <= 0 {
        return;
    }
    let ends = |w: &gtk::Widget| -> i32 { w.downcast_ref::<gtk::Paned>().filter(|p| p.orientation() == root.orientation()).map_or(1, |_| 2) };
    let after = root.end_child().map_or(1, |c| ends(&c));
    root.set_position(size / (after + 1));
    for child in [root.start_child(), root.end_child()].into_iter().flatten() {
        if let Some(p) = child.downcast_ref::<gtk::Paned>() {
            let p = p.clone();
            glib::idle_add_local_once(move || even(&p));
        }
    }
}
