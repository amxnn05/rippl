//! The new tab draws it.
//!
//! A tab with nowhere to be yet has no page: the address field stands on
//! its own in the middle of the page card, 560 wide (less in a narrow
//! window), 50 tall, rounded 14, with a hairline and a soft shadow, lifted
//! 30 above centre ("dead centre reads as low"). What it offers while you
//! type hangs under it in a card of its own, so the field never moves.
//! Under the field, with `home_grid = on`, a grid of the sites in the
//! bookmark folder called Home: four to a row, sixteen at most, hidden
//! while you type.
//!
//! It is the same field as the column's, moved: whatever it does there
//! (offers, keywords, Tab to search a site) it does here. With a page in
//! front it goes back to the top of the column. Meanwhile a stand-in of
//! the same look holds its place there, so the column never loses its
//! field; clicking it is clicking the field.
//!
//! it breathes a faint glow under the field; rippl doesn't, as an
//! animation that never stops costs a frame clock on an idle window.

use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

glib::wrapper! {
    /// Places the home box: at most 560 wide, centred, its field lifted.
    /// Ctrl+Shift+Y's bar (grab.rs) is placed by it too.
    pub struct HomeLayout(ObjectSubclass<imp::HomeLayout>) @extends gtk::LayoutManager;
}

mod imp {
    use super::{FIELD_HEIGHT, FIELD_WIDTH, LIFT, SIDE};
    use webkit6::gtk::subclass::prelude::*;
    use webkit6::prelude::*;
    use webkit6::{glib, gtk};

    #[derive(Default)]
    pub struct HomeLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for HomeLayout {
        const NAME: &'static str = "RipplHomeLayout";
        type Type = super::HomeLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for HomeLayout {}

    impl LayoutManagerImpl for HomeLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        /// Asks for little: the page decides the size, not its field.
        fn measure(&self, widget: &gtk::Widget, orientation: gtk::Orientation, _for: i32) -> (i32, i32, i32, i32) {
            let mut least = 0;
            let mut child = widget.first_child();
            while let Some(c) = child {
                least = least.max(c.measure(orientation, -1).0);
                child = c.next_sibling();
            }
            (least.min(120), least.min(120), -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let w = FIELD_WIDTH.min((width - 2 * SIDE).max(0));
            let x = (width - w) / 2;
            let y = (height / 2 - LIFT - FIELD_HEIGHT / 2).max(SIDE);
            let h = (height - y - SIDE).max(0);
            let mut child = widget.first_child();
            while let Some(c) = child {
                if c.should_layout() {
                    c.size_allocate(&gtk::Allocation::new(x, y, w, h), -1);
                }
                child = c.next_sibling();
            }
        }
    }
}

/// the `Metrics.fieldWidth`.
const FIELD_WIDTH: i32 = 560;
/// The field's height, and how far its middle sits above the page's.
const FIELD_HEIGHT: i32 = 50;
const LIFT: i32 = 30;
/// The least room kept on each side in a narrow window (28 in all).
const SIDE: i32 = 14;

impl Browser {
    /// Lays out the blank page, and moves the field into it whenever it is
    /// on screen.
    pub(crate) fn wire_home(&self) {
        // The blank page holds one child, the home box, placed by hand
        // (HomeLayout): GTK has no "at most this wide", and a fixed 560
        // would make the whole window at least that wide.
        let layout: HomeLayout = glib::Object::new();
        self.blank.set_layout_manager(Some(layout));
        self.home.set_parent(&self.blank);

        let weak = self.weak();
        self.stage.connect_visible_child_notify(move |stage| {
            let Some(b) = weak.upgrade() else { return };
            let home = stage.visible_child().is_some_and(|c| c == *b.blank.upcast_ref::<gtk::Widget>());
            b.at_home(home);
        });
        self.at_home(self.stage.visible_child().is_some_and(|c| c == *self.blank.upcast_ref::<gtk::Widget>()));

        // The column's stand-in hands the keyboard to the field at once,
        // so typing there types in it.
        let focus = gtk::EventControllerFocus::new();
        let weak = self.weak();
        focus.connect_enter(move |_| {
            let Some(b) = weak.upgrade() else { return };
            glib::idle_add_local_once(move || {
                b.field.grab_focus();
            });
        });
        self.field_standin.add_controller(focus);
    }

    /// Puts the field (and its offers) in the middle of the blank page, or
    /// back at the top of the column.
    fn at_home(&self, home: bool) {
        if self.home_mode.get() == home && self.field.parent().is_some() {
            if home {
                self.fill_home_grid();
            }
            return;
        }
        self.home_mode.set(home);
        self.show_progress();
        // The column keeps a field at its top either way.
        self.field_standin.set_visible(home && !self.field_on_top());
        let focused = self.field.has_focus();
        self.field.unparent();
        self.suggest_area.unparent();
        if home {
            self.home.prepend(&self.suggest_area);
            self.home.prepend(&self.field);
            self.field.add_css_class("home-field");
            self.field.set_placeholder_text(Some("Enter a web address"));
            self.suggest_area.add_css_class("home-offers");
            self.suggest_area.remove_css_class("top-offers");
            self.suggest_area.set_halign(gtk::Align::Fill);
            self.suggest_area.set_valign(gtk::Align::Fill);
            self.suggest_area.set_size_request(-1, -1);
            self.field.set_size_request(-1, -1);
            self.field.set_hexpand(false);
            self.suggest_area.set_vexpand(false);
            self.suggest_area.set_propagate_natural_height(true);
            self.suggest_area.set_max_content_height(420);
            // The column keeps showing the tabs while you type here.
            self.tabs_area.set_visible(true);
            self.fill_home_grid();
        } else {
            self.put_field_back();
        }
        if focused || home {
            let field = self.field.clone();
            glib::idle_add_local_once(move || {
                field.grab_focus();
            });
        }
    }

    /// Whether the field is standing on the blank page.
    pub(crate) fn field_at_home(&self) -> bool {
        self.home_mode.get()
    }

    /// The Home grid: the sites in the bookmark folder called Home.
    pub(crate) fn fill_home_grid(&self) {
        while let Some(child) = self.home_grid.first_child() {
            self.home_grid.remove(&child);
        }
        let sites: Vec<_> = if self.config.borrow().home_grid {
            self.places
                .bookmarks()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.folder.trim().eq_ignore_ascii_case("home"))
                .take(16)
                .collect()
        } else {
            Vec::new()
        };
        for site in &sites {
            let label = if site.title.trim().is_empty() { rippl_core::address::pretty(&site.url) } else { site.title.clone() };
            let tile = gtk::Button::with_label(&label);
            tile.add_css_class("home-tile");
            tile.set_focusable(false);
            tile.set_tooltip_text(Some(&site.url));
            if let Some(l) = tile.child().and_downcast::<gtk::Label>() {
                l.set_ellipsize(gtk::pango::EllipsizeMode::End);
                l.set_max_width_chars(1);
            }
            let weak = self.weak();
            let url = site.url.clone();
            tile.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.go(&url);
                }
            });
            self.home_grid.insert(&tile, -1);
        }
        self.home_grid.set_visible(!sites.is_empty() && !self.suggest_area.is_visible());
    }
}
