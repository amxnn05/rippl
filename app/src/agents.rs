//! A coding agent on this computer (Claude Code, Codex, …) using rippl,
//! when you let it: `agents = on`.
//!
//! The agent runs `rippl --mcp` (mcp.rs), an MCP server that speaks to
//! this rippl through its socket, `$XDG_RUNTIME_DIR/rippl.sock`: a file
//! only you can open, never on the network. Opened for agents alone (not
//! for tests), the socket takes only the commands here, `agent-…`.
//!
//! - It works in tabs of its own: opened behind the one you're on, never
//!   selected for you, marked ✦ in the column. It may go, read, click,
//!   type and take pictures there.
//! - Your tabs it may list and read only with `agents_see_tabs = on`, and
//!   never clicks, types or goes anywhere in them. Private tabs it never
//!   sees, nor rippl's own pages.
//! - Only http and https. No script of its own runs in a page: clicking
//!   and typing are rippl's own few lines, in rippl's world. Password
//!   fields it never types into or reads.
//! - Turning `agents` off closes the door at once; its tabs stay yours.

use std::collections::HashSet;

use rippl_core::tabs::{Life, TabId};
use serde_json::{Value, json};
use webkit6::prelude::*;
use webkit6::{gio, glib};

use crate::browser::Browser;

/// The most of a page's text one read returns.
const READ_MOST: usize = 50_000;

/// Waits (5 s at most) for the page to have loaded and `sel` to match,
/// as a person waits for a page: an agent can't tell when it is ready.
const FIND: &str = "
  const until = Date.now() + 5000;
  let el = null;
  while (Date.now() < until) {
    el = document.readyState === 'loading' ? null : document.querySelector(sel);
    if (el) break;
    await new Promise(r => setTimeout(r, 100));
  }
  if (!el) return 'no element matches';";

/// Clicks the first element `sel` matches, if it can be clicked.
const CLICK: &str = "
  el.scrollIntoView({block: 'center'});
  el.click();
  return 'ok';";

/// Types `text` into the box `sel` matches, never a password box.
const TYPE: &str = "
  if (el.type === 'password' || /password|one-time-code|cc-/.test(el.autocomplete || '')) return 'refused: a password or card field';
  if (!('value' in el) && !el.isContentEditable) return 'not a box to type in';
  el.focus();
  if (el.isContentEditable) el.textContent = text; else el.value = text;
  el.dispatchEvent(new Event('input', {bubbles: true}));
  el.dispatchEvent(new Event('change', {bubbles: true}));
  return 'ok';";

fn web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

impl Browser {
    /// Whether `id` is one of the agent's own tabs.
    fn agents_own(&self, id: TabId) -> bool {
        self.agent_tabs.borrow().contains(&id)
    }

    /// A tab it may read: its own, or yours when you let it (never private,
    /// never rippl's own pages).
    fn agent_may_read(&self, id: TabId) -> bool {
        let tabs = self.tabs.borrow();
        let Some(t) = tabs.get(id) else { return false };
        !t.private && web(&t.url) && (self.agents_own(id) || self.config.borrow().agents_see_tabs)
    }

