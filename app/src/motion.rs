//! Motion: what moves, and how (Emil Kowalski's rules and Apple's fluid
//! interfaces). One strong **ease-out** for everything that enters or
//! leaves: it answers at once and settles without overshoot, as a
//! critically damped spring does; bounce is kept for what was flung (the
//! page's elastic edges). Popovers grow from their trigger. Hovers and
//! colours use plain `ease`. A press gives to 97% at once and lets go
//! gently.
//!
//! What a key opens many times a day moves least: a new tab's field has no
//! entrance, Ctrl+Tab fades in 0.12 s and its cards change at once, Ctrl+F
//! drops in 0.18 s. Exits are never ease-in: they start at once.
//!
//! - Popovers and menus pop in; the find bar, a page's question and the
//!   save-password bar drop in; a notice rises, and fades as it goes.
//! - Peek scales up over a fading dim; the Ctrl+Tab cards fade in; the AI
//!   panel slides in from the right.
//! - A new tab slides into the column; one closed with the mouse fades and
//!   folds away. Going from tab to tab cross-fades (0.12 s); Ctrl+S slides
//!   the column away and back.
//! - Pages stretch a little at their top and bottom and spring back
//!   (elastic edges, pagescripts.rs).
//!
//! Peek and the AI panel leave the way they came (`leave`).
//!
//! `motion = reduced`, or the desktop's own "reduce animation"
//! (`gtk-enable-animations` off), makes all of it immediate, GTK's own
//! animations too.
//! Nothing here runs while nothing moves: CSS animations and transitions
//! cost frames only while they play.

use std::time::Duration;

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

/// A row folding away before its tab closes, and a notice fading.
pub(crate) const LEAVE: Duration = Duration::from_millis(150);

pub(crate) const CSS: &str = r#"
/* One curve for what enters and leaves: a strong ease-out, which starts
   at once and settles without overshoot (Apple's critically damped spring,
   damping 1). Nothing here was flung, so nothing bounces. */
window.rippl, popover {
  --ease-out: cubic-bezier(0.23, 1, 0.32, 1);
}

@keyframes rippl-pop { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: none; } }
@keyframes rippl-fade { from { opacity: 0; } to { opacity: 1; } }
@keyframes rippl-rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@keyframes rippl-sink { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateY(6px); } }
@keyframes rippl-drop { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: none; } }
@keyframes rippl-in-left { from { opacity: 0; transform: translateX(-10px); } to { opacity: 1; transform: none; } }
@keyframes rippl-in-right { from { opacity: 0; transform: translateX(24px); } to { opacity: 1; transform: none; } }
@keyframes rippl-fade-out { from { opacity: 1; } to { opacity: 0; } }
@keyframes rippl-shrink { from { opacity: 1; transform: none; } to { opacity: 0; transform: scale(0.96); } }
@keyframes rippl-out-right { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateX(24px); } }
@keyframes rippl-out-left { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateX(-12px) scale(0.98); } }

/* What comes and goes carries .enter while it is shown (motion.rs,
   `enters`): GTK starts an animation when a style changes, not when a
   widget shows again. */

/* Popovers (the site card, menus, accounts, folders) grow out of what
   opened them: they hang below it, so from their top. */
popover.enter > contents { animation: rippl-pop 220ms var(--ease-out); transform-origin: top; }

/* Bars from a key (Ctrl+F, a page's question): in at once, no bounce.
   Notices rise; on their way out they answer at once too (ease-out). */
window.rippl .find.enter, window.rippl .ask.enter { animation: rippl-drop 180ms var(--ease-out); }
window.rippl .toast.enter { animation: rippl-rise 260ms var(--ease-out); }
window.rippl .toast.enter.leaving { animation: rippl-sink 150ms var(--ease-out) forwards; }
window.rippl .status.enter { animation: rippl-fade 120ms var(--ease-out); }

