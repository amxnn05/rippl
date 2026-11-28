//! One window: the column (a view of rippl's own page, `ui/chrome.html`),
//! and one engine view per awake tab, laid exactly over the column's page
//! card. The tab model, the session, history and every decision come from
//! `rippl_core`, as in the GTK build.
//!
//! Signal safety, as in the GTK build: every engine handler only posts an
//! `Ev` to the event loop, which hands it here when nothing is borrowed.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rippl_core::address::{self, Destination};
use rippl_core::config::{Config, Theme};
use rippl_core::keys::Key;
use rippl_core::places::{Places, now};
use rippl_core::session::Session;
use rippl_core::tabs::{Life, TabId, Tabs};
use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

use crate::commands;
use crate::downloads::Downloads;
use crate::platform::{self, Host};
use crate::shield::Shield;

/// What the engine and the column tell rippl, in the order they happened.
pub enum Ev {
    Chrome(String),
    Page(TabId, String),
    Title(TabId, String),
    Load(TabId, bool, String),
    Navigate(TabId, String),
    Popup(TabId, String),
    Serve(String, wry::RequestAsyncResponder),
    DownloadStarted(String, PathBuf),
    DownloadDone(String, Option<PathBuf>, bool),
    Suggest(String, Vec<String>),
    ShieldLists(crate::shield::Lists),
    ShieldApplied,
    Menu(String),
    Control(String, std::sync::mpsc::Sender<String>),
    Grab(u64, crate::grab::GrabEv),
    Tick,
}

impl std::fmt::Debug for Ev {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Ev::Chrome(m) => return write!(f, "Chrome({})", m.chars().take(30).collect::<String>()),
            Ev::Page(..) => "Page",
            Ev::Title(..) => "Title",
            Ev::Load(..) => "Load",
            Ev::Navigate(..) => "Navigate",
            Ev::Popup(..) => "Popup",
            Ev::Serve(..) => "Serve",
            Ev::DownloadStarted(..) => "DownloadStarted",
            Ev::DownloadDone(..) => "DownloadDone",
            Ev::Suggest(..) => "Suggest",
            Ev::ShieldLists(..) => "ShieldLists",
            Ev::ShieldApplied => "ShieldApplied",
            Ev::Menu(..) => "Menu",
            Ev::Control(..) => "Control",
            Ev::Grab(..) => "Grab",
            Ev::Tick => "Tick",
        };
        f.write_str(name)
    }
}

/// The proxy, shareable with the engine's threads.
pub type Proxy = std::sync::Arc<std::sync::Mutex<EventLoopProxy<Ev>>>;

pub fn post(proxy: &Proxy, ev: Ev) {
    if let Ok(p) = proxy.lock() {
        let _ = p.send_event(ev);
    }
}

/// What a tab has that the model doesn't keep.
#[derive(Default, Clone)]
pub struct Extra {
    pub icon: String,
    pub can_back: bool,
    pub can_forward: bool,
    pub zoom: f64,
    pub progress: f64,
    pub hover: String,
}

pub struct Browser {
    pub window: Window,
    pub host: Host,
    pub proxy: Proxy,
    pub context: RefCell<WebContext>,
    pub chrome: RefCell<Option<WebView>>,
    pub views: RefCell<HashMap<TabId, WebView>>,
    pub tabs: RefCell<Tabs>,
    pub extra: RefCell<HashMap<TabId, Extra>>,
    pub config: RefCell<Config>,
    pub places: Places,
    pub token: String,
    pub keymap: RefCell<Vec<(String, Key)>>,
    pub page_rect: Cell<(f64, f64, f64, f64)>,
    pub render_due: Cell<bool>,
    pub dirty: Cell<bool>,
    pub closed: RefCell<Vec<(String, String)>>,
    pub notice: RefCell<Option<(String, Instant)>>,
    pub find: RefCell<Option<(String, String)>>,
    pub bar: RefCell<Option<Value>>,
    pub field_focused: Cell<bool>,
    pub downloads: Downloads,
    pub shield: Shield,
    pub column_hidden: Cell<bool>,
    pub dark: Cell<bool>,
    pub menu_actions: RefCell<HashMap<String, Box<dyn Fn(&Browser)>>>,
    pub last_tick: Cell<Instant>,
    pub ready: Cell<bool>,
    pub pending: RefCell<Vec<Box<dyn FnOnce(&Browser)>>>,
    pub window_theme: Cell<Option<Option<tao::window::Theme>>>,
    pub scheme_set: Cell<bool>,
    pub children: std::sync::Arc<std::sync::Mutex<Vec<(u64, std::process::Child)>>>,
}

