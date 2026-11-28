//! Shield on macOS and Windows: the same lists as the GTK build
//! (EasyList, EasyPrivacy and uBlock Origin's own), fetched again when
//! older than four days, and the same allow list (`shield-allow`).
//!
//! - **macOS**: WebKit's content rule lists, compiled from the very JSON the
//!   GTK build gives WebKitGTK (`rippl_core::shield::convert`), and kept in
//!   WebKit's store. WebKit enforces them; no script runs in pages.
//! - **Windows**: WebView2 has no content rules. Every request a page makes
//!   is checked by Brave's filter engine (the `adblock` crate) from the raw
//!   lists, and a blocked one is answered with an empty 403.
//! - **Linux** (developing this build): WebKitGTK's content filters, as the
//!   GTK build.
//!
//! A site Shield is off for gets no rules in its views; switching it takes
//! effect in its open tabs at once.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use rippl_core::shield as rules;
use wry::WebView;

use crate::browser::{Ev, Proxy, post};

const SOURCES: &[(&str, &str, bool)] = &[
    ("easylist", "https://easylist.to/easylist/easylist.txt", false),
    ("easyprivacy", "https://easylist.to/easylist/easyprivacy.txt", false),
    ("ubo-filters", "https://ublockorigin.github.io/uAssets/filters/filters.txt", true),
    ("ubo-privacy", "https://ublockorigin.github.io/uAssets/filters/privacy.txt", true),
    ("ubo-badware", "https://ublockorigin.github.io/uAssets/filters/badware.txt", true),
    ("ubo-quick-fixes", "https://ublockorigin.github.io/uAssets/filters/quick-fixes.txt", true),
    ("ubo-unbreak", "https://ublockorigin.github.io/uAssets/filters/unbreak.txt", true),
];
const BUNDLES: &[(&str, &[&str])] = &[
    ("ads", &["easylist", "ubo-filters", "ubo-quick-fixes", "ubo-unbreak"]),
    ("privacy", &["easyprivacy", "ubo-privacy", "ubo-badware", "ubo-unbreak"]),
];
const MAX_AGE: Duration = Duration::from_secs(4 * 24 * 60 * 60);

/// What the lists thread hands back: each bundle's text, ready for this
/// system's engine.
pub struct Lists {
    /// (bundle, WebKit's JSON) on macOS and Linux; (bundle, raw ABP text)
    /// on Windows.
    pub bundles: Vec<(String, String)>,
    #[cfg(windows)]
    pub engine: Option<adblock::Engine>,
}

pub fn dir() -> PathBuf {
    rippl_core::dirs::cache().join("shield")
}

fn allow_path() -> PathBuf {
    rippl_core::dirs::config().join("shield-allow")
}

/// The host of a URL, lowercased, without `www.`.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (!host.is_empty()).then_some(host)
}

fn age(path: &std::path::Path) -> Option<Duration> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(m).ok()
}

