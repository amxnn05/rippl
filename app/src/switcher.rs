//! Ctrl+Tab: the tabs of the space on screen as pictures, the one you are
//! on first, then the ones you last
//! left, latest first. Tab (or →) walks on, Shift+Tab (or ←) back; letting
//! go of Ctrl goes to the one picked, Esc stays where you were, and a click
//! picks one. A quick Ctrl+Tab is the tab you were on before, and never
//! draws the cards. The order stays fixed until Ctrl is let go of.
//!
//! The pictures are taken as you leave a tab, 320 wide (about 300 KB
//! each), and go with the tab; a tab never seen since rippl started shows
//! its site's icon or letter. `tab_switcher = off`: Ctrl+Tab is simply the
//! next tab in the column.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use rippl_core::address::monogram;
use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gdk, gio, glib, gtk, WebView};

use crate::browser::Browser;

/// At most this many cards, four to a row.
const MOST: usize = 12;
const ROW: usize = 4;
/// A picture's width, and its card's.
const PICTURE: f32 = 320.0;
const CARD_W: i32 = 200;
const CARD_H: i32 = 125;
/// A Ctrl+Tab let go of sooner than this never shows the cards.
const QUICK: Duration = Duration::from_millis(140);

glib::wrapper! {
    /// Holds a card's face at exactly its size, whatever its picture's.
    pub struct FaceLayout(ObjectSubclass<imp::FaceLayout>) @extends gtk::LayoutManager;
}

mod imp {
    use super::{CARD_H, CARD_W};
    use webkit6::gtk::subclass::prelude::*;
    use webkit6::prelude::*;
    use webkit6::{glib, gtk};

    #[derive(Default)]
    pub struct FaceLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for FaceLayout {
        const NAME: &'static str = "RipplFaceLayout";
        type Type = super::FaceLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for FaceLayout {}

    impl LayoutManagerImpl for FaceLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, _widget: &gtk::Widget, o: gtk::Orientation, _for: i32) -> (i32, i32, i32, i32) {
            let size = if o == gtk::Orientation::Horizontal { CARD_W } else { CARD_H };
            (size, size, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let mut child = widget.first_child();
            while let Some(c) = child {
                if c.should_layout() {
                    c.size_allocate(&gtk::Allocation::new(0, 0, width, height), -1);
                }
                child = c.next_sibling();
            }
        }
    }
}

pub(crate) struct Switcher {
    pub layer: gtk::Box,
    grid: gtk::Grid,
    /// The walk in progress: its order and where it is.
    walk: RefCell<Option<(Vec<TabId>, usize)>>,
    pictures: RefCell<HashMap<TabId, gdk::Texture>>,
    shown_gen: std::cell::Cell<u32>,
}

impl Switcher {
    pub(crate) fn new() -> Switcher {
        let layer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        layer.add_css_class("switcher-dim");
        layer.set_visible(false);
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("switcher");
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);
        panel.set_vexpand(true);
        let grid = gtk::Grid::builder().row_spacing(14).column_spacing(14).build();
        panel.append(&grid);
        layer.append(&panel);
        Switcher {
            layer,
            grid,
            walk: RefCell::new(None),
            pictures: RefCell::new(HashMap::new()),
            shown_gen: std::cell::Cell::new(0),
        }
    }
}