fn session_path() -> PathBuf {
    rippl_core::dirs::data().join("session")
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { position: LogicalPosition::new(x, y).into(), size: LogicalSize::new(w.max(1.0), h.max(1.0)).into() }
}

/// The rippl-ui:// files: the column, built into rippl.
fn ui_file(path: &str) -> Option<(&'static [u8], &'static str)> {
    Some(match path.trim_start_matches('/') {
        "chrome.html" | "" => (include_bytes!("../ui/chrome.html"), "text/html"),
        "chrome.css" => (include_bytes!("../ui/chrome.css"), "text/css"),
        "chrome.js" => (include_bytes!("../ui/chrome.js"), "text/javascript"),
        _ => return None,
    })
}

fn respond(responder: wry::RequestAsyncResponder, body: Vec<u8>, mime: &str) {
    let response = wry::http::Response::builder()
        .header("Content-Type", mime)
        .header("Cache-Control", "no-store")
        .body(std::borrow::Cow::Owned(body))
        .unwrap_or_default();
    responder.respond(response);
}

impl Browser {
    pub fn new(window: Window, proxy: Proxy, urls: Vec<String>) -> Rc<Browser> {
        let host = platform::host(&window);
        let data = rippl_core::dirs::data();
        let first_run = !data.join("places.sqlite").exists();
        let _ = std::fs::create_dir_all(&data);
        let config = Config::load();
        let places = Places::open(Some(&data.join("places.sqlite"))).unwrap_or_else(|e| {
            eprintln!("rippl: places.sqlite: {e}; history kept in memory this time");
            Places::open(None).expect("an in-memory database")
        });
        let keymap = commands::keymap(&config.shortcuts);
        let shield = Shield::new(config.shield, config.ublock, proxy.clone());
        let downloads = Downloads::new(config.history);
        let b = Rc::new(Browser {
            window,
            host,
            proxy,
            context: RefCell::new(WebContext::new(Some(data.join("web")))),
            chrome: RefCell::new(None),
            views: RefCell::new(HashMap::new()),
            tabs: RefCell::new(Tabs::new()),
            extra: RefCell::new(HashMap::new()),
            config: RefCell::new(config),
            places,
            token: new_token(),
            keymap: RefCell::new(keymap),
            page_rect: Cell::new((0.0, 0.0, 0.0, 0.0)),
            render_due: Cell::new(true),
            dirty: Cell::new(false),
            closed: RefCell::new(Vec::new()),
            notice: RefCell::new(None),
            find: RefCell::new(None),
            bar: RefCell::new(None),
            field_focused: Cell::new(false),
            downloads,
            shield,
            column_hidden: Cell::new(false),
            dark: Cell::new(false),
            menu_actions: RefCell::new(HashMap::new()),
            last_tick: Cell::new(Instant::now()),
            ready: Cell::new(false),
            pending: RefCell::new(Vec::new()),
            window_theme: Cell::new(None),
            scheme_set: Cell::new(false),
            children: Default::default(),
        });
        b.build_chrome();
        b.restore(urls, first_run);
        b.apply_theme();
        b
    }

    // ---- the column -----------------------------------------------------

