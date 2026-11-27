//! One site's tabs as one row (Ctrl+Alt+G, or "Group tabs from …" in a
//! tab's menu): every unpinned tab of the front tab's site, in this space,
//! gathered into a single row of tiles, one per tab. A click on a tile
//! brings its tab to front; middle-click closes it; right-click is the
//! tab's own menu. The same shortcut on a grouped tab, or "Ungroup" in the
//! row's menu, lets them go. The rules are `Tabs::group_site` in
//! rippl-core; the group ends by itself when one tab is left.

use rippl_core::address::monogram;
use rippl_core::tabs::{Group, Life, SiteGrouping, Tab, TabId};
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

impl Browser {
    /// Groups the site of `id` (or lets its group go), and says so when
    /// there is nothing to group.
    pub(crate) fn group_site_of(&self, id: TabId) {
        let done = self.tabs.borrow_mut().group_site(id);
        match done {
            SiteGrouping::Alone => self.notice("No other tab from this site to group"),
            SiteGrouping::Grouped(..) | SiteGrouping::Ungrouped(_) => {
                self.dirty.set(true);
                self.render();
            }
        }
    }

    /// The row a site group shows as: a tile per tab, the front one marked.
    pub(crate) fn site_stack(&self, group: &Group, members: &[&Tab], front: Option<TabId>) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.add_css_class("stack");
        let holds_front = members.iter().any(|t| Some(t.id) == front);
        if holds_front {
            row.add_css_class("front");
        }
        // Plain boxes, a line of tiles at a time: a FlowBox or a Button
        // handles the click itself, and the tab never came to front.
        let across = if self.config.borrow().icon_column { 1 } else { 6 };
        let tiles = gtk::Box::new(gtk::Orientation::Vertical, 4);
        tiles.set_valign(gtk::Align::Center);
        for line in members.chunks(across) {
            let strip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            strip.set_valign(gtk::Align::Center);
            for tab in line {
                strip.append(&self.stack_tile(tab, Some(tab.id) == front));
            }
            tiles.append(&strip);
        }
        row.set_child(Some(&tiles));
        let site = group.site.clone().unwrap_or_default();
        row.set_tooltip_text(Some(&format!("{site} · {} tabs (Ctrl+Alt+G ungroups)", members.len())));

        // Right-click on the row itself (not a tile): the group's menu.
        let gid = group.id;
        let click = gtk::GestureClick::builder().button(3).build();
        let weak = self.weak();
        click.connect_released(move |gesture, _, x, y| {
            let Some(b) = weak.upgrade() else { return };
            let Some(w) = gesture.widget() else { return };
            let on_tile = w.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|p| {
                let mut at = Some(p);
                while let Some(x) = at {
                    if x.has_css_class("stack-tile") {
                        return true;
                    }
                    at = x.parent();
                }
                false
            });
            if !on_tile {
                glib::idle_add_local_once(move || b.group_menu(gid, &w));
            }
        });
        row.add_controller(click);
        row
    }

    /// One tab in a site group: its icon (or letter), the loading ring
    /// while it loads, its title as a tooltip.
    fn stack_tile(&self, tab: &Tab, live: bool) -> gtk::CenterBox {
        let tile = gtk::CenterBox::new();
        tile.add_css_class("stack-tile");
        tile.set_cursor_from_name(Some("pointer"));
        // Sized here, not in CSS: every tile the same, the front one wider.
        tile.set_size_request(if live { 38 } else { 30 }, 22);
        tile.set_valign(gtk::Align::Center);
        let face: gtk::Widget = if tab.loading && tab.life == Life::Awake {
            crate::loader::ring(12).upcast()
        } else {
            match self.icon_for(tab.id, &tab.url).filter(|_| !tab.private) {
                Some(icon) => {
                    let image = gtk::Image::from_paintable(Some(&icon));
                    image.set_pixel_size(13);
                    image.upcast()
                }
                None => {
                    let mark = gtk::Label::new(Some(&monogram(&tab.url, &tab.title)));
                    mark.add_css_class("mark");
                    mark.upcast()
                }
            }
        };
        face.set_hexpand(false);
        face.set_halign(gtk::Align::Center);
        face.set_valign(gtk::Align::Center);
        tile.set_center_widget(Some(&face));
        tile.set_tooltip_text(Some(&format!("{}\n{}", tab.title, tab.url)));
        if live {
            tile.add_css_class("front");
        }
        if tab.life != Life::Awake {
            tile.add_css_class("asleep");
        }
        if self.in_split(tab.id) {
            tile.add_css_class("split");
        }
        self.on_click(&tile, tab.id);
        self.mark_fresh(tab.id, tile.upcast_ref());
        if self.closing.borrow().contains(&tab.id) {
            tile.add_css_class("leaving");
        }
        tile
    }
}
