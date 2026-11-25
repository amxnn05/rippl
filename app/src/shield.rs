//! Shield, the ad and tracker blocker, as the browser runs it (the rule
//! conversion is in `rippl_core::shield`).
//!
//! - **Lists**: EasyList and EasyPrivacy, and uBlock Origin's own lists
//!   (uBlock filters, Privacy, Badware risks, Quick fixes, Unbreak) from
//!   its uAssets repository, the lists that make uBlock Origin what it is.
//!   uBlock itself is a browser extension and WebKitGTK runs none; its
//!   lists are what carry over. `ublock = off` keeps EasyList and
//!   EasyPrivacy alone.
//! - **Two bundles**, each compiled into one WebKit content filter: *ads*
//!   (EasyList, uBlock filters, Quick fixes, Unbreak) and *privacy*
//!   (EasyPrivacy, uBlock Privacy, Badware, Unbreak). One filter per list
//!   wouldn't do: WebKit lets an exception (`@@…`, `ignore-previous-rules`)
//!   cancel only rules of its own filter, and Unbreak is almost all
//!   exceptions to the other lists.
//! - Compiled bundles are kept in WebKit's filter store
//!   (`~/.cache/rippl/shield/compiled`) and load at launch in milliseconds.
//!   On a first launch a small seed list blocks the biggest networks at once.
//! - Each source is fetched again when older than four days (EasyList's and
//!   uBlock's own "Expires"), in the background, with the files it
//!   `!#include`s; a bundle is recompiled when one of its sources changed.
//! - Sites Shield is off for (Ctrl+Shift+B, the site card) are kept in
//!   `~/.config/rippl/shield-allow` and compiled in as exceptions.
//!
//! Fetching lists is the only network request rippl makes on its own. It
//! sends no cookies and a generic user agent.

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

const UASSETS: &str = "https://ublockorigin.github.io/uAssets/filters/";

/// (id, address, from uBlock).
const SOURCES: &[(&str, &str, bool)] = &[
    ("easylist", "https://easylist.to/easylist/easylist.txt", false),
    ("easyprivacy", "https://easylist.to/easylist/easyprivacy.txt", false),
    ("ubo-filters", "https://ublockorigin.github.io/uAssets/filters/filters.txt", true),
    ("ubo-privacy", "https://ublockorigin.github.io/uAssets/filters/privacy.txt", true),
    ("ubo-badware", "https://ublockorigin.github.io/uAssets/filters/badware.txt", true),
    ("ubo-quick-fixes", "https://ublockorigin.github.io/uAssets/filters/quick-fixes.txt", true),
    ("ubo-unbreak", "https://ublockorigin.github.io/uAssets/filters/unbreak.txt", true),
];

/// Each bundle and its sources, compiled together.
const BUNDLES: &[(&str, &[&str])] = &[
    ("ads", &["easylist", "ubo-filters", "ubo-quick-fixes", "ubo-unbreak"]),
    ("privacy", &["easyprivacy", "ubo-privacy", "ubo-badware", "ubo-unbreak"]),
];
const SEED_ID: &str = "seed";
/// EasyList and uBlock say "Expires: 4 days".
const MAX_AGE: Duration = Duration::from_secs(4 * 24 * 60 * 60);