/// A list with the files it `!#include`s put in their place.
fn fetch_list(url: &str) -> Option<String> {
    let text = crate::net::get_text(url, 32 * 1024 * 1024)?;
    let names = rules::includes(&text);
    if names.is_empty() {
        return Some(text);
    }
    let base = &url[..url.rfind('/').map_or(url.len(), |i| i + 1)];
    let mut out = String::with_capacity(text.len() * 4);
    for line in text.lines() {
        match line.trim().strip_prefix("!#include ").map(str::trim) {
            Some(name) if names.iter().any(|n| n == name) => {
                if let Some(part) = crate::net::get_text(&format!("{base}{name}"), 32 * 1024 * 1024) {
                    out.push_str(&part);
                    out.push('\n');
                }
            }
            _ => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Some(out)
}

/// Each source: from the cache, fetched first when missing or old.
fn sources(ublock: bool) -> Vec<(&'static str, String)> {
    let _ = std::fs::create_dir_all(dir());
    let mut out = Vec::new();
    for (id, url, from_ublock) in SOURCES {
        if *from_ublock && !ublock {
            continue;
        }
        let path = dir().join(format!("{id}.txt"));
        if age(&path).is_none_or(|a| a > MAX_AGE)
            && let Some(text) = fetch_list(url)
        {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, &text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            out.push((*id, rules::preprocess(&text)));
        }
    }
    out
}

/// The lists, made ready on a thread of their own.
fn build(ublock: bool) -> Lists {
    let src = sources(ublock);
    let mut bundles = Vec::new();
    for (name, ids) in BUNDLES {
        let texts: Vec<&str> = src.iter().filter(|(id, _)| ids.contains(id)).map(|(_, t)| t.as_str()).collect();
        if texts.is_empty() {
            continue;
        }
        #[cfg(windows)]
        bundles.push((name.to_string(), texts.join("\n")));
        #[cfg(not(windows))]
        {
            let (json, _stats) = rules::convert(&texts, &[]);
            bundles.push((name.to_string(), json));
        }
    }
    if bundles.is_empty() {
        // No network yet: the seed blocks the biggest networks.
        #[cfg(windows)]
        bundles.push(("seed".into(), rules::SEED.to_string()));
        #[cfg(not(windows))]
        bundles.push(("seed".into(), rules::convert(&[rules::SEED], &[]).0));
    }
    #[cfg(windows)]
    {
        let mut set = adblock::lists::FilterSet::new(false);
        for (_, text) in &bundles {
            set.add_filter_list(text.clone(), adblock::lists::ParseOptions::default());
        }
        let engine = adblock::Engine::new_with_filter_set(set);
        return Lists { bundles: Vec::new(), engine: Some(engine) };
    }
    #[cfg(not(windows))]
    Lists { bundles }
}

pub struct Shield {
    pub enabled: bool,
    allowed: RefCell<BTreeSet<String>>,
    #[cfg(target_os = "macos")]
    mac: mac::Rules,
    #[cfg(windows)]
    pub win: std::rc::Rc<win::Rules>,
    #[cfg(all(unix, not(target_os = "macos")))]
    gtk: gtk_rules::Rules,
}

impl Shield {
    pub fn new(enabled: bool, ublock: bool, proxy: Proxy) -> Shield {
        let allowed = std::fs::read_to_string(allow_path())
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_ascii_lowercase())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        if enabled {
            std::thread::spawn(move || post(&proxy, Ev::ShieldLists(build(ublock))));
        }
        Shield {
            enabled,
            allowed: RefCell::new(allowed),
            #[cfg(target_os = "macos")]
            mac: mac::Rules::default(),
            #[cfg(windows)]
            win: std::rc::Rc::new(win::Rules::default()),
            #[cfg(all(unix, not(target_os = "macos")))]
            gtk: gtk_rules::Rules::default(),
        }
    }

    pub fn is_allowed(&self, host: &str) -> bool {
        self.allowed.borrow().contains(host)
    }

    pub fn allowed_list(&self) -> Vec<String> {
        self.allowed.borrow().iter().cloned().collect()
    }

    /// Shield off (true) or on for a site; saved.
    pub fn set_allowed(&self, host: &str, off: bool) {
        if off {
            self.allowed.borrow_mut().insert(host.to_string());
        } else {
            self.allowed.borrow_mut().remove(host);
        }
        let text: String = self.allowed.borrow().iter().map(|h| format!("{h}\n")).collect();
        let _ = std::fs::create_dir_all(rippl_core::dirs::config());
        let _ = std::fs::write(allow_path(), text);
    }

    fn wanted(&self, url: &str) -> bool {
        self.enabled && !host_of(url).is_some_and(|h| self.is_allowed(&h))
    }

    /// A view gets the rules, unless Shield is off for its site.
    pub fn attach(&self, view: &WebView, url: &str) {
        let on = self.wanted(url);
        #[cfg(target_os = "macos")]
        self.mac.apply(view, on);
        #[cfg(windows)]
        win::watch(view, &self.win, on);
        #[cfg(all(unix, not(target_os = "macos")))]
        self.gtk.apply(view, on);
    }

    /// The view's page moved (or its site's Shield changed): rules follow.
    pub fn follow(&self, view: &WebView, url: &str) {
        let on = self.wanted(url);
        #[cfg(target_os = "macos")]
        self.mac.apply(view, on);
        #[cfg(windows)]
        win::set_on(view, on);
        #[cfg(all(unix, not(target_os = "macos")))]
        self.gtk.apply(view, on);
    }

    /// The lists are ready: compile (macOS, Linux) or take the engine
    /// (Windows), then every open view gets them.
    pub fn lists_ready(&self, lists: Lists, proxy: Proxy) {
        #[cfg(target_os = "macos")]
        self.mac.compile(lists, proxy);
        #[cfg(windows)]
        {
            *self.win.engine.borrow_mut() = lists.engine;
            let _ = proxy;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        self.gtk.compile(lists, proxy);
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::rc::Rc;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_foundation::{NSError, NSString, NSURL};
    use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore};
    use wry::{WebView, WebViewExtMacOS};

    use super::Lists;
    use crate::browser::{Ev, Proxy, post};

    #[derive(Default)]
    pub struct Rules {
        pub lists: Rc<RefCell<Vec<Retained<WKContentRuleList>>>>,
    }

    impl Rules {
        pub fn apply(&self, view: &WebView, on: bool) {
            let manager = view.manager();
            unsafe {
                manager.removeAllContentRuleLists();
                if on {
                    for l in self.lists.borrow().iter() {
                        manager.addContentRuleList(l);
                    }
                }
            }
        }

        pub fn compile(&self, lists: Lists, proxy: Proxy) {
            let path = super::dir().join("compiled");
            let _ = std::fs::create_dir_all(&path);
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            let Some(mtm) = objc2::MainThreadMarker::new() else { return };
            let Some(store) = (unsafe { WKContentRuleListStore::storeWithURL(Some(&url), mtm) }) else { return };
            let total = lists.bundles.len();
            let done = Rc::new(RefCell::new(Vec::<Retained<WKContentRuleList>>::new()));
            let answered = Rc::new(std::cell::Cell::new(0usize));
            for (name, json) in lists.bundles {
                let done = done.clone();
                let answered = answered.clone();
                let target = self.lists.clone();
                let proxy = proxy.clone();
                let block = RcBlock::new(move |list: *mut WKContentRuleList, _err: *mut NSError| {
                    match unsafe { Retained::retain(list) } {
                        Some(list) => done.borrow_mut().push(list),
                        None => eprintln!("rippl: Shield: a bundle didn't compile"),
                    }
                    answered.set(answered.get() + 1);
                    if answered.get() == total {
                        *target.borrow_mut() = done.borrow().clone();
                        post(&proxy, Ev::ShieldApplied);
                    }
                });
                unsafe {
                    store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                        Some(&NSString::from_str(&format!("rippl-{name}"))),
                        Some(&NSString::from_str(&json)),
                        Some(&block),
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
pub mod win {
    use std::cell::RefCell;
    use std::rc::Rc;

    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::WebResourceRequestedEventHandler;
    use windows::core::{HSTRING, PWSTR};
    use wry::{WebView, WebViewExtWindows};

    #[derive(Default)]
    pub struct Rules {
        pub engine: RefCell<Option<adblock::Engine>>,
    }

    fn take(p: PWSTR) -> String {
        if p.is_null() {
            return String::new();
        }
        let s = unsafe { p.to_string() }.unwrap_or_default();
        unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _)) };
        s
    }

    fn kind(c: COREWEBVIEW2_WEB_RESOURCE_CONTEXT) -> &'static str {
        match c {
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT => "document",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET => "stylesheet",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE => "image",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA => "media",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT => "font",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT => "script",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH => "xmlhttprequest",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_WEBSOCKET => "websocket",
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_PING => "ping",
            _ => "other",
        }
    }

    pub fn set_on(view: &WebView, on: bool) {
        let _ = (view, on);
        OFF.with(|o| {
            let h = view.hwnd().0 as isize;
            let mut o = o.borrow_mut();
            o.retain(|x| *x != h);
            if !on {
                o.push(h);
            }
        });
    }

    thread_local! {
        static OFF: RefCell<Vec<isize>> = const { RefCell::new(Vec::new()) };
    }

    /// Every request of this view goes past the engine.
    pub fn watch(view: &WebView, rules: &Rc<Rules>, on: bool) {
        set_on(view, on);
        let core = view.webview();
        let env = view.environment();
        let hwnd = view.hwnd().0 as isize;
        let rules = rules.clone();
        unsafe {
            let _ = core.AddWebResourceRequestedFilter(&HSTRING::from("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);
            let mut token = Default::default();
            let handler = WebResourceRequestedEventHandler::create(Box::new(move |sender: Option<ICoreWebView2>, args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                let Some(args) = args else { return Ok(()) };
                if OFF.with(|o| o.borrow().contains(&hwnd)) {
                    return Ok(());
                }
                let engine = rules.engine.borrow();
                let Some(engine) = engine.as_ref() else { return Ok(()) };
                let request = args.Request()?;
                let mut uri = PWSTR::null();
                request.Uri(&mut uri)?;
                let url = take(uri);
                if !url.starts_with("http") {
                    return Ok(());
                }
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                args.ResourceContext(&mut context)?;
                if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT {
                    return Ok(());
                }
                let mut source = PWSTR::null();
                if let Some(s) = sender {
                    s.Source(&mut source)?;
                }
                let source = take(source);
                if let Ok(req) = adblock::request::Request::new(&url, &source, kind(context), "GET") {
                    if engine.check_network_request(&req).should_block() {
                        let response = env.CreateWebResourceResponse(None, 403, &HSTRING::from("Blocked by Shield"), &HSTRING::from(""))?;
                        args.SetResponse(&response)?;
                    }
                }
                Ok(())
            }));
            let _ = core.add_WebResourceRequested(&handler, &mut token);
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod gtk_rules {
    //! wry's WebKitGTK bindings don't carry content filters; the Linux
    //! product is the GTK build, so this development build goes unshielded.
    use wry::WebView;

    use super::Lists;
    use crate::browser::Proxy;

    #[derive(Default)]
    pub struct Rules;

    impl Rules {
        pub fn apply(&self, _view: &WebView, _on: bool) {}
        pub fn compile(&self, _lists: Lists, _proxy: Proxy) {}
    }
}
