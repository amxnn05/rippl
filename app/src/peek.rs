//! Peek: Shift+click a link and its page
//! opens in a panel over the one you're reading, which stays where it was
//! underneath, dimmed. Esc, a click beside the panel or its × put it away;
//! "Open as a tab" (or Ctrl+Return) keeps it, as a tab after the one it came
//! from, loaded as it is, with nothing loaded twice.
//!
//! The peek's page is made in the same space and kind of session as the tab
//! it came from: same sign-ins, and private from a private tab. `peek = off`
//! turns it off.

use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{gdk, glib, gtk, WebView};

use crate::browser::Browser;

/// The panel's share of the page.
const WIDE: f64 = 0.82;
const TALL: f64 = 0.86;

glib::wrapper! {
    /// Places the panel: 82% by 86% of the page, centred, its buttons beside it.
    pub struct PeekLayout(ObjectSubclass<imp::PeekLayout>) @extends gtk::LayoutManager;
}

mod imp {
    use super::{TALL, WIDE};
    use webkit6::gtk::subclass::prelude::*;
    use webkit6::prelude::*;
    use webkit6::{glib, gtk};

    #[derive(Default)]
    pub struct PeekLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for PeekLayout {
        const NAME: &'static str = "RipplPeekLayout";
        type Type = super::PeekLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for PeekLayout {}

    impl LayoutManagerImpl for PeekLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, _widget: &gtk::Widget, _o: gtk::Orientation, _for: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let w = (f64::from(width) * WIDE) as i32;
            let h = (f64::from(height) * TALL) as i32;
            // it shifts it right by its buttons' half, so the page
            // itself sits in the middle.
            let x = (width - w) / 2 + 19;
            let y = (height - h) / 2;
            let mut child = widget.first_child();
            while let Some(c) = child {
                if c.should_layout() {
                    c.size_allocate(&gtk::Allocation::new(x.min(width - w).max(0), y, w, h), -1);
                }
                child = c.next_sibling();
            }
        }
    }
}

/// A peek on screen: its page, and the tab it was opened from.
pub(crate) struct Peeking {
    pub view: WebView,
    pub from: TabId,
}

impl Browser {
    /// The layer over the page: the dimming, and the panel in it.
    pub(crate) fn peek_layer() -> (gtk::Box, gtk::Box, gtk::Button, gtk::Button) {
        let layer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        layer.add_css_class("peek-dim");
        layer.set_visible(false);
        layer.set_layout_manager(Some(glib::Object::new::<PeekLayout>()));
        let panel = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.add_css_class("peek-panel");
        frame.set_overflow(gtk::Overflow::Hidden);
        frame.set_hexpand(true);
        let knobs = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let knob = |icon: &str, tip: &str| {
            let b = gtk::Button::from_icon_name(icon);
            b.add_css_class("peek-knob");
            b.set_tooltip_text(Some(tip));
            b.set_focusable(false);
            b.set_valign(gtk::Align::Start);
            b
        };
        let close = knob("window-close-symbolic", "Close (Esc)");
        let keep = knob("view-fullscreen-symbolic", "Open as a tab (Ctrl+Return)");
        knobs.append(&close);
        knobs.append(&keep);
        panel.append(&frame);
        panel.append(&knobs);
        layer.append(&panel);
        (layer, frame, close, keep)
    }

    pub(crate) fn wire_peek(&self) {
        let weak = self.weak();
        self.peek_close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.close_peek();
            }
        });
        let weak = self.weak();
        self.peek_keep.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.keep_peek();
            }
        });
        // A click on the dimming, beside the panel, puts it away.
        let click = gtk::GestureClick::new();
        let weak = self.weak();
        click.connect_released(move |gesture, _, x, y| {
            let Some(b) = weak.upgrade() else { return };
            let hit = b.peek_dim.pick(x, y, gtk::PickFlags::DEFAULT);
            if hit.is_some_and(|w| w == *b.peek_dim.upcast_ref::<gtk::Widget>()) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                b.close_peek();
            }
        });
        self.peek_dim.add_controller(click);
        // Esc and Ctrl+Return, before the page sees them.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            if b.peeking.borrow().is_none() {
                return glib::Propagation::Proceed;
            }
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            match key {
                gdk::Key::Escape => b.close_peek(),
                gdk::Key::Return | gdk::Key::KP_Enter if ctrl => b.keep_peek(),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        self.window.add_controller(keys);
    }

    /// Shift+click on a link in `from`'s page.
    pub(crate) fn peek(&self, url: &str, from: TabId) {
        self.put_away_peek(false);
        let (private, space) = match self.tabs.borrow().get(from) {
            Some(t) => (t.private, t.space),
            None => return,
        };
        let view = self.engine.view(private, space, self.shielded_for(url));
        view.set_hexpand(true);
        view.set_vexpand(true);
        view.load_uri(url);
        self.peek_frame.append(&view);
        Browser::arrive(&self.peek_dim);
        view.grab_focus();
        *self.peeking.borrow_mut() = Some(Peeking { view, from });
    }

    /// Put away: the page goes with the panel, which shrinks back as the
    /// dim lifts (motion.rs, `leave`).
    pub(crate) fn close_peek(&self) {
        self.put_away_peek(true);
    }

    fn put_away_peek(&self, animated: bool) {
        let Some(p) = self.peeking.borrow_mut().take() else { return };
        if let Some(v) = self.front_view() {
            v.grab_focus();
        }
        let view = p.view;
        if !animated || !self.moving() {
            self.peek_frame.remove(&view);
            self.peek_dim.set_visible(false);
            view.terminate_web_process();
            return;
        }
        self.leave(&self.peek_dim);
        // The page stays in the panel while it goes, then ends.
        let frame = self.peek_frame.clone();
        glib::timeout_add_local_once(crate::motion::LEAVE, move || {
            if view.parent().is_some() {
                frame.remove(&view);
            }
            view.terminate_web_process();
        });
    }

    /// Kept: a tab after the one it came from, in front, as it is.
    pub(crate) fn keep_peek(&self) {
        let Some(p) = self.peeking.borrow_mut().take() else { return };
        self.peek_frame.remove(&p.view);
        self.peek_dim.set_visible(false);
        let url = p.view.uri().map(|u| u.to_string()).unwrap_or_default();
        let title = p.view.title().map(|t| t.to_string()).filter(|t| !t.trim().is_empty());
        let id = {
            let mut tabs = self.tabs.borrow_mut();
            let (private, space, group) = tabs.get(p.from).map(|t| (t.private, t.space, t.group)).unwrap_or_default();
            let id = tabs.open(&url);
            if let Some(t) = tabs.get_mut(id) {
                t.life = Life::Awake;
                t.private = private;
                t.space = space;
                t.title = title.unwrap_or_else(|| rippl_core::address::pretty(&url));
                if !t.pinned && group.is_some() {
                    t.group = group;
                }
            }
            id
        };
        self.adopt(id, &p.view);
        self.select(id);
        self.dirty.set(true);
    }

    /// Whether a link click is a peek: Shift with the first button.
    pub(crate) fn is_peek_click(&self, action: &webkit6::NavigationAction) -> bool {
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!(
                "rippl: link click: type {:?} button {} modifiers {:#x}",
                action.navigation_type(),
                action.mouse_button(),
                action.modifiers()
            );
        }
        self.config.borrow().peek
            && action.navigation_type() == webkit6::NavigationType::LinkClicked
            && action.mouse_button() == 1
            && action.modifiers() & gdk::ModifierType::SHIFT_MASK.bits() != 0
    }
}
