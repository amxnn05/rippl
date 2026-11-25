//! Shield, the ad and tracker blocker, as the browser runs it (the rule
//! conversion is in `rippl_core::shield`).
//!
//! - Two lists, EasyList (ads) and EasyPrivacy (trackers), each compiled
//!   into its own WebKit content filter. One filter per list keeps each
//!   under WebKit's per-list rule limit, and an update to one doesn't
//!   recompile the other.
//! - Compiled filters are kept in WebKit's filter store
//!   (`~/.cache/rippl/shield/compiled`). Loading one at launch takes
//!   milliseconds, so pages never wait for a compile.
//! - On first launch, before any list has been downloaded, a small built-in
//!   seed list of the biggest ad and tracking networks blocks at once.
//! - Lists are fetched again when older than four days (EasyList's own
//!   "Expires"), in the background: the old filter keeps blocking until the
//!   new one is ready, then is swapped in for all tabs.
//! - Sites Shield is turned off for (Ctrl+Shift+B) are kept, one per line,
//!   in `~/.config/rippl/shield-allow`, and compiled into each list as
//!   `ignore-previous-rules` exceptions.
//!
//! Fetching a list is the only network request rippl makes on its own.
//! It sends no cookies and a generic user agent.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rippl_core::shield::{self, CONVERTER_VERSION, SEED};
use webkit6::prelude::*;
use webkit6::{gio, glib, soup, UserContentFilterStore, UserContentManager};

use crate::engine;
use crate::pressure;

macro_rules! debug {
    ($($t:tt)*) => {
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!("rippl shield: {}", format!($($t)*));
        }
    };
}

const LISTS: &[(&str, &str)] = &[
    ("easylist", "https://easylist.to/easylist/easylist.txt"),
    ("easyprivacy", "https://easylist.to/easylist/easyprivacy.txt"),
];
const SEED_ID: &str = "seed";
/// EasyList says "Expires: 4 days".
const MAX_AGE: Duration = Duration::from_secs(4 * 24 * 60 * 60);

pub struct Shield {
    /// Every tab's views share this manager, so a filter added here
    /// applies to every page at once.
    pub content: UserContentManager,
    store: UserContentFilterStore,
    allowed: RefCell<BTreeSet<String>>,
    /// Lists whose full filter is in use (the seed goes once all are).
    loaded: RefCell<BTreeSet<&'static str>>,
    busy: Cell<bool>,
    enabled: bool,
}

fn dir() -> PathBuf {
    engine::shared_cache_dir().join("shield")
}

fn allow_path() -> PathBuf {
    glib::user_config_dir().join("rippl").join("shield-allow")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// When `id` was last downloaded, from its stamp file (`<version> <unix time>`).
fn fetched_at(id: &str) -> Option<u64> {
    let text = std::fs::read_to_string(dir().join(format!("{id}.stamp"))).ok()?;
    let (version, time) = text.trim().split_once(' ')?;
    (version.parse() == Ok(CONVERTER_VERSION)).then(|| time.parse().ok())?
}

/// The host of a URL, lowercased, without `www.`.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (!host.is_empty()).then_some(host)
}

