//! The bookmarks bar: the
//! top of the bookmarks in a thin row above the page, on the window's
//! ground. A folder opens as a menu. What the row can't hold goes into a »
//! at its end, a menu of the rest, rather than a row that scrolls sideways
//! with nothing to show it does.
//!
//! A click opens the bookmark in the tab in front; a middle click in a new
//! tab behind it. Off unless asked for: the page gives up 30 pixels to it.

use rippl_core::places::Place;
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

/// The bar's height, and the room between two on it.
const HEIGHT: i32 = 30;
const GAP: i32 = 2;

glib::wrapper! {
    /// Lays the bar out: as many from the first as fit, then the », which
    /// is its last child and shows only when some don't.
    pub struct BarLayout(ObjectSubclass<imp::BarLayout>) @extends gtk::LayoutManager;
}

mod imp {
    use super::{GAP, HEIGHT};
    use webkit6::gtk::subclass::prelude::*;
    use webkit6::prelude::*;
    use webkit6::{glib, gtk};

    #[derive(Default)]
    pub struct BarLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for BarLayout {
        const NAME: &'static str = "RipplBarLayout";
        type Type = super::BarLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for BarLayout {}

    impl LayoutManagerImpl for BarLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        /// Asks for no width (the window decides it) and its own height.
        fn measure(&self, _widget: &gtk::Widget, o: gtk::Orientation, _for: i32) -> (i32, i32, i32, i32) {
            if o == gtk::Orientation::Horizontal { (0, 0, -1, -1) } else { (HEIGHT, HEIGHT, -1, -1) }
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let mut kids = Vec::new();
            let mut child = widget.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                kids.push(c);
            }
            let Some(more) = kids.pop() else { return };
            let widths: Vec<i32> = kids.iter().map(|k| k.measure(gtk::Orientation::Horizontal, -1).1).collect();
            let all: i32 = widths.iter().sum::<i32>() + GAP * widths.len() as i32;
            let more_w = more.measure(gtk::Orientation::Horizontal, -1).1;
            let room = if all <= width { width } else { width - more_w - GAP };
            let mut x = 0;
            let mut cut = false;
            for (k, w) in kids.iter().zip(widths) {
                let h = k.measure(gtk::Orientation::Vertical, w).1.min(height);
                if cut || x + w > room {
                    cut = true;
                    k.set_child_visible(false);
                    continue;
                }
                k.set_child_visible(true);
                k.size_allocate(&gtk::Allocation::new(x, (height - h) / 2, w, h), -1);
                x += w + GAP;
            }
            more.set_child_visible(cut);
            if cut {
                let h = more.measure(gtk::Orientation::Vertical, more_w).1.min(height);
                more.size_allocate(&gtk::Allocation::new(width - more_w, (height - h) / 2, more_w, h), -1);
            }
        }
    }
}

/// One thing on the bar: a bookmark, or a folder of them.
enum Item {
    Site(Place),
    Folder(String, Vec<Place>),
}

/// The bar's things, in the bookmarks' order: loose bookmarks as they are,
/// and each top folder once, where its first bookmark is, holding every
/// bookmark under it.
fn items(all: Vec<Place>) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for p in all {
        let top = p.folder.split('/').next().unwrap_or("").trim().to_string();
        if top.is_empty() {
            out.push(Item::Site(p));
        } else if let Some(Item::Folder(_, list)) = out.iter_mut().find(|i| matches!(i, Item::Folder(n, _) if *n == top)) {
            list.push(p);
        } else {
            out.push(Item::Folder(top, vec![p]));
        }
    }
    out
}

fn label_of(p: &Place) -> String {
    if p.title.trim().is_empty() { rippl_core::address::pretty(&p.url) } else { p.title.clone() }
}

impl Browser {
    /// The bar, empty and hidden; `fill_bookmarks_bar` fills it.
    pub(crate) fn bookmarks_bar() -> gtk::Box {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("bookmarks-bar");
        bar.set_layout_manager(Some(glib::Object::new::<BarLayout>()));
        bar.set_visible(false);
        bar
    }

