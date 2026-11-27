//! Where the tabs and the field are: down the side (the column, as ever),
//! the column as icons only,
//! or tabs across the top.
//!
//! - **Icons only**: the column 52 wide, each tab its site's icon or letter
//!   in a small tile, its title a tooltip, nothing that grows or slides out
//!   under the pointer. The pins stack the same way. The field goes to a
//!   strip above the page, where what it offers hangs under it.
//! - **Tabs on top**: no column. The strip above the page holds the field,
//!   then the pins as icons, then the tabs, each its icon and title, the
//!   one you are on in the grey pill; a + and the settings at the end. The
//!   tabs share the row, down to a least width, and the row scrolls past
//!   that. Spaces stay on Alt+1–9.
//!
//! The new tab keeps its field in the middle of the page either way.

use rippl_core::address::monogram;
use rippl_core::tabs::Life;
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

/// The narrow column's width, and the side column's.
const ICON_COLUMN: i32 = 52;
const SIDE_COLUMN: i32 = 232;
/// The field's width in the strip, beside top tabs.
const TOP_FIELD: i32 = 300;
/// A top tab's widest and least width: past the least, the row scrolls.
const TAB_MOST: i32 = 220;
const TAB_LEAST: i32 = 96;

/// The strip above the page: the field's slot, the tabs, a + and the gear.
pub(crate) struct TopStrip {
    pub bar: gtk::Box,
    pub field_slot: gtk::Box,
    pub tabs_scroll: gtk::ScrolledWindow,
    pub tabs: gtk::Box,
    pub plus: gtk::Button,
    pub gear: gtk::Button,
}

impl TopStrip {
    pub(crate) fn new() -> TopStrip {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bar.add_css_class("top-strip");
        bar.set_visible(false);
        let field_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        tabs.add_css_class("top-tabs");
        let tabs_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&tabs)
            .build();
        let plus = gtk::Button::from_icon_name("list-add-symbolic");
        plus.set_tooltip_text(Some("New tab (Ctrl+T)"));
        plus.set_focusable(false);
        plus.add_css_class("top-knob");
        let gear = gtk::Button::from_icon_name("emblem-system-symbolic");
        gear.set_tooltip_text(Some("Settings (Ctrl+,)"));
        gear.set_focusable(false);
        gear.add_css_class("top-knob");
        bar.append(&field_slot);
        bar.append(&tabs_scroll);
        bar.append(&plus);
        bar.append(&gear);
        TopStrip { bar, field_slot, tabs_scroll, tabs, plus, gear }
    }
}

