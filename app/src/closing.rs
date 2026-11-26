//! Tabs that close themselves, with
//! `close_after = 12h | 1d | 7d | 30d` (never, unless chosen).
//!
//! A tab nobody has looked at for that long closes as Ctrl+W would: Ctrl+
//! Shift+T brings it back while it is recent, rippl://archive for a month
//!, and History has its page for good. The time
//! counts from when you last left the tab, and goes on counting while rippl
//! is closed: the session keeps it.
//!
//! What stays (`rippl_core::tabs::stays_open`): the page on screen, pinned
//! tabs, tabs in a group, sites kept awake from their card, private tabs
//! (nothing could bring them back), blank tabs, and anything busy (playing,
//! on a call, loading). A page holding something typed and not sent stays
//! too; a split pair goes whole or not at all.

use rippl_core::tabs::{self, Life, TabId};
use webkit6::prelude::*;
use webkit6::gio;

use crate::browser::Browser;

/// Whether the page holds something typed and not sent.
const TYPED: &str = "(function () {
  var boxes = document.querySelectorAll('textarea, input:not([type=hidden]):not([type=password]):not([type=checkbox]):not([type=radio]):not([type=submit]):not([type=button])');
  for (var i = 0; i < boxes.length; i++) {
    if (boxes[i].value && boxes[i].value !== boxes[i].defaultValue) return true;
  }
  var edit = document.querySelector('[contenteditable=\"\"], [contenteditable=true]');
  return !!(edit && edit.textContent.trim().length > 0 && document.activeElement === edit);
})()";

impl Browser {
    /// Every tab left alone past its time. At start-up, then with each
    /// look for tabs to sleep.
    pub(crate) fn close_left_alone(&self) {
        let Some(after) = self.config.borrow().close_after else { return };
        let on_screen = self.on_screen();
        let pairs = self.splits.borrow().clone();
        let due = {
            let list = &self.tabs.borrow().list;
            tabs::due_for_closing(list, &on_screen, &pairs, rippl_core::places::now(), after)
        };
        for id in due {
            let view = (self.tabs.borrow().get(id).map(|t| t.life) == Some(Life::Awake))
                .then(|| self.views.borrow().get(&id).cloned())
                .flatten();
            let Some(view) = view else {
                self.retire(id);
                continue;
            };
            // Awake: asked first, as nothing typed may be lost.
            let weak = self.weak();
            view.evaluate_javascript(TYPED, Some("rippl"), None, None::<&gio::Cancellable>, move |r| {
                let Some(b) = weak.upgrade() else { return };
                let typed = r.map(|v| v.to_boolean()).unwrap_or(true);
                if !typed {
                    b.retire(id);
                }
            });
        }
    }

    /// Closes `id` into the archive, if it is still due (you may have gone
    /// back to it while its page was asked), with the rest of its split.
    fn retire(&self, id: TabId) {
        let Some(after) = self.config.borrow().close_after else { return };
        let on_screen = self.on_screen();
        let partners: Vec<TabId> = self.split_of(id).unwrap_or_default().into_iter().filter(|t| *t != id).collect();
        let still = |t: TabId| {
            let tabs = self.tabs.borrow();
            tabs.get(t).is_some_and(|tab| {
                tab.left <= rippl_core::places::now() - after.as_secs() as i64
                    && tabs::stays_open(tab, &on_screen).is_none()
            })
        };
        if !still(id) || partners.iter().any(|p| !still(*p)) {
            return;
        }
        for t in std::iter::once(id).chain(partners) {
            let (url, title) = match self.tabs.borrow().get(t) {
                Some(tab) => (tab.url.clone(), tab.title.clone()),
                None => continue,
            };
            if std::env::var_os("RIPPL_DEBUG").is_some() {
                eprintln!("rippl: closing {t}, left alone: {url}");
            }
            if self.config.borrow().history {
                let _ = self.places.archive_tab(&url, &title);
            }
            self.close(t);
        }
    }
}
