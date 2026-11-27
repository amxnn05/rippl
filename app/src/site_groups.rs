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
        let tiles = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(false)
            .column_spacing(4)
            .row_spacing(4)
            .min_children_per_line(1)
            .max_children_per_line(12)
            .activate_on_single_click(false)
            .build();
        tiles.set_focusable(false);
        for tab in members {
            let tile = self.stack_tile(tab, Some(tab.id) == front);
            tiles.append(&tile);
            if let Some(child) = tile.parent() {
                child.set_focusable(false);
            }
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
                p.has_css_class("stack-tile") || p.ancestor(gtk::Button::static_type()).is_some_and(|a| a.has_css_class("stack-tile"))
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
    fn stack_tile(&self, tab: &Tab, live: bool) -> gtk::Button {
        let tile = gtk::Button::new();
        tile.add_css_class("stack-tile");
        tile.set_focusable(false);
        let face: gtk::Widget = if tab.loading && tab.life == Life::Awake {
            crate::loader::ring(12).upcast()
        } else {
            match self.icon_for(tab.id, &tab.url).filter(|_| !tab.private) {
                Some(icon) => {
                    let image = gtk::Image::from_paintable(Some(&icon));
                    image.set_pixel_size(14);
                    image.upcast()
                }
                None => {
                    let mark = gtk::Label::new(Some(&monogram(&tab.url, &tab.title)));
                    mark.add_css_class("mark");
                    mark.upcast()
                }
            }
        };
        tile.set_child(Some(&face));
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