/* Peek is rare and spatial: the dim fades, the panel glides up.
   The switcher (Ctrl+Tab) and the video bar come from keys: a quick fade,
   and the cards change at once while Tab is held. */
window.rippl .peek-dim.enter { animation: rippl-fade 200ms var(--ease-out); }
window.rippl .peek-dim.enter .peek-panel { animation: rippl-pop 320ms var(--ease-out); }
/* ...and goes back the way it came: the panel shrinks as the dim lifts. */
window.rippl .peek-dim.enter.leaving { animation: rippl-fade-out 160ms var(--ease-out) forwards; }
window.rippl .peek-dim.enter.leaving .peek-panel { animation: rippl-shrink 160ms var(--ease-out) forwards; }
window.rippl .switcher-dim.enter { animation: rippl-fade 120ms var(--ease-out); }
window.rippl .grab-dim.enter { animation: rippl-fade 160ms var(--ease-out); }
window.rippl .grab-dim.enter .grab-box { animation: rippl-pop 180ms var(--ease-out); }
window.rippl .switch-card { transition: background-color 80ms var(--ease-out); }

/* The AI panel slides in. A new tab (Ctrl+T, many times a day) shows its
   field where it is, without an entrance; the offers under it fade. */
window.rippl .ai-panel.enter { animation: rippl-in-right 280ms var(--ease-out); }
window.rippl .ai-panel.enter.leaving { animation: rippl-out-right 160ms var(--ease-out) forwards; }

/* A theme change (light, dark) eases instead of flashing. */
window.rippl { transition: background-color 240ms ease, color 240ms ease; }
window.rippl .stage { transition: background-color 240ms ease, box-shadow 240ms ease; }
window.rippl .home-offers { animation: rippl-fade 120ms var(--ease-out); }

/* Tabs: a new one slides in; one closed with the mouse folds away. */
window.rippl .tabs row.fresh, window.rippl .top-tab.fresh { animation: rippl-in-left 200ms var(--ease-out); }
window.rippl .tabs row.leaving, window.rippl .top-tab.leaving { animation: rippl-out-left 150ms var(--ease-out) forwards; }
window.rippl .pin.fresh, window.rippl .stack-tile.fresh { animation: rippl-pop 200ms var(--ease-out); }
window.rippl .stack-tile.leaving { animation: rippl-sink 150ms var(--ease-out) forwards; }

/* Hover and choice: colours ease. A press answers at once (100 ms down)
   and lets go gently (160 ms back), without overshoot. */
window.rippl .tabs row, window.rippl .pin, window.rippl .new-tab, window.rippl .top-tab, window.rippl .rail-row,
window.rippl .rail-back, window.rippl .bar-item, window.rippl .foot button, window.rippl .top-knob,
window.rippl .menu-item, window.rippl .space, window.rippl .swatch, window.rippl .player-button, window.rippl .home-tile,
window.rippl .stack-tile, window.rippl button {
  transition: background-color 140ms ease, color 140ms ease, box-shadow 200ms ease, opacity 140ms ease,
    transform 160ms var(--ease-out);
}
window.rippl .pin:active, window.rippl .tabs row:active, window.rippl .top-tab:active, window.rippl .rail-row:active,
window.rippl .new-tab:active, window.rippl .bar-item:active, window.rippl .home-tile:active, window.rippl .switch-card:active,
window.rippl .stack-tile:active, window.rippl button:active { transform: scale(0.97);
  transition: background-color 140ms ease, color 140ms ease, box-shadow 200ms ease, opacity 140ms ease,
    transform 100ms var(--ease-out); }
window.rippl .field { transition: background-color 160ms ease, box-shadow 220ms ease; }
window.rippl .rail-row.here, window.rippl .tabs row.front { transition: background-color 160ms ease, box-shadow 200ms ease; }
"#;

/// Whether the desktop allows animation (GNOME's "Reduce animation",
/// `gtk-enable-animations`), as it was before rippl set its own: read once.
pub(crate) fn desktop_animates() -> bool {
    thread_local!(static DESKTOP: std::cell::OnceCell<bool> = const { std::cell::OnceCell::new() });
    DESKTOP.with(|d| *d.get_or_init(|| gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())))
}