pub struct Shield {
    /// Every tab's views share this manager, so a filter added here
    /// applies to every page at once.
    pub content: UserContentManager,
    store: UserContentFilterStore,
    allowed: RefCell<BTreeSet<String>>,
    /// Bundles whose full filter is in use (the seed goes once all are).
    loaded: RefCell<BTreeSet<&'static str>>,
    busy: Cell<bool>,
    enabled: bool,
    ublock: bool,
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

fn read_stamp(name: &str) -> Option<String> {
    std::fs::read_to_string(dir().join(format!("{name}.stamp"))).ok().map(|s| s.trim().to_string())
}

/// When source `id` was last downloaded.
fn fetched_at(id: &str) -> Option<u64> {
    read_stamp(id)?.split(' ').next_back()?.parse().ok()
}

/// What a bundle's compiled filter was made from: converter version,
/// whether uBlock's lists were in, and its sources' download times.
fn bundle_key(sources: &[&str], ublock: bool) -> String {
    let times: Vec<String> = sources.iter().map(|s| fetched_at(s).unwrap_or(0).to_string()).collect();
    format!("{CONVERTER_VERSION} {} {}", u8::from(ublock), times.join(","))
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
    pub fn new(enabled: bool, ublock: bool) -> Rc<Shield> {
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
            ublock,
        })
    }

    /// The sources of `bundle` in use (uBlock's only with `ublock = on`).
    fn sources_of(&self, bundle: &str) -> Vec<&'static str> {
        let wanted = BUNDLES.iter().find(|(b, _)| *b == bundle).map(|(_, s)| *s).unwrap_or(&[]);
        wanted
            .iter()
            .copied()
            .filter(|id| SOURCES.iter().any(|(s, _, ubo)| s == id && (self.ublock || !ubo)))
            .collect()
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
            // Filters from before bundles (rippl 0.1's) are no longer used.
            for old in ["easylist", "easyprivacy"] {
                let _ = this.store.remove_future(old).await;
            }
            for (bundle, _) in BUNDLES {
                // Whatever was compiled last blocks from the first request,
                // even if it is out of date (other sources, an older
                // converter): `refresh` rebuilds it in the background and
                // swaps it in. Falling back to the seed list meanwhile let
                // most trackers through for the seconds a compile takes.
                match this.store.load_future(bundle).await {
                    Ok(filter) => {
                        this.content.add_filter(&filter);
                        this.loaded.borrow_mut().insert(bundle);
                    }
                    Err(e) => debug!("{bundle}: no compiled filter ({e})"),
                }
            }
            if this.loaded.borrow().len() < BUNDLES.len() {
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

    /// Fetches the sources older than four days (all with `force`), then
    /// recompiles every bundle whose sources changed or whose filter is
    /// missing.
    async fn refresh(&self, force: bool) {
        if self.busy.replace(true) {
            return;
        }
        let mut wanted: Vec<&'static str> = BUNDLES.iter().flat_map(|(b, _)| self.sources_of(b)).collect();
        wanted.dedup();
        wanted.sort();
        wanted.dedup();
        for id in wanted {
            let stale = fetched_at(id).is_none_or(|t| now().saturating_sub(t) > MAX_AGE.as_secs());
            if !(stale || force) {
                continue;
            }
            let Some(url) = SOURCES.iter().find(|(s, _, _)| *s == id).map(|(_, u, _)| *u) else { continue };
            match fetch_list(url).await {
                Ok(text) => {
                    if let Err(e) = std::fs::write(dir().join(format!("{id}.txt")), &text) {
                        eprintln!("rippl: shield: could not keep {id}: {e}");
                        continue;
                    }
                    let _ = std::fs::write(dir().join(format!("{id}.stamp")), format!("{}", now()));
                    debug!("{id}: fetched ({} KB)", text.len() / 1024);
                }
                // Offline, or the server is down: keep what we have and try
                // again at the next look.
                Err(e) => debug!("{id}: fetch failed: {e}"),
            }
        }
        for (bundle, _) in BUNDLES {
            let key = bundle_key(&self.sources_of(bundle), self.ublock);
            if read_stamp(bundle).as_deref() != Some(key.as_str()) || !self.loaded.borrow().contains(bundle) {
                debug!("{bundle}: out of date, rebuilding (the old filter blocks meanwhile)");
                self.compile(bundle).await;
            }
        }
        self.busy.set(false);
    }

    /// Compiles `bundle` from its sources on disk and swaps it in.
    async fn compile(&self, bundle: &'static str) {
        let sources = self.sources_of(bundle);
        let texts: Vec<String> =
            sources.iter().filter_map(|id| std::fs::read_to_string(dir().join(format!("{id}.txt"))).ok()).collect();
        if texts.is_empty() {
            return;
        }
        let allowed = self.allowed_list();
        let started = std::time::Instant::now();
        // Converting is plain Rust: off the main thread.
        let Ok((json, stats)) = gio::spawn_blocking(move || {
            let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
            shield::convert(&refs, &allowed)
        })
        .await
        else {
            return;
        };
        let converted = started.elapsed();
        match self.store.save_future(bundle, &glib::Bytes::from_owned(json.into_bytes())).await {
            Ok(filter) => {
                self.content.remove_filter_by_id(bundle);
                self.content.add_filter(&filter);
                let first = self.loaded.borrow_mut().insert(bundle);
                let _ = std::fs::write(dir().join(format!("{bundle}.stamp")), bundle_key(&sources, self.ublock));
                debug!(
                    "{bundle} ({}): {} rules ({} skipped), converted in {converted:?}, compiled in {:?}",
                    sources.join(" + "),
                    stats.rules(),
                    stats.skipped,
                    started.elapsed() - converted
                );
                if first && self.loaded.borrow().len() == BUNDLES.len() {
                    self.content.remove_filter_by_id(SEED_ID);
                }
            }
            Err(e) => eprintln!("rippl: shield: {bundle} failed to compile: {e}"),
        }
        // Compiling takes a lot of heap for a moment; give it back.
        glib::timeout_add_local_once(Duration::from_secs(2), pressure::trim);
    }

    pub fn is_allowed(&self, host: &str) -> bool {
        self.allowed.borrow().contains(host)
    }

    /// Turns Shield off for `host` (or back on), recompiles every bundle
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
            if this.loaded.borrow().len() < BUNDLES.len() {
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
        let bundles: Vec<String> = BUNDLES
            .iter()
            .map(|(b, _)| {
                let on = if self.loaded.borrow().contains(b) { "on" } else { "pending" };
                let sources: Vec<String> = self
                    .sources_of(b)
                    .iter()
                    .map(|s| {
                        let age = fetched_at(s).map_or("never".into(), |t| format!("{}h", now().saturating_sub(t) / 3600));
                        format!("{s}:{age}")
                    })
                    .collect();
                format!("{b}={on}[{}]", sources.join(" "))
            })
            .collect();
        format!("{} ublock={} allowed={:?}", bundles.join(" "), self.ublock, self.allowed_list())
    }

    pub fn update_now(self: &Rc<Self>) {
        let this = self.clone();
        glib::MainContext::default().spawn_local(async move { this.refresh(true).await });
    }
}

/// A filter list with the files it `!#include`s put in their place (uBlock's
/// lists are mostly includes). Only files beside the list, one level deep.
async fn fetch_list(url: &str) -> Result<String, String> {
    let text = fetch(url).await?;
    let names = shield::includes(&text);
    if names.is_empty() {
        return Ok(text);
    }
    let base = &url[..url.rfind('/').map_or(url.len(), |i| i + 1)];
    if !base.starts_with(UASSETS) && !base.starts_with("https://easylist.to/") {
        return Ok(text);
    }
    let mut out = String::with_capacity(text.len() * 4);
    for line in text.lines() {
        match line.trim().strip_prefix("!#include ").map(str::trim) {
            Some(name) if names.iter().any(|n| n == name) => match fetch(&format!("{base}{name}")).await {
                Ok(part) => {
                    out.push_str(&part);
                    out.push('\n');
                }
                Err(e) => debug!("{name}: include failed: {e}"),
            },
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok(out)
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