    /// Shows the bar as the bookmarks are now, or hides it.
    pub(crate) fn fill_bookmarks_bar(&self) {
        let bar = &self.bookmarks_bar;
        while let Some(c) = bar.first_child() {
            c.unparent();
        }
        let on = self.config.borrow().bookmarks_bar;
        let things = if on { items(self.places.bookmarks().unwrap_or_default()) } else { Vec::new() };
        bar.set_visible(on);
        if on {
            self.stage.add_css_class("under-bar");
        } else {
            self.stage.remove_css_class("under-bar");
            return;
        }
        if things.is_empty() {
            let hint = gtk::Label::new(Some("Bookmarks show here: Ctrl+D, or the star in the field"));
            hint.add_css_class("bar-hint");
            hint.set_parent(bar);
        }
        for thing in &things {
            let widget: gtk::Widget = match thing {
                Item::Site(p) => self.bar_site(p).upcast(),
                Item::Folder(name, list) => self.bar_folder(name, list).upcast(),
            };
            widget.set_parent(bar);
        }
        // The », listing what the row doesn't hold at the time it is opened.
        // A label of its own: given as the button's label, GTK adds an arrow.
        let more = gtk::MenuButton::builder()
            .child(&gtk::Label::new(Some("»")))
            .has_frame(false)
            .focusable(false)
            .tooltip_text("More bookmarks")
            .build();
        more.add_css_class("bar-item");
        let pop = gtk::Popover::new();
        pop.add_css_class("bar-menu");
        more.set_popover(Some(&pop));
        let weak = self.weak();
        pop.connect_show(move |pop| {
            let Some(b) = weak.upgrade() else { return };
            let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let mut things = items(b.places.bookmarks().unwrap_or_default()).into_iter();
            let mut child = b.bookmarks_bar.first_child();
            while let Some(c) = child {
                child = c.next_sibling();
                // The last child is the » itself.
                let Some(thing) = things.next().filter(|_| child.is_some()) else { break };
                if c.is_child_visible() {
                    continue;
                }
                match thing {
                    Item::Site(p) => list.append(&b.menu_site(&p, pop)),
                    Item::Folder(name, places) => {
                        let head = gtk::Label::new(Some(&name));
                        head.add_css_class("bar-folder-head");
                        head.set_xalign(0.0);
                        list.append(&head);
                        for p in &places {
                            list.append(&b.menu_site(p, pop));
                        }
                    }
                }
            }
            pop.set_child(Some(&list));
        });
        more.set_parent(bar);
    }

    /// For the control socket: what the bar shows, and what the » holds;
    /// `more` opens the ».
    pub(crate) fn bar_state(&self, open_more: bool) -> String {
        let mut shown = Vec::new();
        let mut hidden = 0;
        let mut child = self.bookmarks_bar.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if child.is_none() {
                if open_more && let Some(m) = c.downcast_ref::<gtk::MenuButton>() {
                    m.popup();
                }
                break;
            }
            if c.is_child_visible() {
                shown.push(c.tooltip_text().map(|t| t.to_string()).unwrap_or_else(|| "folder".into()));
            } else {
                hidden += 1;
            }
        }
        format!("visible={} shown={} hidden={hidden}", self.bookmarks_bar.is_visible(), shown.join(","))
    }

    fn bar_site(&self, p: &Place) -> gtk::Button {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if let Some(icon) = self.icon_for_url(&p.url) {
            let image = gtk::Image::from_paintable(Some(&icon));
            image.set_pixel_size(14);
            line.append(&image);
        }
        let label = gtk::Label::builder()
            .label(label_of(p))
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(22)
            .build();
        line.append(&label);
        let button = gtk::Button::builder().child(&line).has_frame(false).focusable(false).tooltip_text(&p.url).build();
        button.add_css_class("bar-item");
        let url = p.url.clone();
        let weak = self.weak();
        button.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.go(&url);
            }
        });
        // The middle button: a new tab behind this one.
        let middle = gtk::GestureClick::builder().button(2).build();
        let url = p.url.clone();
        let weak = self.weak();
        middle.connect_released(move |_, _, _, _| {
            if let Some(b) = weak.upgrade() {
                b.open(&url, false, false);
            }
        });
        button.add_controller(middle);
        button
    }

    fn bar_folder(&self, name: &str, places: &[Place]) -> gtk::MenuButton {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        line.append(&gtk::Image::from_icon_name("folder-symbolic"));
        line.append(&gtk::Label::new(Some(name)));
        let button = gtk::MenuButton::builder().child(&line).has_frame(false).focusable(false).build();
        button.add_css_class("bar-item");
        let pop = gtk::Popover::new();
        pop.add_css_class("bar-menu");
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let mut last_sub = String::new();
        for p in places {
            // Folders inside it: a quiet heading before their bookmarks.
            let sub = p.folder.split_once('/').map(|(_, s)| s.to_string()).unwrap_or_default();
            if sub != last_sub && !sub.is_empty() {
                let head = gtk::Label::new(Some(&sub));
                head.add_css_class("bar-folder-head");
                head.set_xalign(0.0);
                list.append(&head);
            }
            last_sub = sub;
            list.append(&self.menu_site(p, &pop));
        }
        pop.set_child(Some(&list));
        button.set_popover(Some(&pop));
        button
    }

    /// A bookmark as a line in a folder's menu, or in the »'s.
    fn menu_site(&self, p: &Place, pop: &gtk::Popover) -> gtk::Button {
        let button = self.bar_site(p);
        button.set_tooltip_text(Some(&p.url));
        if let Some(label) = button.child().and_then(|c| c.last_child()).and_downcast::<gtk::Label>() {
            label.set_max_width_chars(40);
            label.set_xalign(0.0);
        }
        let pop = pop.downgrade();
        button.connect_clicked(move |_| {
            if let Some(p) = pop.upgrade() {
                p.popdown();
            }
        });
        button
    }
}
