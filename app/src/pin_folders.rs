//! Folders among the pins: pinned tabs put
//! into a folder stand as one square, a folder's mark, in the place of its
//! first pin; a click opens it, listing its pins, and a name to change.
//! A tab's menu puts a pin into a new folder or one already there, or takes
//! it out. Each space has its own; unpinning takes a tab out of its folder.

use rippl_core::address::monogram;
use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

impl Browser {
    /// The square standing for folder `name` (its pins `members`).
    pub(crate) fn pin_folder_square(&self, name: &str, members: &[TabId], live: bool) -> gtk::Button {
        let square = gtk::Button::from_icon_name("folder-symbolic");
        square.add_css_class("pin");
        square.add_css_class("pin-folder");
        square.set_focusable(false);
        square.set_tooltip_text(Some(&format!("{name} · {} pinned", members.len())));
        if live {
            square.add_css_class("front");
        }
        let weak = self.weak();
        let name = name.to_string();
        square.connect_clicked(move |button| {
            if let Some(b) = weak.upgrade() {
                b.open_pin_folder(&name, button.upcast_ref());
            }
        });
        square
    }

    /// The folder, open: its name (to change), then its pins.
    pub(crate) fn open_pin_folder(&self, name: &str, anchor: &gtk::Widget) {
        let space = self.tabs.borrow().space;
        let members: Vec<(TabId, String, String, bool, bool)> = {
            let tabs = self.tabs.borrow();
            tabs.here()
                .filter(|t| t.pinned && t.pin_folder.as_deref() == Some(name))
                .map(|t| (t.id, t.url.clone(), t.title.clone(), Some(t.id) == tabs.front, t.life == Life::Awake))
                .collect()
        };
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu.set_margin_top(8);
        menu.set_margin_bottom(8);
        menu.set_margin_start(8);
        menu.set_margin_end(8);
        let entry = gtk::Entry::builder().text(name).placeholder_text("Folder name").build();
        let weak = self.weak();
        let old = name.to_string();
        entry.connect_activate(move |e| {
            let Some(b) = weak.upgrade() else { return };
            b.tabs.borrow_mut().rename_pin_folder(space, &old, &e.text());
            if let Some(p) = e.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                p.popdown();
            }
            b.dirty.set(true);
            b.render();
        });
        menu.append(&entry);
        for (id, url, title, live, awake) in members {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            match self.icon_for(id, &url) {
                Some(icon) => {
                    let image = gtk::Image::from_paintable(Some(&icon));
                    image.set_pixel_size(15);
                    line.append(&image);
                }
                None => {
                    let mark = gtk::Label::new(Some(&monogram(&url, &title)));
                    mark.add_css_class("mark");
                    line.append(&mark);
                }
            }
            let label = gtk::Label::builder()
                .label(&title)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(32)
                .build();
            line.append(&label);
            let row = gtk::Button::builder().child(&line).focusable(false).build();
            row.add_css_class("flat");
            row.add_css_class("menu-item");
            if live {
                row.add_css_class("front");
            }
            if !awake {
                row.add_css_class("asleep");
            }
            row.set_tooltip_text(Some(&url));
            let weak = self.weak();
            row.connect_clicked(move |button| {
                let Some(b) = weak.upgrade() else { return };
                if let Some(p) = button.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                    p.popdown();
                }
                glib::idle_add_local_once(move || b.select(id));
            });
            // Right-click: the tab's own menu (take it out, unpin, …).
            let right = gtk::GestureClick::builder().button(3).build();
            let weak = self.weak();
            right.connect_released(move |g, _, _, _| {
                let Some(b) = weak.upgrade() else { return };
                if let Some(p) = g.widget().and_then(|w| w.ancestor(gtk::Popover::static_type())).and_downcast::<gtk::Popover>() {
                    p.popdown();
                }
                let anchor = b.pins.clone();
                glib::idle_add_local_once(move || b.tab_menu(id, &anchor));
            });
            row.add_controller(right);
            menu.append(&row);
        }
        let popover = gtk::Popover::builder().child(&menu).has_arrow(false).build();
        popover.add_css_class("tab-menu");
        // Hung from the pins' grid, which stays, at the square's place: the
        // square itself goes whenever the column is drawn again.
        popover.set_parent(&self.pins);
        // From the square's left edge: centred under it, it ran off the
        // window at the column's side.
        popover.set_halign(gtk::Align::Start);
        if let Some(b) = anchor.compute_bounds(&self.pins) {
            let rect = gtk::gdk::Rectangle::new(b.x() as i32, b.y() as i32, b.width() as i32, b.height() as i32);
            popover.set_pointing_to(Some(&rect));
        }
        popover.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        popover.popup();
    }

    /// The tab menu's lines for a pin: into a new folder, into one there
    /// is, or out of its own.
    pub(crate) fn pin_folder_items(&self, id: TabId) -> Vec<crate::sitecard::MenuAction> {
        let (space, current) = match self.tabs.borrow().get(id) {
            Some(t) if t.pinned => (t.space, t.pin_folder.clone()),
            _ => return Vec::new(),
        };
        let folders = self.tabs.borrow().pin_folders(space);
        let mut items: Vec<crate::sitecard::MenuAction> = Vec::new();
        let fresh = (1..).map(|n| format!("Folder {n}")).find(|n| !folders.contains(n)).unwrap_or_default();
        items.push((
            "Put in a new folder".into(),
            std::rc::Rc::new(move |b: &Browser, id| {
                b.tabs.borrow_mut().set_pin_folder(id, Some(&fresh));
                b.dirty.set(true);
                b.render();
            }),
        ));
        for f in folders.into_iter().filter(|f| Some(f) != current.as_ref()) {
            let label = format!("Put in “{f}”");
            items.push((
                label,
                std::rc::Rc::new(move |b: &Browser, id| {
                    b.tabs.borrow_mut().set_pin_folder(id, Some(&f));
                    b.dirty.set(true);
                    b.render();
                }),
            ));
        }
        if let Some(f) = current {
            items.push((
                format!("Take out of “{f}”"),
                std::rc::Rc::new(|b: &Browser, id| {
                    b.tabs.borrow_mut().set_pin_folder(id, None);
                    b.dirty.set(true);
                    b.render();
                }),
            ));
        }
        items
    }
}
