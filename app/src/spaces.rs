//! Spaces: separate sets of tabs in the one window, each with its own
//! cookies and sign-ins.
//!
//! - The first space is everything there was before spaces: its tabs and
//!   its sign-ins (the store rippl always had), so making a second space
//!   signs nobody out.
//! - A new space gets its own store (`spaces/<id>/` in the data folder):
//!   signed in to a site in one space, you're not in another.
//! - Switching swaps the column's tabs. The tabs left behind are parked:
//!   still playing, and they sleep after the usual wait like any tab.
//! - Alt+1–9 switch. One button per
//!   space at the column's foot; **+** makes one (Ctrl+Alt+T). Right-click a
//!   space's button to rename or delete it. Deleting a space closes its
//!   tabs and forgets its sign-ins and site data.
//! - Tabs move between spaces from the tab menu.

use rippl_core::tabs::{SpaceId, TabId};
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

impl Browser {
    /// Shows `space`, with the tab it had in front.
    pub(crate) fn switch_space(&self, space: SpaceId) {
        if !self.tabs.borrow().spaces.iter().any(|s| s.id == space) {
            return;
        }
        self.unsplit_layout();
        let front = self.tabs.borrow_mut().switch_space(space);
        self.apply_space_look();
        match front {
            Some(id) => self.select(id),
            None => {
                self.stage.set_visible_child(&self.blank);
                self.show_front();
                self.render();
                self.field.grab_focus();
            }
        }
        self.dirty.set(true);
    }

    /// Ctrl+Alt+T, or the foot's +: a new space, shown at once.
    pub(crate) fn new_space(&self) {
        let id = self.tabs.borrow_mut().add_space("");
        self.switch_space(id);
        let name = self.tabs.borrow().spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        self.notice(&format!("{name}: its own tabs and its own sign-ins"));
    }

    /// Deletes `space` (never the last): its tabs close, its sign-ins and
    /// site data are forgotten.
    pub(crate) fn delete_space(&self, space: SpaceId) {
        let doomed = {
            let tabs = self.tabs.borrow();
            if tabs.spaces.len() <= 1 {
                return;
            }
            tabs.list.iter().filter(|t| t.space == space).map(|t| t.id).collect::<Vec<TabId>>()
        };
        for id in &doomed {
            if let Some(view) = self.views.borrow_mut().remove(id) {
                self.detach(&view);
                view.terminate_web_process();
            }
            self.family.borrow_mut().remove(id);
        }
        let was_here = self.tabs.borrow().space == space;
        self.tabs.borrow_mut().remove_space(space);
        self.engine.forget_space(space);
        let front = self.front();
        if was_here {
            match front {
                Some(id) => self.select(id),
                None => self.switch_space(self.tabs.borrow().space),
            }
        }
        self.dirty.set(true);
        self.render();
    }

    /// Moves a tab to another space. A tab with a page reloads there, in
    /// that space's store (its sign-ins are that space's now).
    pub(crate) fn move_tab_to_space(&self, id: TabId, space: SpaceId) {
        if self.tabs.borrow().get(id).is_none_or(|t| t.space == space) {
            return;
        }
        let was_front = self.front() == Some(id);
        // Its page was made in the old space's store: let it go, and let
        // it come back (with its history) in the new one.
        if self.views.borrow().contains_key(&id) {
            self.sleep_tab(id);
        }
        self.tabs.borrow_mut().move_to_space(id, space);
        if was_front {
            let next = self.tabs.borrow().here().find(|t| !t.pinned).map(|t| t.id);
            match next {
                Some(n) => self.select(n),
                None => {
                    self.stage.set_visible_child(&self.blank);
                    self.show_front();
                }
            }
        }
        self.dirty.set(true);
        self.render();
    }

    /// The foot's space buttons: one per space, the one on screen lit.
    pub(crate) fn render_spaces(&self) {
        while let Some(child) = self.spaces_box.first_child() {
            self.spaces_box.remove(&child);
        }
        let tabs = self.tabs.borrow();
        // One space and nothing to switch: no buttons (nothing of spaces
        // shows until there are two), only the +.
        let many = tabs.spaces.len() > 1;
        for (n, space) in tabs.spaces.iter().enumerate() {
            if !many {
                break;
            }
            let letter = space.name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string());
            let button = gtk::Button::with_label(&letter.unwrap_or_else(|| (n + 1).to_string()));
            button.add_css_class("space");
            if let Some(c) = space.colours.first() {
                button.add_css_class(&format!("tint-{}", &c[1..]));
            }
            if space.id == tabs.space {
                button.add_css_class("here");
            }
            button.set_focusable(false);
            button.set_tooltip_text(Some(&format!("{} (Alt+{})", space.name, n + 1)));
            let id = space.id;
            // A left click is the button's own (a gesture on a button never
            // sees it: the button claims it first); the right one is ours.
            let weak = self.weak();
            button.connect_clicked(move |_| {
                let Some(b) = weak.upgrade() else { return };
                glib::idle_add_local_once(move || b.switch_space(id));
            });
            let click = gtk::GestureClick::builder().button(3).build();
            let weak = self.weak();
            click.connect_released(move |gesture, _, _, _| {
                let Some(b) = weak.upgrade() else { return };
                let Some(w) = gesture.widget() else { return };
                glib::idle_add_local_once(move || b.space_menu(id, &w));
            });
            button.add_controller(click);
            self.spaces_box.append(&button);
        }
        let add = gtk::Button::with_label("+");
        add.add_css_class("space");
        add.set_focusable(false);
        add.set_tooltip_text(Some("New space (Ctrl+Alt+T)"));
        let weak = self.weak();
        add.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.new_space();
            }
        });
        self.spaces_box.append(&add);
    }

    /// Right-click on a space's button: rename it, or delete it.
    fn space_menu(&self, space: SpaceId, anchor: &gtk::Widget) {
        let (name, count, many) = {
            let tabs = self.tabs.borrow();
            let name = tabs.spaces.iter().find(|s| s.id == space).map(|s| s.name.clone()).unwrap_or_default();
            (name, tabs.list.iter().filter(|t| t.space == space).count(), tabs.spaces.len() > 1)
        };
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 6);
        menu.set_margin_top(8);
        menu.set_margin_bottom(8);
        menu.set_margin_start(8);
        menu.set_margin_end(8);
        let entry = gtk::Entry::builder().text(&name).placeholder_text("Space name").build();
        let weak = self.weak();
        entry.connect_activate(move |e| {
            let Some(b) = weak.upgrade() else { return };
            b.tabs.borrow_mut().rename_space(space, &e.text());
            if let Some(p) = e.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                p.popdown();
            }
            b.dirty.set(true);
            b.render();
        });
        menu.append(&entry);
        self.colour_rows(space, &menu);
        if many {
            let delete = gtk::Button::with_label(&format!(
                "Delete space, its {count} tab{} and its sign-ins",
                if count == 1 { "" } else { "s" }
            ));
            delete.add_css_class("flat");
            delete.add_css_class("menu-item");
            let weak = self.weak();
            delete.connect_clicked(move |button| {
                let Some(b) = weak.upgrade() else { return };
                if let Some(p) = button.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                p.popdown();
            }
                glib::idle_add_local_once(move || b.delete_space(space));
            });
            menu.append(&delete);
        }
        let popover = gtk::Popover::builder().child(&menu).has_arrow(false).build();
        popover.add_css_class("tab-menu");
        crate::motion::enters(&popover);
        popover.set_parent(anchor);
        // Gone with its button when the column is drawn again.
        popover.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        popover.popup();
        entry.grab_focus();
    }
}