    /// One agent command: `agent-<verb> <json>`. `None` if it isn't one.
    pub(crate) fn agent_command(&self, line: &str, reply: Box<dyn FnOnce(String)>) -> Option<Box<dyn FnOnce(String)>> {
        let Some(rest) = line.strip_prefix("agent-") else { return Some(reply) };
        if !self.config.borrow().agents {
            reply(json!({"error": "agents are off in rippl's settings (Settings › About › Developer: agents)"}).to_string());
            return None;
        }
        let (verb, args) = rest.split_once(' ').unwrap_or((rest, "{}"));
        let args: Value = serde_json::from_str(args).unwrap_or(json!({}));
        let tab = args.get("tab").and_then(Value::as_u64);
        let text = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let err = |e: &str| json!({"error": e}).to_string();
        self.agent_seen();
        match verb {
            "tabs" => {
                let see = self.config.borrow().agents_see_tabs;
                let tabs = self.tabs.borrow();
                let list: Vec<Value> = tabs
                    .list
                    .iter()
                    .filter(|t| !t.private && web(&t.url))
                    .filter(|t| see || self.agents_own(t.id))
                    .map(|t| {
                        json!({"tab": t.id, "url": t.url, "title": t.title, "own": self.agents_own(t.id), "loading": t.loading, "asleep": t.life != Life::Awake})
                    })
                    .collect();
                reply(json!(list).to_string());
            }
            "open" => {
                let url = text("url");
                if !web(&url) {
                    reply(err("only http and https addresses"));
                    return None;
                }
                let id = self.open(&url, false, false);
                self.agent_tabs.borrow_mut().insert(id);
                self.render();
                reply(json!({"tab": id}).to_string());
            }
            "go" | "close" | "click" | "type" | "read" | "shot" => {
                let Some(id) = tab else {
                    reply(err("which tab? (\"tab\": its number, from tabs)"));
                    return None;
                };
                let own = self.agents_own(id);
                let reading = matches!(verb, "read" | "shot");
                if !(own || reading && self.agent_may_read(id)) {
                    reply(err(if reading { "not a tab it may see" } else { "only in its own tabs" }));
                    return None;
                }
                match verb {
                    "close" => {
                        self.close(id);
                        self.agent_tabs.borrow_mut().remove(&id);
                        reply(json!({"ok": true}).to_string());
                    }
                    "go" => {
                        let url = text("url");
                        if !web(&url) {
                            reply(err("only http and https addresses"));
                            return None;
                        }
                        match self.agent_view(id) {
                            Some(v) => v.load_uri(&url),
                            None => {
                                if let Some(t) = self.tabs.borrow_mut().get_mut(id) {
                                    t.url = url.clone();
                                }
                            }
                        }
                        reply(json!({"ok": true}).to_string());
                    }
                    _ => {
                        let Some(view) = self.agent_view(id) else {
                            reply(err("that tab has no page yet"));
                            return None;
                        };
                        // A page still coming in: its first document goes
                        // when it does, and a script run in it with it. Wait
                        // for it (15 s at most).
                        let (verb, sel, typed) = (verb.to_string(), text("selector"), text("text"));
                        let weak = self.weak();
                        let go: Pending = std::rc::Rc::new(std::cell::RefCell::new(None));
                        let v = view.clone();
                        *go.borrow_mut() = Some(Box::new(move || {
                            if let Some(b) = weak.upgrade() {
                                b.agent_in_page(&verb, &v, &sel, &typed, reply);
                            }
                        }));
                        if !view.is_loading() {
                            if let Some(f) = go.borrow_mut().take() {
                                f();
                            }
                        } else {
                            let handler: std::rc::Rc<std::cell::Cell<Option<glib::SignalHandlerId>>> = std::rc::Rc::default();
                            let (g, h) = (go.clone(), handler.clone());
                            let id = view.connect_load_changed(move |v, e| {
                                if e == webkit6::LoadEvent::Finished {
                                    if let Some(id) = h.take() {
                                        v.disconnect(id);
                                    }
                                    if let Some(f) = g.borrow_mut().take() {
                                        f();
                                    }
                                }
                            });
                            handler.set(Some(id));
                            let (g, v) = (go.clone(), view.clone());
                            glib::timeout_add_local_once(std::time::Duration::from_secs(15), move || {
                                if let Some(f) = g.borrow_mut().take() {
                                    if let Some(id) = handler.take() {
                                        v.disconnect(id);
                                    }
                                    f();
                                }
                            });
                        }
                    }
                }
            }
            _ => reply(err("unknown agent command")),
        }
        None
    }

    /// `id`'s view, woken if it was asleep (it isn't put on screen).
    fn agent_view(&self, id: TabId) -> Option<webkit6::WebView> {
        if let Some(v) = self.views.borrow().get(&id).cloned() {
            return Some(v);
        }
        self.wake_for_split(id);
        self.views.borrow().get(&id).cloned()
    }

    fn agent_in_page(&self, verb: &str, view: &webkit6::WebView, selector: &str, text: &str, reply: Box<dyn FnOnce(String)>) {
        if verb == "shot" {
            // WebKit draws only what is on screen: a tab behind has no picture.
            if !self.on_screen().iter().any(|t| self.views.borrow().get(t) == Some(view)) {
                reply(json!({"error": "rippl takes pictures only of a page on screen; this tab is behind the user's"}).to_string());
                return;
            }
            view.snapshot(webkit6::SnapshotRegion::Visible, webkit6::SnapshotOptions::NONE, None::<&gio::Cancellable>, move |r| {
                reply(match r {
                    Ok(t) => json!({"png": glib::base64_encode(&t.save_to_png_bytes()).to_string()}).to_string(),
                    Err(e) => json!({"error": e.to_string()}).to_string(),
                })
            });
            return;
        }
        let body = match verb {
            "read" => "return JSON.stringify({title: document.title, url: location.href, text: document.body ? document.body.innerText : ''});".to_string(),
            "click" => format!("{FIND}{CLICK}"),
            "type" => format!("{FIND}{TYPE}"),
            _ => String::new(),
        };
        let args = glib::VariantDict::new(None);
        args.insert("sel", selector);
        args.insert("text", text);
        view.call_async_javascript_function(&body, Some(&args.end()), Some("rippl"), None, None::<&gio::Cancellable>, move |r| {
            reply(match r {
                Ok(v) => {
                    let s = v.to_str().to_string();
                    if s.starts_with('{') {
                        // A read: the text, trimmed.
                        let mut page: Value = serde_json::from_str(&s).unwrap_or(json!({}));
                        if let Some(t) = page.get("text").and_then(Value::as_str) {
                            let t: String = t.chars().take(READ_MOST).collect();
                            page["text"] = json!(t);
                        }
                        page.to_string()
                    } else if s == "ok" {
                        json!({"ok": true}).to_string()
                    } else {
                        json!({"error": s}).to_string()
                    }
                }
                Err(e) => json!({"error": e.to_string()}).to_string(),
            })
        });
    }

    /// An agent is at work: say so, once a minute at most.
    fn agent_seen(&self) {
        let now = rippl_core::places::now();
        if now - self.agent_said.get() > 60 {
            self.agent_said.set(now);
            self.notice("An agent is using rippl, in tabs marked ✦ (Settings: agents = off stops it)");
        }
    }
}

/// A step held until its page has loaded.
type Pending = std::rc::Rc<std::cell::RefCell<Option<Box<dyn FnOnce()>>>>;

/// The agent's tabs, for the column's mark.
pub(crate) type AgentTabs = std::cell::RefCell<HashSet<TabId>>;
