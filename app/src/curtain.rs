//! Hide anything on a page, for good (Ctrl+Shift+H). The cookie banner, the newsletter box, the "related"
//! rail: click it and it is gone, now and on every later visit to that site,
//! before the page draws a frame.
//!
//! - The picker runs in rippl's isolated script world: it outlines what is
//!   under the pointer, and a click hides it and reports a CSS selector
//!   that finds exactly that element. Esc cancels.
//! - The report comes back through a script message handler registered
//!   only in that world, so a page's own scripts can't send one.
//! - Each site's selectors become one user style sheet, limited to that
//!   site's pages (and its subdomains), with `display: none !important`.
//!   WebKit applies them as pages load, so nothing flashes.
//! - Ctrl+Shift+U shows everything on the site again; rippl://settings
//!   lists and clears them too.

use webkit6::prelude::*;
use webkit6::gio;

use crate::browser::Browser;
use crate::shield::host_of;

pub const HANDLER: &str = "ripplHide";

const PICKER: &str = r##"
(() => {
  if (window.__ripplPicking) return;
  window.__ripplPicking = true;
  const box = document.createElement("div");
  box.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;border:2px solid #e0457b;background:rgba(224,69,123,.15);border-radius:3px;transition:all .05s";
  document.documentElement.appendChild(box);
  let target = null;
  const esc = s => CSS.escape(s);
  const unique = sel => { try { return document.querySelectorAll(sel).length === 1; } catch (e) { return false; } };
  // A selector for exactly this element: an id if it has a stable one,
  // otherwise classes and :nth-of-type steps up to the nearest id.
  const selector = (el) => {
    if (el.id && !/\d{3,}/.test(el.id) && unique("#" + esc(el.id))) return "#" + esc(el.id);
    const parts = [];
    for (let n = el; n && n.nodeType === 1 && n !== document.documentElement; n = n.parentElement) {
      if (n !== el && n.id && !/\d{3,}/.test(n.id)) { parts.unshift("#" + esc(n.id)); break; }
      let part = n.tagName.toLowerCase();
      const classes = [...n.classList].filter(c => !/\d{3,}|^(is-|has-|js-)|active|hover|open/.test(c)).slice(0, 3);
      if (classes.length) part += "." + classes.map(esc).join(".");
      const same = n.parentElement ? [...n.parentElement.children].filter(c => c.tagName === n.tagName) : [];
      if (same.length > 1) part += ":nth-of-type(" + (same.indexOf(n) + 1) + ")";
      parts.unshift(part);
      const sel = parts.join(" > ");
      if (unique(sel)) return sel;
    }
    return parts.join(" > ");
  };
  const move = e => {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    if (!el || el === box || el === document.body || el === document.documentElement) return;
    target = el;
    const r = el.getBoundingClientRect();
    Object.assign(box.style, { left: r.left + "px", top: r.top + "px", width: r.width + "px", height: r.height + "px" });
  };
  const stop = () => {
    window.__ripplPicking = false;
    box.remove();
    removeEventListener("mousemove", move, true);
    removeEventListener("click", click, true);
    removeEventListener("keydown", key, true);
  };
  const click = e => {
    e.preventDefault(); e.stopPropagation(); e.stopImmediatePropagation();
    // What is under the click, not what was last hovered: the pointer can
    // move between the two.
    const under = document.elementFromPoint(e.clientX, e.clientY);
    if (under && under !== box && under !== document.body && under !== document.documentElement) target = under;
    if (target) {
      const sel = selector(target);
      target.style.setProperty("display", "none", "important");
      window.webkit.messageHandlers.ripplHide.postMessage(JSON.stringify({ host: location.hostname, selector: sel }));
    }
    stop();
  };
  const key = e => { if (e.key === "Escape") { e.preventDefault(); stop(); } };
  addEventListener("mousemove", move, true);
  addEventListener("click", click, true);
  addEventListener("keydown", key, true);
})()
"##;

impl Browser {
    /// Listens for the picker's reports, and puts the kept style sheets in.
    pub(crate) fn wire_curtain(&self) {
        if !self.primary {
            return;
        }
        for content in self.engine.contents() {
            content.register_script_message_handler(HANDLER, Some("rippl"));
            content.connect_script_message_received(Some(HANDLER), move |_, value| {
                let Some(b) = crate::windows::all().into_iter().next() else { return };
                let Ok(report) = serde_json::from_str::<serde_json::Value>(&value.to_str()) else { return };
                let host = report.get("host").and_then(|h| h.as_str()).unwrap_or("");
                let selector = report.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                b.keep_hidden(host, selector);
            });
        }
        self.apply_hidden();
    }

    /// Ctrl+Shift+H: start picking on the page in front.
    pub(crate) fn pick_to_hide(&self) {
        let Some(view) = self.front_view() else { return };
        if !view.uri().is_some_and(|u| u.starts_with("http")) {
            return;
        }
        view.grab_focus();
        self.notice("Click what you want gone · Esc to cancel");
        view.evaluate_javascript(PICKER, Some("rippl"), None, None::<&gio::Cancellable>, |_| {});
    }

    /// Keeps `selector` hidden on `host` (as the picker reported it).
    pub(crate) fn keep_hidden(&self, host: &str, selector: &str) {
        let host = host.trim_start_matches("www.").to_ascii_lowercase();
        if host.is_empty() || selector.is_empty() || selector.len() > 1000 {
            return;
        }
        let private = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.private)).unwrap_or(false);
        if private {
            return; // hidden on the page; nothing kept from a private tab
        }
        if let Err(e) = self.places.hide(&host, selector) {
            eprintln!("rippl: hide: {e}");
            return;
        }
        self.apply_hidden();
        self.notice(&format!("Hidden on {host} · Ctrl+Shift+U shows it again"));
    }

    /// Ctrl+Shift+U: everything hidden on the front page's site, back.
    pub(crate) fn unhide_here(&self) {
        let url = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone()));
        let Some(host) = url.as_deref().and_then(host_of) else { return };
        if self.places.unhide(Some(&host)).unwrap_or(0) > 0 {
            self.apply_hidden();
            if let Some(v) = self.front_view() {
                v.reload();
            }
            self.notice(&format!("Everything on {host} is shown again"));
        }
    }

    /// Hidden elements live with the rest of the sites' styles (pagelook.rs).
    pub(crate) fn apply_hidden(&self) {
        self.apply_site_styles();
    }
}