impl Shield {
    pub fn new(enabled: bool) -> Rc<Shield> {
        let _ = std::fs::create_dir_all(dir().join("compiled"));
        let allowed = std::fs::read_to_string(allow_path())
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_ascii_lowercase())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        Rc::new(Shield {
            content: UserContentManager::new(),
            store: UserContentFilterStore::new(&dir().join("compiled").to_string_lossy()),
            allowed: RefCell::new(allowed),
            loaded: RefCell::new(BTreeSet::new()),
            busy: Cell::new(false),
            enabled,
        })
    }

    /// Puts the compiled filters in place, then calls `ready`, at most a
    /// second later, so the first pages load already blocked. Then checks
    /// in the background whether the lists need fetching again.
    pub fn start(self: &Rc<Self>, ready: impl FnOnce() + 'static) {
        if !self.enabled {
            debug!("off (shield = off in config)");
            return ready();
        }
        // Called once: by whichever comes first, the filters or the timeout.
        type Once = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;
        let ready: Once = Rc::new(RefCell::new(Some(Box::new(ready))));
        let fire = {
            let ready = ready.clone();
            move || {
                if let Some(f) = ready.borrow_mut().take() {
                    f();
                }
            }
        };
        glib::timeout_add_local_once(Duration::from_secs(1), fire.clone());

        let this = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let started = std::time::Instant::now();
            for (id, _) in LISTS {
                // A compiled filter from an older converter is rebuilt from
                // the list kept on disk, rather than trusted.
                if fetched_at(id).is_none() {
                    continue;
                }
                match this.store.load_future(id).await {
                    Ok(filter) => {
                        this.content.add_filter(&filter);
                        this.loaded.borrow_mut().insert(id);
                    }
                    Err(e) => debug!("{id}: no compiled filter ({e})"),
                }
            }
            if this.loaded.borrow().len() < LISTS.len() {
                this.add_seed().await;
            }
            debug!("ready in {:?}: {:?}", started.elapsed(), this.loaded.borrow());
            fire();
            this.refresh(false).await;
        });

        // Look again every six hours, for a browser left open for days.
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_secs(6 * 60 * 60), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            glib::MainContext::default().spawn_local(async move { this.refresh(false).await });
            glib::ControlFlow::Continue
        });
    }

    async fn add_seed(&self) {
        let (json, _) = shield::convert(&[SEED], &self.allowed_list());
        match self.store.save_future(SEED_ID, &glib::Bytes::from_owned(json.into_bytes())).await {
            Ok(filter) => {
                self.content.add_filter(&filter);
                debug!("seed list in place");
            }
            Err(e) => eprintln!("rippl: shield seed list failed to compile: {e}"),
        }
    }

    pub(crate) fn allowed_list(&self) -> Vec<String> {
        self.allowed.borrow().iter().cloned().collect()
    }

    /// Fetches the lists older than four days (all of them with `force`)
    /// and recompiles each one that arrived.
    async fn refresh(&self, force: bool) {
        if self.busy.replace(true) {
            return;
        }
        for (id, url) in LISTS {
            let stale = fetched_at(id).is_none_or(|t| now().saturating_sub(t) > MAX_AGE.as_secs());
            if !(stale || force) {
                continue;
            }
            match fetch(url).await {
                Ok(text) => {
                    if let Err(e) = std::fs::write(dir().join(format!("{id}.txt")), &text) {
                        eprintln!("rippl: shield: could not keep {id}: {e}");
                        continue;
                    }
                    self.compile(id).await;
                }
                // Offline, or the server is down: keep the filter we have
                // and try again at the next look.
                Err(e) => debug!("{id}: fetch failed: {e}"),
            }
        }
        self.busy.set(false);
    }

    /// Compiles list `id` from the copy on disk and swaps it in.
    async fn compile(&self, id: &'static str) {
        let Ok(text) = std::fs::read_to_string(dir().join(format!("{id}.txt"))) else { return };
        let allowed = self.allowed_list();
        let started = std::time::Instant::now();
        // Converting is plain Rust: off the main thread.
        let Ok((json, stats)) = gio::spawn_blocking(move || shield::convert(&[&text], &allowed)).await else {
            return;
        };
        let converted = started.elapsed();
        match self.store.save_future(id, &glib::Bytes::from_owned(json.into_bytes())).await {
            Ok(filter) => {
                self.content.remove_filter_by_id(id);
                self.content.add_filter(&filter);
                let first = self.loaded.borrow_mut().insert(id);
                let _ = std::fs::write(dir().join(format!("{id}.stamp")), format!("{CONVERTER_VERSION} {}", now()));
                debug!(
                    "{id}: {} rules ({} skipped), converted in {converted:?}, compiled in {:?}",
                    stats.rules(),
                    stats.skipped,
                    started.elapsed() - converted
                );
                if first && self.loaded.borrow().len() == LISTS.len() {
                    self.content.remove_filter_by_id(SEED_ID);
                }
            }
            Err(e) => eprintln!("rippl: shield: {id} failed to compile: {e}"),
        }
        // Compiling a list takes a lot of heap for a moment; give it back.
        glib::timeout_add_local_once(Duration::from_secs(2), pressure::trim);
    }

    pub fn is_allowed(&self, host: &str) -> bool {
        self.allowed.borrow().contains(host)
    }

    /// Turns Shield off for `host` (or back on), recompiles every list
    /// with the new exceptions, then calls `done`.
    pub fn toggle(self: &Rc<Self>, host: &str, done: impl FnOnce(bool) + 'static) {
        let now_allowed = {
            let mut allowed = self.allowed.borrow_mut();
            if !allowed.remove(host) {
                allowed.insert(host.to_string());
            }
            allowed.contains(host)
        };
        let text: String = self.allowed.borrow().iter().map(|h| format!("{h}\n")).collect();
        if let Some(parent) = allow_path().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(allow_path(), format!("# Sites rippl's Shield is off for, one per line.\n{text}"));
        let this = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let ids: Vec<&'static str> = this.loaded.borrow().iter().copied().collect();
            for id in ids {
                this.compile(id).await;
            }
            if this.loaded.borrow().len() < LISTS.len() {
                this.content.remove_filter_by_id(SEED_ID);
                this.add_seed().await;
            }
            done(now_allowed);
        });
    }

    /// One line for the control socket.
    pub fn status(&self) -> String {
        if !self.enabled {
            return "off".into();
        }
        let lists: Vec<String> = LISTS
            .iter()
            .map(|(id, _)| {
                let age = fetched_at(id).map_or("never".to_string(), |t| format!("{}h", now().saturating_sub(t) / 3600));
                let on = if self.loaded.borrow().contains(id) { "on" } else { "pending" };
                format!("{id}={on}(fetched {age} ago)")
            })
            .collect();
        format!("{} allowed={:?}", lists.join(" "), self.allowed_list())
    }

    pub fn update_now(self: &Rc<Self>) {
        let this = self.clone();
        glib::MainContext::default().spawn_local(async move { this.refresh(true).await });
    }
}

