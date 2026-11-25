//! Signing in: noticing a sign-in that worked and offering to save it,
//! and offering saved accounts under the box when you click it.
//!
//! **Saving.** A script in rippl's isolated world (pages can't see it)
//! watches the top frame's forms. When a form with a filled password box
//! is sent (submit, Enter in the password box, or a click on a button),
//! it reports the username and password once. rippl holds that in memory,
//! wiped on drop, and only asks to save once the sign-in has **worked**:
//! - the page loaded something new with no password box on it, or
//! - on a single-page app, the password box went away.
//!
//! A page that comes back with its password box (a wrong password) is
//! never offered. The question is a bar over the page: *Save*, *Not now*,
//! *Never for this site*, or *Update* when the username is saved with
//! another password. Nothing is asked or kept from a private tab, from an
//! http page (except on this machine), or from a site on the never list,
//! and the offer lapses after two minutes.
//!
//! **Filling.** Nothing is ever filled on its own. Clicking a sign-in box
//! on a page whose exact origin has saved accounts shows them under the
//! box. Picking one reads that password from the keyring, checks again
//! that the page in front is still on that origin, and only then puts it
//! into the form, in the top frame only.

use std::time::{Duration, Instant};

use rippl_core::passwords::{self, Ask, Capture};
use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gdk, gio, glib, gtk, UserContentInjectedFrames, UserScript, UserScriptInjectionTime, WebView};
use zeroize::Zeroizing;

use crate::browser::Browser;
use crate::keyring::Account;

pub const HANDLER: &str = "ripplPasswords";
const OFFER_LASTS: Duration = Duration::from_secs(120);

const FORMS: &str = r##"
(() => {
  if (window.__ripplForms) return;
  window.__ripplForms = true;
  const post = m => { m.url = location.href; window.webkit.messageHandlers.ripplPasswords.postMessage(JSON.stringify(m)); };
  const shown = el => el && el.isConnected && el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden";
  const passwordBoxes = () => [...document.querySelectorAll("input[type=password]")].filter(shown);
  const TEXTY = "input:not([type]), input[type=text], input[type=email], input[type=tel], input[type=number]";
  // The username box for a password box: one marked username/email, or
  // else the last text-like box before it (in its form, if it has one).
  const userFor = pw => {
    const scope = pw.form || document;
    const boxes = [...scope.querySelectorAll(TEXTY)].filter(shown);
    const marked = boxes.find(b => /username|email/.test(b.autocomplete || ""));
    if (marked) return marked;
    let best = null;
    for (const b of boxes) if (b.compareDocumentPosition(pw) & Node.DOCUMENT_POSITION_FOLLOWING) best = b;
    return best;
  };
  let sent = "";
  const watchGone = pw => {
    let n = 0;
    const t = setInterval(() => {
      if (!shown(pw)) { clearInterval(t); post({ kind: "gone" }); }
      if (++n > 30) clearInterval(t);
    }, 500);
  };
  const capture = () => {
    const filled = passwordBoxes().filter(p => p.value);
    if (!filled.length) return;
    const pw = filled[0], user = userFor(pw);
    const m = { kind: "capture", username: user ? user.value : "", password: pw.value,
      newPassword: pw.autocomplete === "new-password" || filled.length > 1 };
    const key = m.username + "\u0000" + m.password;
    if (key === sent) return;
    sent = key;
    post(m);
    watchGone(pw);
  };
  addEventListener("submit", capture, true);
  addEventListener("keydown", e => { if (e.key === "Enter" && e.target && e.target.type === "password") capture(); }, true);
  addEventListener("click", e => {
    const b = e.target.closest && e.target.closest("button, input[type=submit], [role=button]");
    if (b && passwordBoxes().some(p => p.value)) setTimeout(capture, 0);
  }, true);
  // A click into a sign-in box: tell rippl where it is, so saved accounts
  // can be offered under it.
  addEventListener("focusin", e => {
    const el = e.target;
    if (!(el instanceof HTMLInputElement)) return;
    const pw = el.type === "password" ? el : passwordBoxes()[0];
    if (!pw || (el !== pw && userFor(pw) !== el)) return;
    const r = el.getBoundingClientRect();
    post({ kind: "focus", x: r.left, y: r.bottom, width: r.width });
  }, true);
  // Filling, called by rippl after you pick an account: the native value
  // setter, then the events a framework listens for.
  window.__ripplFill = (username, password) => {
    const pw = (document.activeElement && document.activeElement.type === "password" && shown(document.activeElement))
      ? document.activeElement : passwordBoxes()[0];
    if (!pw) return false;
    const setValue = (el, v) => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, v);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
    };
    const user = userFor(pw);
    if (user && username) setValue(user, username);
    setValue(pw, password);
    return true;
  };
  window.__ripplHasPasswordBox = () => passwordBoxes().length > 0;
})()
"##;