impl Browser {
    pub(crate) fn wire_switcher(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            // The shortcut recorder is listening: the page gets the keys.
            if b.keys_suspended.get() {
                return glib::Propagation::Proceed;
            }
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
            let walking = b.switcher.walk.borrow().is_some();
            match key {
                gdk::Key::Tab | gdk::Key::ISO_Left_Tab if ctrl => {
                    let back = shift || key == gdk::Key::ISO_Left_Tab;
                    if !b.config.borrow().tab_switcher {
                        if let Some(id) = b.tabs.borrow().neighbour(if back { -1 } else { 1 }) {
                            let w = b.weak();
                            glib::idle_add_local_once(move || w.upgrade().inspect(|b| b.select(id)).map(|_| ()).unwrap_or(()));
                        }
                    } else if walking {
                        b.walk_by(if back { -1 } else { 1 });
                    } else {
                        b.start_walk(back);
                    }
                }
                gdk::Key::Right | gdk::Key::Down if walking => b.walk_by(1),
                gdk::Key::Left | gdk::Key::Up if walking => b.walk_by(-1),
                gdk::Key::Escape if walking => b.end_walk(false),
                gdk::Key::Return | gdk::Key::KP_Enter if walking => b.end_walk(true),
                _ if walking => {}
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        let weak = self.weak();
        keys.connect_key_released(move |_, key, _, _| {
            let Some(b) = weak.upgrade() else { return };
            if matches!(key, gdk::Key::Control_L | gdk::Key::Control_R) && b.switcher.walk.borrow().is_some() {
                b.end_walk(true);
            }
        });
        // Ctrl let go of, as the modifiers say it (some keyboards, and
        // virtual ones, send no key release for it).
        let weak = self.weak();
        keys.connect_modifiers(move |_, state| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            if !state.contains(gdk::ModifierType::CONTROL_MASK) && b.switcher.walk.borrow().is_some() {
                b.end_walk(true);
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(keys);
        // A click beside the cards: stays where you were.
        let click = gtk::GestureClick::new();
        let weak = self.weak();
        click.connect_released(move |_, _, x, y| {
            let Some(b) = weak.upgrade() else { return };
            let hit = b.switcher.layer.pick(x, y, gtk::PickFlags::DEFAULT);
            if hit.is_some_and(|w| w == *b.switcher.layer.upcast_ref::<gtk::Widget>()) {
                b.end_walk(false);
            }
        });
        self.switcher.layer.add_controller(click);
    }

    /// This space's tabs, the one in front first, then the latest left.
    fn walk_order(&self) -> Vec<TabId> {
        let tabs = self.tabs.borrow();
        let mut here: Vec<_> = tabs.here().collect();
        here.sort_by_key(|t| std::cmp::Reverse(t.touched));
        let mut order: Vec<TabId> = here.iter().map(|t| t.id).collect();
        if let Some(f) = tabs.front {
            order.retain(|t| *t != f);
            order.insert(0, f);
        }
        order.truncate(MOST);
        order
    }

    fn start_walk(&self, back: bool) {
        let order = self.walk_order();
        if order.len() < 2 {
            return;
        }
        let at = if back { order.len() - 1 } else { 1 };
        *self.switcher.walk.borrow_mut() = Some((order, at));
        // Drawn only if Ctrl is still held a moment later.
        let turn = self.switcher.shown_gen.get().wrapping_add(1);
        self.switcher.shown_gen.set(turn);
        let weak = self.weak();
        glib::timeout_add_local_once(QUICK, move || {
            let Some(b) = weak.upgrade() else { return };
            if b.switcher.shown_gen.get() == turn && b.switcher.walk.borrow().is_some() {
                b.draw_walk();
                b.switcher.layer.set_visible(true);
            }
        });
    }

    fn walk_by(&self, step: isize) {
        if let Some((order, at)) = self.switcher.walk.borrow_mut().as_mut() {
            let n = order.len() as isize;
            *at = ((*at as isize + step).rem_euclid(n)) as usize;
        }
        if self.switcher.layer.is_visible() {
            self.mark_walk();
        } else {
            // Walking on is no quick flick: show it now.
            self.switcher.shown_gen.set(self.switcher.shown_gen.get().wrapping_add(1));
            self.draw_walk();
            self.switcher.layer.set_visible(true);
        }
    }

    /// Done: to the card picked (`go`), or back where you were.
    pub(crate) fn end_walk(&self, go: bool) {
        let Some((order, at)) = self.switcher.walk.borrow_mut().take() else { return };
        self.switcher.layer.set_visible(false);
        while let Some(c) = self.switcher.grid.first_child() {
            self.switcher.grid.remove(&c);
        }
        if go && let Some(id) = order.get(at).copied().filter(|id| Some(*id) != self.front()) {
            self.select(id);
        }
    }

    fn draw_walk(&self) {
        let grid = &self.switcher.grid;
        while let Some(c) = grid.first_child() {
            grid.remove(&c);
        }
        let Some((order, _)) = self.switcher.walk.borrow().clone() else { return };
        // As many to a row as the page has room for, four at most.
        let room = self.page_area.width() - 2 * 18 - 40;
        let row = ((room / (CARD_W + 12 + 14)).max(1) as usize).min(ROW);
        for (i, id) in order.iter().enumerate() {
            let (url, title) = match self.tabs.borrow().get(*id) {
                Some(t) => (t.url.clone(), t.title.clone()),
                None => continue,
            };
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.add_css_class("switch-card");
            let face = gtk::Box::new(gtk::Orientation::Vertical, 0);
            face.add_css_class("switch-face");
            face.set_layout_manager(Some(glib::Object::new::<FaceLayout>()));
            face.set_overflow(gtk::Overflow::Hidden);
            match self.switcher.pictures.borrow().get(id) {
                Some(picture) => {
                    let p = gtk::Picture::for_paintable(picture);
                    p.set_content_fit(gtk::ContentFit::Cover);
                    p.set_vexpand(true);
                    face.append(&p);
                }
                None => {
                    let centre = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    centre.set_valign(gtk::Align::Center);
                    centre.set_vexpand(true);
                    match self.icon_for(*id, &url) {
                        Some(icon) => {
                            let image = gtk::Image::from_paintable(Some(&icon));
                            image.set_pixel_size(32);
                            centre.append(&image);
                        }
                        None => {
                            let mark = gtk::Label::new(Some(&monogram(&url, &title)));
                            mark.add_css_class("switch-mark");
                            centre.append(&mark);
                        }
                    }
                    face.append(&centre);
                }
            }
            card.append(&face);
            let label = gtk::Label::builder()
                .label(if title.is_empty() { "New tab" } else { &title })
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .width_request(CARD_W)
                .build();
            card.append(&label);
            let click = gtk::GestureClick::new();
            let weak = self.weak();
            click.connect_released(move |g, _, _, _| {
                g.set_state(gtk::EventSequenceState::Claimed);
                let Some(b) = weak.upgrade() else { return };
                if let Some((_, at)) = b.switcher.walk.borrow_mut().as_mut() {
                    *at = i;
                }
                b.end_walk(true);
            });
            card.add_controller(click);
            grid.attach(&card, (i % row) as i32, (i / row) as i32, 1, 1);
        }
        self.mark_walk();
    }

    fn mark_walk(&self) {
        let at = self.switcher.walk.borrow().as_ref().map(|(_, a)| *a);
        let mut i = 0;
        let mut child = self.switcher.grid.first_child();
        while let Some(c) = child {
            if Some(i) == at {
                c.add_css_class("chosen");
            } else {
                c.remove_css_class("chosen");
            }
            i += 1;
            child = c.next_sibling();
        }
    }

    /// A tab is being left: its picture, for the switcher.
    pub(crate) fn keep_picture(&self, id: TabId, view: &WebView) {
        if !self.config.borrow().tab_switcher || view.width() < 50 {
            return;
        }
        let weak = self.weak();
        view.snapshot(
            webkit6::SnapshotRegion::Visible,
            webkit6::SnapshotOptions::NONE,
            None::<&gio::Cancellable>,
            move |shot| {
                let (Some(b), Ok(shot)) = (weak.upgrade(), shot) else { return };
                if let Some(small) = b.shrink(&shot) {
                    b.switcher.pictures.borrow_mut().insert(id, small);
                }
            },
        );
    }

    /// A tab closed: its picture goes too.
    pub(crate) fn drop_picture(&self, id: TabId) {
        self.switcher.pictures.borrow_mut().remove(&id);
    }

    /// `shot` drawn at the picture's size, on the window's renderer: the
    /// top of the page, in the card's shape.
    fn shrink(&self, shot: &gdk::Texture) -> Option<gdk::Texture> {
        use gtk::{graphene, gsk};
        let w = PICTURE.min(shot.width() as f32);
        let full = (w * shot.height() as f32 / shot.width().max(1) as f32).round();
        let h = full.min((w * CARD_H as f32 / CARD_W as f32).round());
        let snap = gtk::Snapshot::new();
        let bounds = graphene::Rect::new(0.0, 0.0, w, h);
        snap.push_clip(&bounds);
        snap.append_scaled_texture(shot, gsk::ScalingFilter::Trilinear, &graphene::Rect::new(0.0, 0.0, w, full));
        snap.pop();
        let node = snap.to_node()?;
        let renderer = self.window.native()?.renderer()?;
        Some(renderer.render_texture(node, Some(&bounds)))
    }

    /// For the control socket: Ctrl+Tab, or Tab again with Ctrl held.
    pub(crate) fn walk_next(&self) {
        if self.switcher.walk.borrow().is_some() {
            self.walk_by(1);
        } else {
            self.start_walk(false);
        }
    }

    /// For the control socket: whether the cards are up, the order, the
    /// pick, and which have pictures.
    pub(crate) fn walk_state(&self) -> String {
        let pictures = self.switcher.pictures.borrow();
        match self.switcher.walk.borrow().as_ref() {
            Some((order, at)) => format!(
                "shown={} at={at} order={}",
                self.switcher.layer.is_visible(),
                order.iter().map(|t| format!("{t}{}", if pictures.contains_key(t) { "*" } else { "" })).collect::<Vec<_>>().join(",")
            ),
            None => format!("off pictures={}", pictures.len()),
        }
    }
}