    fn build_chrome(&self) {
        let proxy = self.proxy.clone();
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        let (r, g, bl) = if self.prefers_dark() { (19, 19, 19) } else { (244, 244, 244) };
        let builder = WebViewBuilder::new()
            .with_id("chrome")
            .with_url(platform::to_engine("rippl-ui://chrome/chrome.html"))
            .with_background_color((r, g, bl, 255))
            .with_bounds(rect(0.0, 0.0, size.width, size.height))
            .with_devtools(cfg!(debug_assertions))
            .with_asynchronous_custom_protocol("rippl-ui".into(), |_id, request, responder| {
                match ui_file(request.uri().path()) {
                    Some((body, mime)) => respond(responder, body.to_vec(), mime),
                    None => respond(responder, b"not here".to_vec(), "text/plain"),
                }
            })
            .with_ipc_handler(move |req| post(&proxy, Ev::Chrome(req.body().clone())));
        match platform::build(builder, &self.window, &self.host) {
            Ok(view) => *self.chrome.borrow_mut() = Some(view),
            Err(e) => {
                eprintln!("rippl: the column's view: {e}");
                std::process::exit(1);
            }
        }
    }

    pub fn resized(&self) {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        if let Some(c) = self.chrome.borrow().as_ref() {
            let _ = c.set_bounds(rect(0.0, 0.0, size.width, size.height));
        }
        self.place_views();
    }

    fn eval_chrome(&self, js: &str) {
        if let Some(c) = self.chrome.borrow().as_ref() {
            let _ = c.evaluate_script(js);
        }
    }

