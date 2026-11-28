//! What plays in another tab, at the foot of the column: the tab's mark and title, its pause, and a click on it to
//! go there. Paused from here, it stays with its play and a × to put it
//! away, until it plays again, its page goes or the tab sleeps.
//!
//! For the newest tab to make a sound that is out of sight: not the page on
//! screen nor its split partner, and of this space only. Off unless
//! `player = on`; off, nothing of it reaches a page. The pause
//! is two lines of script in rippl's own world, run when it is pressed.

use rippl_core::address::monogram;
use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{gio, glib, gtk};

use crate::browser::Browser;

/// What plays here, paused and remembered.
const STILL: &str = "(function () {
  var held = [];
  document.querySelectorAll('audio, video').forEach(function (el) {
    if (el.paused) return;
    held.push(el);
    el.pause();
  });
  window.__ripplStilled = held;
  return held.length;
})()";

/// What `STILL` paused, playing again.
const AGAIN: &str = "(function () {
  var held = window.__ripplStilled || [];
  window.__ripplStilled = null;
  held.forEach(function (el) {
    if (!el.paused || !el.isConnected) return;
    var played = el.play();
    if (played && played.catch) played.catch(function () {});
  });
  return held.length;
})()";

impl Browser {
    /// A tab's sound started or stopped.
    pub(crate) fn heard(&self, id: TabId, noisy: bool) {
        let mut sounding = self.sounding.borrow_mut();
        if noisy {
            sounding.retain(|t| *t != id);
            sounding.push(id);
            self.stilled.borrow_mut().remove(&id);
        } else if !self.stilled.borrow().contains(&id) {
            sounding.retain(|t| *t != id);
        }
    }

    /// The tab the player is for, if any.
    fn player_tab(&self) -> Option<TabId> {
        if !self.config.borrow().player || self.config.borrow().icon_column || self.config.borrow().top_tabs {
            return None;
        }
        let shown = self.on_screen();
        let tabs = self.tabs.borrow();
        self.sounding.borrow().iter().rev().copied().find(|id| {
            !shown.contains(id)
                && tabs.get(*id).is_some_and(|t| t.space == tabs.space && t.life == Life::Awake)
        })
    }

    /// Draws the player row again (with the column).
    pub(crate) fn render_player(&self) {
        while let Some(child) = self.player_row.first_child() {
            self.player_row.remove(&child);
        }
        let Some(id) = self.player_tab() else {
            self.player_row.set_visible(false);
            return;
        };
        let (url, title) = match self.tabs.borrow().get(id) {
            Some(t) => (t.url.clone(), t.title.clone()),
            None => return,
        };
        let stilled = self.stilled.borrow().contains(&id);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        match self.icon_for(id, &url) {
            Some(icon) => {
                let image = gtk::Image::from_paintable(Some(&icon));
                image.set_pixel_size(15);
                image.add_css_class("icon");
                image.set_valign(gtk::Align::Center);
                line.append(&image);
            }
            None => {
                let mark = gtk::Label::new(Some(&monogram(&url, &title)));
                mark.add_css_class("mark");
                mark.set_valign(gtk::Align::Center);
                line.append(&mark);
            }
        }
        let label = gtk::Label::builder()
            .label(&title)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(1)
            .build();
        line.append(&label);
        let button = |icon: &str, tip: &str| {
            let b = gtk::Button::from_icon_name(icon);
            b.add_css_class("player-button");
            b.set_tooltip_text(Some(tip));
            b.set_focusable(false);
            b
        };
        if stilled {
            let away = button("window-close-symbolic", "Put away");
            let weak = self.weak();
            away.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.stilled.borrow_mut().remove(&id);
                    b.sounding.borrow_mut().retain(|t| *t != id);
                    b.render_player();
                }
            });
            line.append(&away);
        }
        let toggle = if stilled {
            button("media-playback-start-symbolic", "Play")
        } else {
            button("media-playback-pause-symbolic", "Pause")
        };
        let weak = self.weak();
        toggle.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.player_toggle(id);
            }
        });
        line.append(&toggle);
        self.player_row.append(&line);
        self.player_row.set_tooltip_text(Some("Go to the tab"));
        self.player_row.set_visible(true);
        *self.player_for.borrow_mut() = Some(id);
    }

    /// Pause from the player, or play what it paused.
    pub(crate) fn player_toggle(&self, id: TabId) {
        let Some(view) = self.views.borrow().get(&id).cloned() else { return };
        let again = self.stilled.borrow().contains(&id);
        if !again {
            self.stilled.borrow_mut().insert(id);
        }
        let weak = self.weak();
        view.evaluate_javascript(if again { AGAIN } else { STILL }, Some("rippl"), None, None::<&gio::Cancellable>, move |r| {
            let Some(b) = weak.upgrade() else { return };
            // Nothing a script could reach: nothing to play again later.
            let none = matches!(r.as_ref().map(|v| v.to_int32()), Ok(0));
            if again || none {
                b.stilled.borrow_mut().remove(&id);
            }
            glib::idle_add_local_once(move || b.render_player());
        });
    }

    pub(crate) fn wire_player(&self) {
        let click = gtk::GestureClick::new();
        let weak = self.weak();
        click.connect_released(move |gesture, _, x, y| {
            let Some(b) = weak.upgrade() else { return };
            // The buttons take their own clicks; the rest is the way there.
            let hit = b.player_row.pick(x, y, gtk::PickFlags::DEFAULT);
            if hit.is_some_and(|w| w.is::<gtk::Button>() || w.ancestor(gtk::Button::static_type()).is_some()) {
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            if let Some(id) = *b.player_for.borrow() {
                b.select(id);
            }
        });
        self.player_row.add_controller(click);
    }
}