/// GETs `url` as text, for anything that isn't a filter list (engine
/// suggestions): the same private session rules.
pub async fn fetch_text(url: &str) -> Result<String, String> {
    let session = soup::Session::new();
    session.set_user_agent("rippl");
    session.set_timeout(5);
    let message = soup::Message::new("GET", url).map_err(|e| e.to_string())?;
    let bytes = session.send_and_read_future(&message, glib::Priority::DEFAULT).await.map_err(|e| e.to_string())?;
    if message.status() != soup::Status::Ok {
        return Err(format!("HTTP {:?}", message.status()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// GETs `url` as text. A session of its own: no cookies, no cache shared
/// with pages, a generic user agent, a 60 s timeout.
async fn fetch(url: &str) -> Result<String, String> {
    let session = soup::Session::new();
    session.set_user_agent("rippl");
    session.set_timeout(60);
    let message = soup::Message::new("GET", url).map_err(|e| e.to_string())?;
    let bytes = session
        .send_and_read_future(&message, glib::Priority::LOW)
        .await
        .map_err(|e| e.to_string())?;
    if message.status() != soup::Status::Ok {
        return Err(format!("HTTP {:?}", message.status()));
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // A captive portal's page is not a filter list.
    if !text.starts_with("[Adblock") && !text.starts_with('!') {
        return Err("not a filter list".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::host_of;

    #[test]
    fn hosts() {
        assert_eq!(host_of("https://www.Example.com:8080/a?b").as_deref(), Some("example.com"));
        assert_eq!(host_of("http://user@site.org/").as_deref(), Some("site.org"));
        assert_eq!(host_of("about:blank"), None);
    }
}