impl Browser {
    pub(crate) fn wire_chrome(&self) {
        let weak = self.weak();
        self.top.plus.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.open("", false, true);
            }
        });
        let weak = self.weak();
        self.top.gear.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.open_internal("settings");
            }
        });
        // A turn of the wheel walks the row of top tabs sideways.
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        let scroll = self.top.tabs_scroll.clone();
        wheel.connect_scroll(move |_, _, dy| {
            let h = scroll.hadjustment();
            h.set_value(h.value() + dy * 40.0);
            glib::Propagation::Stop
        });
        self.top.tabs_scroll.add_controller(wheel);
        // The window's width changed: the tabs share the new room.
        let weak = self.weak();
        self.top.tabs_scroll.hadjustment().connect_page_size_notify(move |_| {
            if let Some(b) = weak.upgrade() {
                b.size_top_tabs();
            }
        });
        self.apply_chrome();
    }

    /// Whether the field lives in the strip above the page.
    pub(crate) fn field_on_top(&self) -> bool {
        let c = self.config.borrow();
        c.top_tabs || c.icon_column
    }

    /// Lays the window out as the settings say.
    pub(crate) fn apply_chrome(&self) {
        let (top, icons) = {
            let c = self.config.borrow();
            (c.top_tabs, c.icon_column && !c.top_tabs)
        };
        self.column.set_visible(!top);
        if top {
            self.window.add_css_class("no-column");
        } else {
            self.window.remove_css_class("no-column");
        }
        // The new tab row: its + alone in the narrow column.
        if let Some(label) = self.new_tab.child().and_then(|c| c.last_child()) {
            label.set_visible(!icons);
        }
        self.column.set_size_request(if icons { ICON_COLUMN } else { SIDE_COLUMN }, -1);
        if icons {
            self.column.add_css_class("icons");
        } else {
            self.column.remove_css_class("icons");
        }
        // The foot's settings and spaces stack in the narrow column.
        if let Some(foot) = self.spaces_box.parent().and_downcast::<gtk::Box>() {
            foot.set_orientation(if icons { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal });
        }
        self.spaces_box.set_orientation(if icons { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal });
        self.player_row.set_visible(false);
        self.top.bar.set_visible(top || icons);
        self.top.tabs_scroll.set_visible(top);
        self.top.plus.set_visible(top);
        self.top.gear.set_visible(top);
        self.top.field_slot.set_hexpand(!top);
        // The tab menu hangs from wherever the tabs are.
        let menu_home: gtk::Widget = if top { self.top.bar.clone().upcast() } else { self.tabs_area.clone().upcast() };
        if self.tab_menu.parent().as_ref() != Some(&menu_home) {
            self.tab_menu.unparent();
            self.tab_menu.set_parent(&menu_home);
        }
        if !self.field_at_home() {
            self.put_field_back();
        }
        self.render();
    }

    /// The field (and what it offers) back from the new tab's middle: to
    /// the top of the column, or into the strip above the page.
    pub(crate) fn put_field_back(&self) {
        self.field.unparent();
        self.suggest_area.unparent();
        self.field.remove_css_class("home-field");
        self.field.set_placeholder_text(Some("Search or enter address"));
        self.suggest_area.set_propagate_natural_height(false);
        self.suggest_area.set_max_content_height(-1);
        if self.field_on_top() {
            let top = self.config.borrow().top_tabs;
            self.field.set_size_request(if top { TOP_FIELD } else { -1 }, -1);
            self.field.set_hexpand(!top);
            self.top.field_slot.append(&self.field);
            // What it offers floats over the page, under the field.
            self.suggest_area.add_css_class("home-offers");
            self.suggest_area.add_css_class("top-offers");
            self.suggest_area.set_halign(gtk::Align::Start);
            self.suggest_area.set_valign(gtk::Align::Start);
            self.suggest_area.set_vexpand(false);
            self.suggest_area.set_propagate_natural_height(true);
            self.suggest_area.set_max_content_height(420);
            self.suggest_area.set_size_request(520, -1);
            self.page_area.add_overlay(&self.suggest_area);
            self.tabs_area.set_visible(true);
        } else {
            self.field.set_size_request(-1, -1);
            self.field.set_hexpand(false);
            self.suggest_area.remove_css_class("home-offers");
            self.suggest_area.remove_css_class("top-offers");
            self.suggest_area.set_halign(gtk::Align::Fill);
            self.suggest_area.set_valign(gtk::Align::Fill);
            self.suggest_area.set_size_request(-1, -1);
            self.suggest_area.set_vexpand(true);
            self.column.prepend(&self.suggest_area);
            self.column.prepend(&self.field);
            self.tabs_area.set_visible(!self.suggest_area.is_visible());
        }
    }

    /// Draws the row of top tabs again (from `render`).
    pub(crate) fn render_top(&self) {
        self.row_of.borrow_mut().clear();
        let strip = &self.top.tabs;
        while let Some(c) = strip.first_child() {
            strip.remove(&c);
        }
        let tabs = self.tabs.borrow();
        let mut live_tab: Option<gtk::Widget> = None;
        for tab in tabs.here() {
            let id = tab.id;
            let live = Some(id) == tabs.front;
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let mark: gtk::Widget = match self.icon_for(id, &tab.url).filter(|_| !tab.private && !tab.url.is_empty()) {
                _ if tab.pinned && tab.loading && tab.life == Life::Awake && !tab.url.is_empty() => crate::loader::ring(15).upcast(),
                Some(icon) => {
                    let image = gtk::Image::from_paintable(Some(&icon));
                    image.set_pixel_size(15);
                    image.upcast()
                }
                None => {
                    let text = if tab.private { "◐".to_string() } else { monogram(&tab.url, &tab.title) };
                    let l = gtk::Label::new(Some(&text));
                    l.add_css_class("mark");
                    l.upcast()
                }
            };
            mark.set_valign(gtk::Align::Center);
            line.append(&mark);
            let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            tile.add_css_class("top-tab");
            tile.set_tooltip_text(Some(&format!("{}\n{}", tab.title, tab.url)));
            if tab.pinned {
                tile.add_css_class("pinned");
            } else {
                let label = gtk::Label::builder()
                    .label(if tab.playing_audio { format!("{} ♪", tab.title) } else { tab.title.clone() })
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(1)
                    .build();
                line.append(&label);
                let close = gtk::Button::from_icon_name("window-close-symbolic");
                close.add_css_class("flat");
                close.add_css_class("close");
                close.set_tooltip_text(Some("Close tab"));
                close.set_focusable(false);
                let weak = self.weak();
                close.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.close_animated(id);
                    }
                });
                // Loading: the ring where the × is, the × under the pointer.
                let slot = gtk::Overlay::builder().child(&close).build();
                if tab.loading && tab.life == Life::Awake {
                    let ring = crate::loader::ring(13);
                    ring.set_can_target(false);
                    slot.add_overlay(&ring);
                    tile.add_css_class("loading");
                }
                line.append(&slot);
            }
            tile.append(&line);
            if live {
                tile.add_css_class("front");
                live_tab = Some(tile.clone().upcast());
            }
            if tab.life != Life::Awake && !tab.url.is_empty() {
                tile.add_css_class("asleep");
            }
            if self.in_split(id) {
                tile.add_css_class("split");
            }
            self.on_click(&tile, id);
            self.mark_fresh(id, tile.upcast_ref());
            strip.append(&tile);
        }
        drop(tabs);
        self.settle_fresh();
        self.size_top_tabs();
        // The tab you are on, in sight.
        if let Some(live) = live_tab {
            let scroll = self.top.tabs_scroll.clone();
            let row = self.top.tabs.clone();
            // After the row is laid out: its tabs' places, in the row's own
            // terms (the viewport's are already scrolled).
            glib::timeout_add_local_once(std::time::Duration::from_millis(30), move || {
                if let Some(b) = live.compute_bounds(&row) {
                    let h = scroll.hadjustment();
                    let (x, w) = (f64::from(b.x()), f64::from(b.width()));
                    if x < h.value() {
                        h.set_value(x);
                    } else if x + w > h.value() + h.page_size() {
                        h.set_value(x + w - h.page_size());
                    }
                }
            });
        }
    }

    /// The top tabs share the row: as wide as there is room for, between
    /// their least and widest; pins keep their own small width.
    fn size_top_tabs(&self) {
        let room = self.top.tabs_scroll.width();
        if room <= 0 {
            return;
        }
        let mut tiles = Vec::new();
        let mut pinned = 0;
        let mut child = self.top.tabs.first_child();
        while let Some(c) = child {
            if c.has_css_class("pinned") {
                pinned += c.measure(gtk::Orientation::Horizontal, -1).1 + 2;
            } else {
                tiles.push(c.clone());
            }
            child = c.next_sibling();
        }
        if tiles.is_empty() {
            return;
        }
        let each = ((room - pinned - 4) / tiles.len() as i32 - 2).clamp(TAB_LEAST, TAB_MOST);
        for t in tiles {
            if t.width_request() != each {
                t.set_size_request(each, -1);
            }
        }
    }
}