/// A sign-in that was sent and may have worked.
pub struct Pending {
    pub origin: String,
    pub capture: Capture,
    pub at: Instant,
}

impl Drop for Pending {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.capture.password.zeroize();
    }
}

/// The question on screen (or waiting for its tab to come to front).
pub struct SaveQuestion {
    pub tab: TabId,
    pub pending: Pending,
    pub ask: Ask,
}

impl Browser {
    pub(crate) fn save_bar() -> (gtk::Box, gtk::Label, gtk::Button, gtk::Button, gtk::Button) {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("ask");
        bar.set_halign(gtk::Align::Center);
        bar.set_valign(gtk::Align::Start);
        bar.set_margin_top(10);
        let label = gtk::Label::new(None);
        let never = gtk::Button::with_label("Never for this site");
        let later = gtk::Button::with_label("Not now");
        let save = gtk::Button::with_label("Save");
        save.add_css_class("suggested-action");
        bar.append(&label);
        bar.append(&never);
        bar.append(&later);
        bar.append(&save);
        bar.set_visible(false);
        (bar, label, never, later, save)
    }

    pub(crate) fn wire_signin(&self) {
        let content = &self.engine.content;
        content.add_script(&UserScript::for_world(
            FORMS,
            UserContentInjectedFrames::TopFrame,
            UserScriptInjectionTime::End,
            "rippl",
            &[],
            &[],
        ));
        content.register_script_message_handler(HANDLER, Some("rippl"));
        let weak = self.weak();
        content.connect_script_message_received(Some(HANDLER), move |_, value| {
            if let Some(b) = weak.upgrade() {
                b.signin_message(&value.to_str());
            }
        });

        let weak = self.weak();
        self.save_yes.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.answer_save("save");
            }
        });
        let weak = self.weak();
        self.save_later.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.answer_save("later");
            }
        });
        let weak = self.weak();
        self.save_never.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.answer_save("never");
            }
        });

        self.accounts_popover.set_parent(&self.page_area);
        self.accounts_popover.set_position(gtk::PositionType::Bottom);
        self.accounts_popover.set_has_arrow(false);
        let weak = self.weak();
        self.accounts_list.connect_row_activated(move |_, row| {
            let Some(b) = weak.upgrade() else { return };
            let account = b.offered.borrow().get(row.index() as usize).cloned();
            b.accounts_popover.popdown();
            if let Some(account) = account {
                b.fill(account);
            }
        });
    }

    /// Which tab a message came from: the front tab if it shows that
    /// address, otherwise the first tab that does.
    fn tab_showing(&self, url: &str) -> Option<(TabId, WebView)> {
        let views = self.views.borrow();
        let showing = |id: &TabId| views.get(id).filter(|v| v.uri().as_deref() == Some(url)).cloned();
        if let Some(front) = self.front()
            && let Some(v) = showing(&front)
        {
            return Some((front, v));
        }
        views.iter().find(|(_, v)| v.uri().as_deref() == Some(url)).map(|(id, v)| (*id, v.clone()))
    }

    fn signin_message(&self, json: &str) {
        let Ok(m) = serde_json::from_str::<serde_json::Value>(json) else { return };
        let text = |k: &str| m.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = text("url");
        let Some((tab, _)) = self.tab_showing(&url) else { return };
        let Some(origin) = passwords::origin_of(&url) else { return };
        match text("kind").as_str() {
            "capture" => {
                let private = self.tabs.borrow().get(tab).is_none_or(|t| t.private);
                if private || !passwords::may_save(&origin) {
                    return;
                }
                let new = m.get("newPassword").and_then(|v| v.as_bool()).unwrap_or(false);
                let password = Zeroizing::new(text("password"));
                if let Some(capture) = Capture::new(&text("username"), &password, new) {
                    self.pending.borrow_mut().insert(tab, Pending { origin, capture, at: Instant::now() });
                }
            }
            "gone" => self.signed_in(tab),
            "focus" => {
                let x = m.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = m.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let w = m.get("width").and_then(|v| v.as_f64()).unwrap_or(200.0);
                self.offer_accounts(tab, origin, (x, y, w));
            }
            _ => {}
        }
    }

    /// A page finished loading in `tab`: if a sign-in was sent from it and
    /// the new page has no password box, the sign-in worked.
    pub(crate) fn signin_page_loaded(&self, tab: TabId, view: &WebView) {
        if !self.pending.borrow().contains_key(&tab) {
            return;
        }
        let weak = self.weak();
        view.evaluate_javascript(
            "typeof __ripplHasPasswordBox === 'function' ? __ripplHasPasswordBox() : false",
            Some("rippl"),
            None,
            None::<&gio::Cancellable>,
            move |result| {
                let Some(b) = weak.upgrade() else { return };
                match result {
                    Ok(v) if v.to_boolean() => {
                        // Back on a sign-in form: most likely it failed.
                        b.pending.borrow_mut().remove(&tab);
                    }
                    _ => b.signed_in(tab),
                }
            },
        );
    }

    /// The sign-in from `tab` worked: decide whether to ask.
    fn signed_in(&self, tab: TabId) {
        let Some(pending) = self.pending.borrow_mut().remove(&tab) else { return };
        if pending.at.elapsed() > OFFER_LASTS {
            return;
        }
        let never = self.places.never_save(&pending.origin).unwrap_or(false);
        let keyring = self.keyring.clone();
        let weak = self.weak();
        glib::MainContext::default().spawn_local(async move {
            let saved = keyring.password(&pending.origin, &pending.capture.username).await.unwrap_or(None);
            let ask = passwords::what_to_ask(&pending.capture, saved.as_deref().map(|s| s.as_str()), never);
            let Some(b) = weak.upgrade() else { return };
            if ask == Ask::Nothing {
                return;
            }
            *b.save_question.borrow_mut() = Some(SaveQuestion { tab, pending, ask });
            b.show_save_question();
        });
    }

    /// Shows the save question if its tab is in front.
    pub(crate) fn show_save_question(&self) {
        let question = self.save_question.borrow();
        match question.as_ref().filter(|q| Some(q.tab) == self.front()) {
            Some(q) => {
                let host = passwords::host_of_origin(&q.pending.origin);
                let who = if q.pending.capture.username.is_empty() {
                    String::new()
                } else {
                    format!(" for {}", q.pending.capture.username)
                };
                let (text, button) = match q.ask {
                    Ask::Update => (format!("Update the saved password{who} on {host}?"), "Update"),
                    _ => (format!("Save the password{who} on {host} in your keyring?"), "Save"),
                };
                self.save_label.set_label(&text);
                self.save_yes.set_label(button);
                self.save_bar.set_visible(true);
            }
            None => self.save_bar.set_visible(false),
        }
    }

    /// `save`, `later` or `never`.
    pub(crate) fn answer_save(&self, answer: &str) {
        let Some(question) = self.save_question.borrow_mut().take() else { return };
        self.save_bar.set_visible(false);
        let origin = question.pending.origin.clone();
        match answer {
            "save" => {
                let keyring = self.keyring.clone();
                let weak = self.weak();
                glib::MainContext::default().spawn_local(async move {
                    let c = &question.pending.capture;
                    let result = keyring.save(&question.pending.origin, &c.username, &c.password).await;
                    let Some(b) = weak.upgrade() else { return };
                    match result {
                        Ok(()) => b.notice("Saved in your keyring"),
                        Err(e) => {
                            eprintln!("rippl: keyring: {e}");
                            b.notice("Couldn't save: no keyring is available");
                        }
                    }
                });
            }
            "never" => {
                let _ = self.places.set_never_save(&origin, true);
                self.notice(&format!("rippl won't offer to save passwords on {}", passwords::host_of_origin(&origin)));
            }
            _ => {}
        }
    }

    /// A sign-in box was clicked: offer the origin's saved accounts.
    fn offer_accounts(&self, tab: TabId, origin: String, (x, y, width): (f64, f64, f64)) {
        if Some(tab) != self.front() || self.tabs.borrow().get(tab).is_none_or(|t| t.private) {
            return;
        }
        let keyring = self.keyring.clone();
        let weak = self.weak();
        glib::MainContext::default().spawn_local(async move {
            let accounts = keyring.accounts(Some(&origin)).await.unwrap_or_default();
            let Some(b) = weak.upgrade() else { return };
            if accounts.is_empty() || b.front() != Some(tab) {
                return;
            }
            let Some(view) = b.front_view() else { return };
            // The page's CSS pixels, scaled by its zoom, into the page area.
            let zoom = view.zoom_level();
            let point = gtk::graphene::Point::new((x * zoom) as f32, (y * zoom) as f32);
            let Some(at) = view.compute_point(&b.page_area, &point) else { return };
            while let Some(child) = b.accounts_list.first_child() {
                b.accounts_list.remove(&child);
            }
            for a in &accounts {
                let label = gtk::Label::builder()
                    .label(if a.username.is_empty() { "(no username)" } else { &a.username })
                    .xalign(0.0)
                    .build();
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&label));
                b.accounts_list.append(&row);
            }
            *b.offered.borrow_mut() = accounts;
            let rect = gdk::Rectangle::new(at.x() as i32, at.y() as i32, ((width * zoom) as i32).max(1), 1);
            b.accounts_popover.set_pointing_to(Some(&rect));
            b.accounts_popover.popup();
        });
    }

    /// Puts `account` into the form in front, if the page is still on the
    /// account's origin.
    pub(crate) fn fill(&self, account: Account) {
        let keyring = self.keyring.clone();
        let weak = self.weak();
        glib::MainContext::default().spawn_local(async move {
            let password = match keyring.password(&account.origin, &account.username).await {
                Ok(Some(p)) => p,
                Ok(None) => return,
                Err(e) => {
                    eprintln!("rippl: keyring: {e}");
                    return;
                }
            };
            let Some(b) = weak.upgrade() else { return };
            let Some(view) = b.front_view() else { return };
            // The page may have moved on while the keyring answered.
            let here = view.uri().and_then(|u| passwords::origin_of(&u));
            if here.as_deref() != Some(account.origin.as_str()) {
                return;
            }
            // Every copy of the password rippl makes is wiped when dropped.
            let quoted = Zeroizing::new(serde_json::to_string(password.as_str()).unwrap_or_default());
            let user = serde_json::to_string(&account.username).unwrap_or_default();
            let script = Zeroizing::new(format!("__ripplFill({user}, {})", quoted.as_str()));
            view.grab_focus();
            view.evaluate_javascript(&script, Some("rippl"), None, None::<&gio::Cancellable>, |_| {});
        });
    }
}