    pub fn prefers_dark(&self) -> bool {
        match self.config.borrow().theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => self.window.theme() == tao::window::Theme::Dark,
        }
    }

    /// The theme setting: the window told (it decides pages'
    /// `prefers-color-scheme`), and the column follows.
    pub fn apply_theme(&self) {
        let theme = match self.config.borrow().theme {
            Theme::Light => Some(tao::window::Theme::Light),
            Theme::Dark => Some(tao::window::Theme::Dark),
            Theme::System => None,
        };
        if self.window_theme.get() != Some(theme) {
            self.window_theme.set(Some(theme));
            self.window.set_theme(theme);
        }
        self.theme_changed();
    }

    /// The system's light or dark changed (or the window's did).
    pub fn theme_changed(&self) {
        let dark = self.prefers_dark();
        if dark != self.dark.get() || !self.ready.get() {
            self.dark.set(dark);
            self.render_due.set(true);
        }
    }

    /// Everything the column shows, as one message.
    pub fn render(&self) {
        if !self.ready.get() {
            return;
        }
        let state = self.state();
        self.eval_chrome(&format!("rippl.update({state})"));
        self.render_due.set(false);
    }

    fn state(&self) -> Value {
        let tabs = self.tabs.borrow();
        let extra = self.extra.borrow();
        let config = self.config.borrow();
        let front = tabs.front;
        let list: Vec<Value> = tabs
            .here()
            .map(|t| {
                let e = extra.get(&t.id).cloned().unwrap_or_default();
                json!({
                    "id": t.id, "title": if t.title.is_empty() { address::pretty(&t.url) } else { t.title.clone() },
                    "url": t.url, "pretty": address::pretty(&t.url), "pinned": t.pinned, "front": Some(t.id) == front,
                    "asleep": t.life != Life::Awake && Some(t.id) != front, "loading": t.loading, "private": t.private,
                    "group": t.group, "letter": address::monogram(&t.url, &t.title), "icon": e.icon,
                    "unread": rippl_core::tabs::unread(&t.title),
                })
            })
            .collect();
        let groups: Vec<Value> = tabs
            .groups
            .iter()
            .filter(|g| g.space == tabs.space)
            .map(|g| json!({"id": g.id, "name": g.name, "collapsed": g.collapsed, "site": g.site}))
            .collect();
        let spaces: Vec<Value> = tabs
            .spaces
            .iter()
            .map(|s| json!({"id": s.id, "name": s.name, "here": s.id == tabs.space, "colour": s.colours.first()}))
            .collect();
        let front_tab = front.and_then(|f| tabs.get(f));
        let url = front_tab.map(|t| t.url.clone()).unwrap_or_default();
        let e = front.and_then(|f| extra.get(&f).cloned()).unwrap_or_default();
        let shown = platform::from_engine(&url);
        let field = json!({
            "url": shown, "pretty": if shown.starts_with("rippl://") { shown.clone() } else { address::pretty(&shown) },
            "starred": !url.is_empty() && self.places.is_bookmarked(&url).unwrap_or(false),
            "shield_off": crate::shield::host_of(&url).is_some_and(|h| self.shield.is_allowed(&h)),
            "can_back": e.can_back, "can_forward": e.can_forward,
            "loading": front_tab.is_some_and(|t| t.loading), "progress": e.progress,
        });
        let home = front_tab.is_none_or(|t| t.url.is_empty());
        let home_grid: Vec<Value> = if home && config.home_grid {
            self.places
                .bookmarks()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.folder.eq_ignore_ascii_case("home"))
                .take(12)
                .map(|p| json!({"url": p.url, "title": if p.title.is_empty() { address::pretty(&p.url) } else { p.title.clone() }, "letter": address::monogram(&p.url, &p.title)}))
                .collect()
        } else {
            Vec::new()
        };
        let bookmarks_bar: Option<Vec<Value>> = config.bookmarks_bar.then(|| {
            self.places
                .bookmarks()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| p.folder.is_empty() || p.folder.eq_ignore_ascii_case("bar"))
                .take(30)
                .map(|p| json!({"url": p.url, "title": p.title}))
                .collect()
        });
        let notice = self.notice.borrow().as_ref().map(|(n, _)| n.clone());
        let find = self.find.borrow().as_ref().map(|(_, count)| json!({"count": count}));
        let keys = commands::keymap_json(&self.keymap.borrow());
        let column_width = config_column_width();
        json!({
            "platform": platform::NAME,
            "theme": if self.dark.get() { "dark" } else { "light" },
            "tabs": list, "groups": groups, "spaces": spaces, "field": field, "home": home,
            "home_grid": home_grid, "home_look": crate::home::look(&config),
            "bookmarks_bar": bookmarks_bar, "notice": notice, "find": find, "bar": *self.bar.borrow(),
            "downloads": self.downloads.column(), "keys": keys, "icons": config.icons,
            "top_tabs": config.top_tabs, "icon_column": config.icon_column, "column_hidden": self.column_hidden.get(),
            "motion": config.motion_full, "column_width": column_width,
            "pin_folders": tabs.pin_folders(tabs.space),
        })
    }

    pub fn notice(&self, text: &str) {
        *self.notice.borrow_mut() = if text.is_empty() { None } else { Some((text.to_string(), Instant::now())) };
        self.render_due.set(true);
    }

    // ---- tabs and their views --------------------------------------------

    fn restore(&self, urls: Vec<String>, first_run: bool) {
        let text = std::fs::read_to_string(session_path()).unwrap_or_default();
        let session = Session::read(&text);
        let front = {
            let mut tabs = self.tabs.borrow_mut();
            let (front, _splits) = session.restore_all(&mut tabs);
            front
        };
        let mut chosen = front;
        for url in urls {
            chosen = Some(self.tabs.borrow_mut().open(url));
        }
        if first_run {
            chosen = Some(self.tabs.borrow_mut().open("rippl://welcome"));
        }
        match chosen {
            Some(id) => self.select(id),
            None => {
                let id = self.tabs.borrow_mut().open("");
                self.select(id);
            }
        }
    }

    pub fn front(&self) -> Option<TabId> {
        self.tabs.borrow().front
    }

    /// A new tab, after the front one; in front unless `background`.
    pub fn open(&self, url: &str, private: bool, background: bool) -> TabId {
        let id = {
            let mut tabs = self.tabs.borrow_mut();
            let id = tabs.open(url);
            if let Some(t) = tabs.get_mut(id) {
                t.private = private;
            }
            id
        };
        if background {
            if !url.is_empty() {
                self.wake(id);
            }
        } else {
            self.select(id);
        }
        self.dirty.set(true);
        self.render_due.set(true);
        id
    }

    pub fn select(&self, id: TabId) {
        self.tabs.borrow_mut().select(id);
        let url = self.tabs.borrow().get(id).map(|t| t.url.clone()).unwrap_or_default();
        if !url.is_empty() && !self.views.borrow().contains_key(&id) {
            self.wake(id);
        }
        self.close_find();
        self.place_views();
        let title = self.tabs.borrow().get(id).map(|t| t.title.clone()).unwrap_or_default();
        self.window.set_title(if title.is_empty() { "rippl" } else { &title });
        if url.is_empty() {
            self.eval_chrome("rippl.focusField && setTimeout(() => rippl.focusField(), 30)");
        } else if !self.field_focused.get()
            && let Some(v) = self.views.borrow().get(&id)
        {
            let _ = v.focus();
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    /// The view for a tab: made now, at its address.
    pub fn wake(&self, id: TabId) {
        let (url, private) = match self.tabs.borrow().get(id) {
            Some(t) => (t.url.clone(), t.private),
            None => return,
        };
        if url.is_empty() || self.views.borrow().contains_key(&id) {
            return;
        }
        match self.make_view(id, &url, private) {
            Some(view) => {
                self.shield.attach(&view, &url);
                self.views.borrow_mut().insert(id, view);
                if let Some(t) = self.tabs.borrow_mut().get_mut(id) {
                    t.life = Life::Awake;
                    t.touched = Instant::now();
                }
            }
            None => self.notice("That page couldn't be opened."),
        }
    }

    fn make_view(&self, id: TabId, url: &str, private: bool) -> Option<WebView> {
        let proxy = self.proxy.clone();
        let p_ipc = proxy.clone();
        let p_title = proxy.clone();
        let p_load = proxy.clone();
        let p_nav = proxy.clone();
        let p_new = proxy.clone();
        let p_dl = proxy.clone();
        let p_done = proxy.clone();
        let p_serve = std::sync::Arc::new(std::sync::Mutex::new(proxy.clone()));
        let (x, y, w, h) = self.page_rect.get();
        let config = self.config.borrow().clone();
        let script = crate::scripts::page_script(&self.keymap.borrow(), &config);
        let label = id.to_string();
        let dark = self.dark.get();
        let mut context = self.context.borrow_mut();
        let builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_id(&label)
            .with_url(platform::to_engine(url))
            .with_bounds(rect(x, y, w, h))
            .with_visible(false)
            .with_focused(false)
            .with_background_color(if dark { (28, 28, 28, 255) } else { (255, 255, 255, 255) })
            .with_devtools(true)
            .with_clipboard(true)
            .with_hotkeys_zoom(false)
            .with_incognito(private)
            .with_autoplay(config.autoplay)
            .with_initialization_script(script)
            .with_ipc_handler(move |req| post(&p_ipc, Ev::Page(id, req.body().clone())))
            .with_document_title_changed_handler(move |title| post(&p_title, Ev::Title(id, title)))
            .with_on_page_load_handler(move |ev, url| {
                post(&p_load, Ev::Load(id, matches!(ev, PageLoadEvent::Started), url));
            })
            .with_navigation_handler(move |url| {
                // Addresses the engine can't show go to the system.
                let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
                if !matches!(scheme.as_str(), "http" | "https" | "about" | "data" | "blob" | "rippl" | "file" | "javascript") {
                    platform::open_uri(&url);
                    return false;
                }
                post(&p_nav, Ev::Navigate(id, url));
                true
            })
            .with_new_window_req_handler(move |url, _features| {
                post(&p_new, Ev::Popup(id, url));
                wry::NewWindowResponse::Deny
            })
            .with_download_started_handler(move |url, path| {
                let suggested = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| {
                    url.rsplit('/').next().unwrap_or("download").split('?').next().unwrap_or("download").to_string()
                });
                let dir = rippl_core::dirs::downloads();
                let _ = std::fs::create_dir_all(&dir);
                *path = rippl_core::downloads::destination(&dir, &suggested, |p| p.exists());
                post(&p_dl, Ev::DownloadStarted(url, path.clone()));
                true
            })
            .with_download_completed_handler(move |url, path, ok| post(&p_done, Ev::DownloadDone(url, path, ok)));
        // rippl:// pages. WebKitGTK keeps a scheme per engine (once), the
        // others per view.
        let once = cfg!(all(unix, not(target_os = "macos")));
        let builder = if once && self.scheme_set.get() {
            builder
        } else {
            builder.with_asynchronous_custom_protocol("rippl".into(), move |_wid, request, responder| {
                let uri = request.uri().to_string();
                if let Ok(proxy) = p_serve.lock() {
                    post(&proxy, Ev::Serve(uri, responder));
                }
            })
        };
        let built = platform::build(builder, &self.window, &self.host);
        if built.is_ok() && !private {
            self.scheme_set.set(true);
        }
        drop(context);
        match built {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("rippl: a tab's view: {e}");
                None
            }
        }
    }

    /// The front tab's view on the page card; the others hidden.
    pub fn place_views(&self) {
        let (x, y, w, h) = self.page_rect.get();
        let front = self.front();
        for (id, view) in self.views.borrow().iter() {
            if Some(*id) == front && w > 1.0 {
                let _ = view.set_bounds(rect(x, y, w, h));
                let _ = view.set_visible(true);
            } else {
                let _ = view.set_visible(false);
            }
        }
    }

    pub fn close(&self, id: TabId) {
        let (url, title, private) = match self.tabs.borrow().get(id) {
            Some(t) => (t.url.clone(), t.title.clone(), t.private),
            None => return,
        };
        if !private && !url.is_empty() {
            let mut closed = self.closed.borrow_mut();
            closed.push((url, title));
            if closed.len() > 30 {
                closed.remove(0);
            }
        }
        let view = self.views.borrow_mut().remove(&id);
        drop(view);
        self.extra.borrow_mut().remove(&id);
        let next = self.tabs.borrow_mut().close(id);
        match next {
            Some(n) => self.select(n),
            None => {
                let n = self.tabs.borrow_mut().open("");
                self.select(n);
            }
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    /// A tab's view goes, with its page process; its address stays.
    pub fn sleep(&self, id: TabId) {
        let view = self.views.borrow_mut().remove(&id);
        drop(view);
        if let Some(t) = self.tabs.borrow_mut().get_mut(id) {
            t.life = Life::Asleep;
            t.loading = false;
        }
        self.render_due.set(true);
    }

    pub fn go(&self, typed: &str, new_tab: bool, background: bool) {
        let typed = typed.trim();
        if typed.is_empty() {
            return;
        }
        let config = self.config.borrow().clone();
        let url = if let Some(u) = address::keyword(typed, &config.keywords) {
            u
        } else if typed.starts_with("rippl://") {
            typed.to_string()
        } else {
            match address::resolve(typed, &config.search) {
                Destination::Page(u) => u,
                Destination::Nothing => return,
            }
        };
        let url = rippl_core::clean::clean_link(&url);
        let blank = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.is_empty())).unwrap_or(true);
        if new_tab || background {
            self.open(&url, false, background);
            return;
        }
        let Some(id) = self.front() else {
            self.open(&url, false, false);
            return;
        };
        let _ = self.places.visit(&url, "", true);
        self.load(id, &url);
        if blank {
            self.select(id);
        }
    }

    pub fn load(&self, id: TabId, url: &str) {
        if let Some(t) = self.tabs.borrow_mut().get_mut(id) {
            t.url = url.to_string();
            t.title.clear();
        }
        let has_view = self.views.borrow().contains_key(&id);
        if has_view {
            if let Some(v) = self.views.borrow().get(&id) {
                let _ = v.load_url(&platform::to_engine(url));
            }
        } else {
            self.wake(id);
        }
        self.place_views();
        if let Some(v) = self.views.borrow().get(&id) {
            let _ = v.focus();
        }
        self.dirty.set(true);
        self.render_due.set(true);
    }

    pub fn front_view_eval(&self, js: &str) {
        if let Some(id) = self.front()
            && let Some(v) = self.views.borrow().get(&id)
        {
            let _ = v.evaluate_script(js);
        }
    }

    pub fn front_url(&self) -> String {
        self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone())).unwrap_or_default()
    }

    pub fn close_find(&self) {
        if self.find.borrow_mut().take().is_some() {
            self.front_view_eval("window.__rippl && __rippl.findClear()");
            self.render_due.set(true);
        }
    }

    pub fn set_zoom(&self, change: Option<f64>) {
        let Some(id) = self.front() else { return };
        let mut extra = self.extra.borrow_mut();
        let e = extra.entry(id).or_default();
        if e.zoom == 0.0 {
            e.zoom = 1.0;
        }
        e.zoom = match change {
            Some(step) => (e.zoom + step).clamp(0.3, 3.0),
            None => 1.0,
        };
        let z = e.zoom;
        drop(extra);
        if let Some(v) = self.views.borrow().get(&id) {
            let _ = v.zoom(z);
        }
        self.notice(&format!("{}%", (z * 100.0).round()));
    }

    /// The keys rippl listens for changed: every page and the column hear.
    pub fn apply_shortcuts(&self) {
        *self.keymap.borrow_mut() = commands::keymap(&self.config.borrow().shortcuts);
        let json = commands::keymap_json(&self.keymap.borrow());
        for v in self.views.borrow().values() {
            let _ = v.evaluate_script(&format!("window.__rippl && __rippl.keys({json})"));
        }
        self.render_due.set(true);
    }

    pub fn save_session(&self) {
        let text = Session::of(&self.tabs.borrow()).write();
        let path = session_path();
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        self.dirty.set(false);
    }

    /// Once a second: notices end, tabs fall asleep, the session is kept.
    pub fn tick(&self) {
        let expired = self.notice.borrow().as_ref().is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(4));
        if expired {
            self.notice("");
        }
        if self.dirty.get() {
            self.save_session();
        }
        if self.last_tick.get().elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_tick.set(Instant::now());
        let wait = self.config.borrow().sleep_after;
        let on_screen: Vec<TabId> = self.front().into_iter().collect();
        let due = {
            let tabs = self.tabs.borrow();
            let awake: Vec<_> = tabs.list.iter().filter(|t| self.views.borrow().contains_key(&t.id)).cloned().collect();
            rippl_core::tabs::due_for_sleep(&awake, &on_screen, Instant::now(), wait)
        };
        for id in due {
            self.sleep(id);
        }
    }

    pub fn quit(&self) {
        self.stop_grabs();
        self.save_session();
        std::process::exit(0);
    }

    pub fn suggest(&self, typed: &str) {
        let typed_t = typed.trim();
        if typed_t.is_empty() {
            self.eval_chrome(&format!("rippl.offers([], {})", json!(typed)));
            return;
        }
        let places = self.places.search(typed_t, 40).unwrap_or_default();
        let ranked = rippl_core::suggest::rank(typed_t, places, now(), 6);
        let mut offers: Vec<Value> = Vec::new();
        let config = self.config.borrow();
        let is_url = address::url_from(typed_t).is_some();
        if !is_url {
            offers.push(json!({"title": format!("Search for “{typed_t}”"), "url": "", "go": typed_t, "fill": typed_t}));
        }
        for p in &ranked {
            offers.push(json!({"title": p.title, "url": p.url, "pretty": address::pretty(&p.url)}));
        }
        self.eval_chrome(&format!("rippl.offers({}, {})", Value::Array(offers), json!(typed)));
        if config.suggestions && !is_url
            && let Some(endpoint) = address::suggest_endpoint(&config.search)
        {
            let proxy = self.proxy.clone();
            let typed = typed.to_string();
            let url = endpoint.replace("{}", &address::encode_query(typed_t));
            std::thread::spawn(move || {
                if let Some(body) = crate::net::get_text(&url, 64 * 1024) {
                    post(&proxy, Ev::Suggest(typed, address::parse_suggestions(&body)));
                }
            });
        }
    }

    pub fn suggestions_came(&self, typed: &str, words: Vec<String>) {
        let t = typed.trim();
        let places = self.places.search(t, 40).unwrap_or_default();
        let ranked = rippl_core::suggest::rank(t, places, now(), 4);
        let mut offers: Vec<Value> = vec![json!({"title": format!("Search for “{t}”"), "url": "", "go": t, "fill": t})];
        for w in words.iter().filter(|w| w.as_str() != t).take(4) {
            offers.push(json!({"title": w, "url": "", "go": w, "fill": w}));
        }
        for p in &ranked {
            offers.push(json!({"title": p.title, "url": p.url, "pretty": address::pretty(&p.url)}));
        }
        self.eval_chrome(&format!("rippl.offers({}, {})", Value::Array(offers), json!(typed)));
    }
}

/// The column's width, kept across launches.
pub fn config_column_width() -> Option<u32> {
    std::fs::read_to_string(rippl_core::dirs::data().join("column-width")).ok()?.trim().parse().ok()
}