/// `w` carries the class `enter` while it is on screen, so its animation
/// (above) plays each time it shows, not only the first.
pub(crate) fn enters(w: &impl IsA<gtk::Widget>) {
    w.connect_map(|w| w.add_css_class("enter"));
    w.connect_unmap(|w| w.remove_css_class("enter"));
}

impl Browser {
    /// Each overlay, bar and popover that comes and goes.
    pub(crate) fn wire_motion(&self) {
        enters(&self.peek_dim);
        enters(&self.switcher.layer);
        enters(&self.find_bar);
        enters(&self.grab_bar);
        enters(&self.ask_bar);
        enters(&self.save_bar);
        enters(&self.popup_bar);
        enters(&self.toast);
        enters(&self.status);
        enters(&self.ai.panel);
        enters(&self.card);
        enters(&self.tab_menu);
        enters(&self.accounts_popover);
    }

    /// Whether anything moves: `motion = full`, unless the desktop asks
    /// for less motion.
    pub(crate) fn moving(&self) -> bool {
        desktop_animates() && self.config.borrow().motion_full
    }

    /// Full motion, or none (`motion = reduced`): GTK's own animations
    /// with it, and the page cross-fade.
    pub(crate) fn apply_motion(&self) {
        let full = self.moving();
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_enable_animations(full);
        }
        self.stage.set_transition_type(if full { gtk::StackTransitionType::Crossfade } else { gtk::StackTransitionType::None });
        self.stage.set_transition_duration(if full { 120 } else { 0 });
        self.column_slide.set_transition_duration(if full { 240 } else { 0 });
    }

    /// Closed with the mouse (its ×, a middle click): the row folds away,
    /// then the tab goes. The keyboard's Ctrl+W stays immediate.
    pub(crate) fn close_animated(&self, id: TabId) {
        if !self.moving() {
            self.close(id);
            return;
        }
        // Already folding away: one close is enough.
        if !self.closing.borrow_mut().insert(id) {
            return;
        }
        if let Some(row) = self.row_of.borrow().get(&id) {
            row.add_css_class("leaving");
        }
        let weak = self.weak();
        glib::timeout_add_local_once(LEAVE, move || {
            if let Some(b) = weak.upgrade() {
                b.closing.borrow_mut().remove(&id);
                b.close(id);
            }
        });
    }

    /// A notice's leaving: it sinks and fades, then hides.
    pub(crate) fn toast_leave(&self) {
        if !self.moving() {
            self.toast.set_visible(false);
            return;
        }
        self.toast.add_css_class("leaving");
        let toast = self.toast.clone();
        glib::timeout_add_local_once(LEAVE, move || {
            if toast.has_css_class("leaving") {
                toast.set_visible(false);
                toast.remove_css_class("leaving");
            }
        });
    }

    /// `w` goes the way it came (its `.leaving` animation), then hides;
    /// shown again meanwhile (`arrive`), it stays.
    pub(crate) fn leave(&self, w: &impl IsA<gtk::Widget>) {
        let w = w.as_ref().clone();
        if !self.moving() || !w.is_visible() {
            w.set_visible(false);
            return;
        }
        w.add_css_class("leaving");
        glib::timeout_add_local_once(LEAVE, move || {
            if w.has_css_class("leaving") {
                w.set_visible(false);
                w.remove_css_class("leaving");
            }
        });
    }

    /// `w` shown, even if it was on its way out.
    pub(crate) fn arrive(w: &impl IsA<gtk::Widget>) {
        w.remove_css_class("leaving");
        w.set_visible(true);
    }

    /// Ctrl+S: the column slides away, or back.
    pub(crate) fn toggle_column(&self) {
        self.column_slide.set_reveal_child(!self.column_slide.reveals_child());
    }
}
