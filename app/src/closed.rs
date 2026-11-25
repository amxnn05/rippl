//! Tabs closed lately, to bring back with Ctrl+Shift+T ⇧⌘T:
//! the same address, back in its place, with its Back/Forward history and
//! where it was scrolled. Never a private tab's. The last 25, in memory
//! only: they go at quit.
//!
//! Also here, the small address commands it has: copy the address
//! (Ctrl+Shift+C) and paste and go (Ctrl+Shift+V).

use rippl_core::tabs::Life;
use webkit6::prelude::*;
use webkit6::gdk;

use crate::browser::Browser;

const KEPT: usize = 25;

pub struct Closed {
    pub url: String,
    pub title: String,
    /// WebKit's session state (history, scroll), if the tab had a page.
    pub memory: Option<Vec<u8>>,
    pub index: usize,
    pub pinned: bool,
}

impl Browser {
    /// Called by `close` before the tab goes.
    pub(crate) fn remember_closed(&self, id: rippl_core::tabs::TabId) {
        let tabs = self.tabs.borrow();
        let Some(tab) = tabs.get(id) else { return };
        if tab.private || tab.url.is_empty() {
            return;
        }
        let memory = match self.views.borrow().get(&id) {
            Some(view) => view.session_state().and_then(|s| s.serialize()).map(|b| b.to_vec()),
            None => tab.memory.clone(),
        };
        let mut closed = self.closed.borrow_mut();
        closed.push(Closed {
            url: tab.url.clone(),
            title: tab.title.clone(),
            memory,
            index: tabs.index(id).unwrap_or(0),
            pinned: tab.pinned,
        });
        if closed.len() > KEPT {
            closed.remove(0);
        }
    }

    /// Ctrl+Shift+T: the last tab closed, back where it was.
    pub(crate) fn reopen_closed(&self) {
        let Some(c) = self.closed.borrow_mut().pop() else {
            self.notice("No closed tabs to bring back");
            return;
        };
        let id = {
            let mut tabs = self.tabs.borrow_mut();
            let id = tabs.push(&c.url);
            if let Some(t) = tabs.get_mut(id) {
                t.title = c.title.clone();
                t.pinned = c.pinned;
                // Asleep with its memory: waking restores history and scroll.
                t.life = if c.memory.is_some() { Life::Asleep } else { Life::Lazy };
                t.memory = c.memory.clone();
            }
            tabs.move_to(id, c.index);
            id
        };
        self.dirty.set(true);
        self.select(id);
    }

    /// Ctrl+Shift+C: the front page's address to the clipboard.
    pub(crate) fn copy_address(&self) {
        let url = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone()));
        if let Some(url) = url.filter(|u| !u.is_empty()) {
            // Clean: no click IDs, no campaign tags.
            let url = rippl_core::clean::clean_link(&url);
            gdk::prelude::DisplayExt::clipboard(&WidgetExt::display(&self.window)).set_text(&url);
            self.notice("Address copied");
        }
    }

    /// Ctrl+Shift+V: go to (or search for) what is on the clipboard.
    pub(crate) fn paste_and_go(&self) {
        let clipboard = gdk::prelude::DisplayExt::clipboard(&WidgetExt::display(&self.window));
        let weak = self.weak();
        clipboard.read_text_async(None::<&webkit6::gio::Cancellable>, move |text| {
            let (Some(b), Ok(Some(text))) = (weak.upgrade(), text) else { return };
            let text = text.trim().to_string();
            if !text.is_empty() && text.len() < 8192 {
                b.open("", false, true);
                b.go(&text);
            }
        });
    }
}
