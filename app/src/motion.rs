//! Motion: what moves, and how.
//!
//! the three movements, as CSS: **glide**, a spring (0.34 s, a little
//! give at the end) for what comes onto the page: peek, the switcher, a
//! popover; **settle**, a firmer spring (0.30 s) for what slides into
//! place: bars, rows, the panel; **quick**, a 0.14 s ease-out, for hovers
//! and colours. A pressed button gives to 97%, a fade is
//! 0.2 s.
//!
//! - Popovers and menus pop in; the find bar, a page's question and the
//!   save-password bar drop in; a notice rises, and fades as it goes.
//! - Peek and the Ctrl+Tab cards scale up over a fading dim; the AI panel
//!   slides in from the right.
//! - A new tab slides into the column; one closed with the mouse fades and
//!   folds away. Going from tab to tab cross-fades (0.12 s); Ctrl+S slides
//!   the column away and back.
//! - Pages stretch a little at their top and bottom and spring back
//!   (elastic edges, pagescripts.rs).
//!
//! `motion = reduced` makes all of it immediate, GTK's own animations too.
//! Nothing here runs while nothing moves: CSS animations and transitions
//! cost frames only while they play.

use std::time::Duration;

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

/// A row folding away before its tab closes, and a notice fading.
const LEAVE: Duration = Duration::from_millis(170);

pub(crate) const CSS: &str = r#"
/* the springs, as curves: glide overshoots a little, settle less. */
@keyframes rippl-pop { from { opacity: 0; transform: scale(0.94); } to { opacity: 1; transform: none; } }
@keyframes rippl-fade { from { opacity: 0; } to { opacity: 1; } }
@keyframes rippl-fade-out { from { opacity: 1; } to { opacity: 0; } }
@keyframes rippl-rise { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: none; } }
@keyframes rippl-sink { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateY(8px); } }
@keyframes rippl-drop { from { opacity: 0; transform: translateY(-10px); } to { opacity: 1; transform: none; } }
@keyframes rippl-in-left { from { opacity: 0; transform: translateX(-14px); } to { opacity: 1; transform: none; } }
@keyframes rippl-in-right { from { opacity: 0; transform: translateX(24px); } to { opacity: 1; transform: none; } }
@keyframes rippl-out-left { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateX(-18px) scale(0.98); } }

/* What comes and goes carries .enter while it is shown (motion.rs,
   `enters`): GTK starts an animation when a style changes, not when a
   widget shows again. */

/* Popovers: the site card, menus, the accounts list, folders, the » menu. */
popover.enter > contents { animation: rippl-pop 240ms cubic-bezier(0.2, 1.3, 0.4, 1); }

/* Bars over the page drop in; notices rise and sink. */
window.rippl .find.enter, window.rippl .ask.enter { animation: rippl-drop 300ms cubic-bezier(0.25, 1.15, 0.4, 1); }
window.rippl .toast.enter { animation: rippl-rise 300ms cubic-bezier(0.25, 1.2, 0.4, 1); }
window.rippl .toast.enter.leaving { animation: rippl-sink 170ms ease-in forwards; }
window.rippl .status.enter { animation: rippl-fade 140ms ease-out; }

/* Peek and the switcher: the dim fades, the panel glides up. */
window.rippl .peek-dim.enter, window.rippl .switcher-dim.enter { animation: rippl-fade 200ms ease-out; }
window.rippl .peek-dim.enter .peek-panel { animation: rippl-pop 340ms cubic-bezier(0.2, 1.25, 0.4, 1); }
window.rippl .switcher-dim.enter .switcher { animation: rippl-pop 280ms cubic-bezier(0.2, 1.3, 0.4, 1); }
window.rippl .switch-card { transition: background-color 140ms ease-out, transform 200ms cubic-bezier(0.2, 1.3, 0.4, 1); }
window.rippl .switch-card.chosen { transform: scale(1.03); }

/* The AI panel slides in; the new tab's field rises into place. */
window.rippl .ai-panel.enter { animation: rippl-in-right 300ms cubic-bezier(0.25, 1.15, 0.4, 1); }
window.rippl .field.home-field { animation: rippl-rise 380ms cubic-bezier(0.25, 1.15, 0.4, 1); }
window.rippl .home-offers { animation: rippl-pop 220ms cubic-bezier(0.2, 1.3, 0.4, 1); }
window.rippl .home-grid { animation: rippl-fade 300ms ease-out; }

/* Tabs: a new one slides in; one closed with the mouse folds away. */
window.rippl .tabs row.fresh, window.rippl .top-tab.fresh { animation: rippl-in-left 300ms cubic-bezier(0.25, 1.2, 0.4, 1); }
window.rippl .tabs row.leaving, window.rippl .top-tab.leaving { animation: rippl-out-left 170ms ease-in forwards; }
window.rippl .pin.fresh { animation: rippl-pop 280ms cubic-bezier(0.2, 1.3, 0.4, 1); }

/* Hover and choice: colours ease; pressing gives. */
window.rippl .tabs row, window.rippl .pin, window.rippl .new-tab, window.rippl .top-tab, window.rippl .rail-row,
window.rippl .rail-back, window.rippl .bar-item, window.rippl .foot button, window.rippl .top-knob,
window.rippl .menu-item, window.rippl .space, window.rippl .swatch, window.rippl .player-button, window.rippl .home-tile,
window.rippl button {
  transition: background-color 140ms ease-out, color 140ms ease-out, box-shadow 200ms ease-out, opacity 140ms ease-out,
    transform 160ms cubic-bezier(0.2, 1.3, 0.4, 1);
}
window.rippl .pin:active, window.rippl .tabs row:active, window.rippl .top-tab:active, window.rippl .rail-row:active,
window.rippl .new-tab:active, window.rippl .bar-item:active, window.rippl .home-tile:active, window.rippl .switch-card:active,
window.rippl button:active { transform: scale(0.97); }
window.rippl .field { transition: background-color 160ms ease-out, box-shadow 220ms ease-out; }
window.rippl .rail-row.here, window.rippl .tabs row.front { transition: background-color 200ms ease-out, box-shadow 240ms ease-out; }
"#;

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

    /// Full motion, or none (`motion = reduced`): GTK's own animations
    /// with it, and the page cross-fade.
    pub(crate) fn apply_motion(&self) {
        let full = self.config.borrow().motion_full;
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
        if !self.config.borrow().motion_full {
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
        if !self.config.borrow().motion_full {
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

    /// Ctrl+S: the column slides away, or back.
    pub(crate) fn toggle_column(&self) {
        self.column_slide.set_reveal_child(!self.column_slide.reveals_child());
    }
}
