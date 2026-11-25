//! WebKit, set up once: one web context for every tab, one persistent
//! network session (cookies, cache, sign-ins), and the settings every view
//! shares. Nothing else in rippl creates WebKit objects except `view` here.

use std::path::PathBuf;

use webkit6::prelude::*;
use webkit6::{
    CacheModel, CookieAcceptPolicy, CookiePersistentStorage, MemoryPressureSettings, NetworkSession, Settings,
    UserContentManager, WebContext, WebView,
};

/// Something told of each download (see downloads.rs).
pub type DownloadHook = std::rc::Rc<dyn Fn(&webkit6::Download)>;

pub struct Engine {
    pub context: WebContext,
    pub session: NetworkSession,
    pub settings: Settings,
    /// Shared by every view: Shield's filters live here (see shield.rs).
    pub content: UserContentManager,
    /// Told of every download, from the kept session and from each private
    /// tab's own (see downloads.rs).
    pub on_download: std::cell::RefCell<Option<DownloadHook>>,
    /// Whether rippl's web process extension (GPC, HTTPS-only) is loaded.
    pub extension: bool,
    /// Each space's own store of cookies and site data (space 0 uses
    /// `session`, the store rippl always had).
    spaces: std::cell::RefCell<std::collections::HashMap<u32, NetworkSession>>,
    /// What every new page process is told at start: (gpc, https_only,
    /// hosts allowed on http this session).
    start: std::cell::RefCell<(bool, bool, Vec<String>)>,
}

/// rippl's web process extension (crates/rippl-web-extension), from beside
/// the binary or the system install, copied into a folder of its own:
/// WebKit loads every library in the folder it is given, and lets that
/// folder into the page processes' sandbox.
fn install_extension() -> Option<PathBuf> {
    const NAME: &str = "librippl_web_extension.so";
    let candidates = [
        std::env::var_os("RIPPL_EXTENSION").map(PathBuf::from),
        std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(NAME))),
        Some(PathBuf::from("/usr/lib/rippl").join(NAME)),
    ];
    let source = candidates.into_iter().flatten().find(|p| p.is_file())?;
    let dir = shared_cache_dir().join("web-extension");
    let target = dir.join(NAME);
    let same = |a: &std::path::Path, b: &std::path::Path| -> bool {
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.len() == y.len() && x.modified().ok() <= y.modified().ok(),
            _ => false,
        }
    };
    if !same(&source, &target) {
        std::fs::create_dir_all(&dir).ok()?;
        let tmp = dir.join(format!("{NAME}.tmp"));
        std::fs::copy(&source, &tmp).ok()?;
        std::fs::rename(&tmp, &target).ok()?;
    }
    Some(dir)
}

/// `navigator.globalPrivacyControl`, for pages that check it in script
/// (the header itself comes from the extension).
const GPC_SCRIPT: &str = "Object.defineProperty(Navigator.prototype, 'globalPrivacyControl', \
    { get() { return true; }, configurable: true, enumerable: true });";

/// What every network session gets, kept or private.
fn harden(session: &NetworkSession) {
    // Intelligent Tracking Prevention: WebKit's own classifier that
    // partitions and purges the storage of sites that track across
    // sites, as Safari has it on.
    session.set_itp_enabled(true);
    if let Some(cookies) = session.cookie_manager() {
        // No third-party cookies at all, as Safari since 2020. The cookie
        // a tracker sets from inside someone else's page is the
        // cross-site tracking cookie.
        cookies.set_accept_policy(CookieAcceptPolicy::NoThirdParty);
    }
}

/// The profile this run is (`rippl --profile work`), if not the default
/// one. A profile is a whole separate browser: its own spaces, tabs,
/// sign-ins, history and bookmarks. Shared by
/// every profile: settings, passwords (the keyring), downloads, and the
/// ad-blocking lists.
static PROFILE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Sets the profile, once, before anything reads a folder. Names are
/// letters, digits, `-` and `_`, up to 32.
pub fn set_profile(name: Option<&str>) -> Result<(), String> {
    let name = name.map(str::trim).filter(|n| !n.is_empty() && *n != "default");
    if let Some(n) = name
        && (n.len() > 32 || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(format!("{n:?} isn't a profile name: letters, digits, - and _ only"));
    }
    let _ = PROFILE.set(name.map(str::to_string));
    Ok(())
}

pub fn profile() -> Option<&'static str> {
    PROFILE.get().and_then(|p| p.as_deref())
}

fn profiled(base: PathBuf) -> PathBuf {
    match profile() {
        Some(p) => base.join("profiles").join(p),
        None => base,
    }
}

/// `~/.local/share/rippl` (or `$XDG_DATA_HOME/rippl`), or the profile's.
pub fn data_dir() -> PathBuf {
    profiled(webkit6::glib::user_data_dir().join("rippl"))
}

/// `~/.cache/rippl`, or the profile's.
pub fn cache_dir() -> PathBuf {
    profiled(webkit6::glib::user_cache_dir().join("rippl"))
}

/// The cache every profile shares: Shield's lists, the web extension.
pub fn shared_cache_dir() -> PathBuf {
    webkit6::glib::user_cache_dir().join("rippl")
}

/// Total RAM in MB, from /proc/meminfo.
fn total_ram_mb() -> Option<u32> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some((kb / 1024) as u32)
}

impl Engine {
    pub fn new(content: UserContentManager, gpc: bool, https_only: bool) -> Engine {
        // Each web process is told how much it may use before it starts
        // dropping its caches. WebKit's own default is generous (most of the
        // machine's RAM); an eighth of RAM, between 512 MB and 2 GB, keeps
        // a heavy page working while making it give memory back much
        // sooner. Pages are never killed for it (kill threshold left at 0).
        let mut pressure = MemoryPressureSettings::new();
        let limit = total_ram_mb().map_or(1024, |mb| (mb / 8).clamp(512, 2048));
        pressure.set_memory_limit(limit);
        pressure.set_conservative_threshold(0.33);
        pressure.set_strict_threshold(0.5);
        pressure.set_poll_interval(5.0);

        let context = WebContext::builder().memory_pressure_settings(&pressure).build();
        // RIPPL_LEAN=1: smaller in-memory caches and fewer pages kept for
        // instant Back. Slower Back, less RAM. Default is a normal browser.
        let lean = std::env::var_os("RIPPL_LEAN").is_some();
        context.set_cache_model(if lean { CacheModel::DocumentBrowser } else { CacheModel::WebBrowser });

        // GPC and HTTPS-only live in the page processes: before the first one starts.
        let extension = match install_extension() {
            Some(dir) => {
                context.set_web_process_extensions_directory(&dir.to_string_lossy());
                context.set_web_process_extensions_initialization_user_data(
                    &(gpc, https_only, Vec::<String>::new()).to_variant(),
                );
                true
            }
            None => {
                eprintln!("rippl: web extension not found: no Global Privacy Control header, no HTTPS-only");
                false
            }
        };
        if gpc {
            content.add_script(&webkit6::UserScript::new(
                GPC_SCRIPT,
                webkit6::UserContentInjectedFrames::AllFrames,
                webkit6::UserScriptInjectionTime::Start,
                &[],
                &[],
            ));
        }

        // The network process gets the same limits. WebKit reads them only
        // before the first network session exists, so this goes first.
        NetworkSession::set_memory_pressure_settings(&mut pressure);
        let data = data_dir().join("web");
        let cache = cache_dir().join("web");
        let session = NetworkSession::new(data.to_str(), cache.to_str());
        harden(&session);
        // Cookies are only kept between launches if WebKitGTK is told
        // where: without this, every sign-in was lost at quit.
        if let Some(cookies) = session.cookie_manager() {
            let file = data.join("cookies.sqlite");
            cookies.set_persistent_storage(&file.to_string_lossy(), CookiePersistentStorage::Sqlite);
        }

        let settings = Settings::new();
        settings.set_enable_developer_extras(true); // Inspect Element in the context menu
        settings.set_enable_back_forward_navigation_gestures(true);
        settings.set_enable_smooth_scrolling(true);
        // RIPPL_NO_GPU=1: pages drawn without the GPU. Each page process
        // then skips loading the GPU drivers (LLVM, Mesa, NVIDIA): 167 → 95
        // MB for a light page, but only ~7% less on heavy sites, and about
        // twice the CPU while scrolling. Off by default (docs/performance.md).
        if std::env::var_os("RIPPL_NO_GPU").is_some() {
            settings.set_hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Never);
        }
        // <a ping> (a request to a tracker on every click): WebKitGTK 2.54
        // says its setting "is deprecated and does nothing", so Shield
        // blocks third-party pings instead (rippl_core::shield). DNS
        // prefetching, which tells the DNS server every link on a page, is
        // off by default.
        // A page may ask for windows; rippl decides (sitecard.rs): one right
        // after a click or key opens, one on its own is stopped and offered.
        settings.set_javascript_can_open_windows_automatically(true);
        // Pages may not read the clipboard without a paste.
        settings.set_javascript_can_access_clipboard(false);
        // The user agent is left as WebKitGTK writes it: it already claims
        // to be Safari, which is what sites test against. Adding a name to
        // it is what made Google serve a degraded page.

        Engine {
            context,
            session,
            settings,
            content,
            on_download: Default::default(),
            extension,
            start: std::cell::RefCell::new((gpc, https_only, Vec::new())),
            spaces: Default::default(),
        }
    }

    /// Tells every page process the two switches again (after a change in
    /// rippl://settings).
    pub fn send_settings(&self, gpc: bool, https_only: bool) {
        {
            let mut start = self.start.borrow_mut();
            start.0 = gpc;
            start.1 = https_only;
        }
        self.refresh_start();
        let message = webkit6::UserMessage::new("rippl-settings", Some(&(gpc, https_only).to_variant()));
        self.context.send_message_to_all_extensions(&message);
    }

    /// HTTPS-only: let `host` load over http until rippl quits: in the page
    /// processes running now, and in every one started from now on.
    pub fn allow_http(&self, host: &str) {
        {
            let mut start = self.start.borrow_mut();
            if start.2.iter().any(|h| h == host) {
                return;
            }
            start.2.push(host.to_string());
        }
        self.refresh_start();
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!("rippl: telling page processes {host} may use http");
        }
        let message = webkit6::UserMessage::new("rippl-allow-http", Some(&host.to_variant()));
        self.context.send_message_to_all_extensions(&message);
    }

    fn refresh_start(&self) {
        if self.extension {
            let start = self.start.borrow().clone();
            self.context.set_web_process_extensions_initialization_user_data(&start.to_variant());
        }
    }

    /// The store of `space`: cookies, sign-ins, site data of its own, in
    /// `spaces/<id>/` beside the first space's.
    pub fn session_for(&self, space: u32) -> NetworkSession {
        if space == 0 {
            return self.session.clone();
        }
        if let Some(s) = self.spaces.borrow().get(&space) {
            return s.clone();
        }
        let data = data_dir().join("spaces").join(space.to_string()).join("web");
        let cache = cache_dir().join("spaces").join(space.to_string()).join("web");
        let session = NetworkSession::new(data.to_str(), cache.to_str());
        harden(&session);
        if let Some(cookies) = session.cookie_manager() {
            let file = data.join("cookies.sqlite");
            cookies.set_persistent_storage(&file.to_string_lossy(), CookiePersistentStorage::Sqlite);
        }
        if let Some(hook) = self.on_download.borrow().clone() {
            session.connect_download_started(move |_, d| hook(d));
        }
        self.spaces.borrow_mut().insert(space, session.clone());
        session
    }

    /// A space was deleted: its sign-ins and site data go with it.
    pub fn forget_space(&self, space: u32) {
        if space == 0 {
            return;
        }
        let session = self.spaces.borrow_mut().remove(&space);
        let data = data_dir().join("spaces").join(space.to_string());
        let cache = cache_dir().join("spaces").join(space.to_string());
        let remove = move || {
            let _ = std::fs::remove_dir_all(&data);
            let _ = std::fs::remove_dir_all(&cache);
        };
        match session.and_then(|s| s.website_data_manager()) {
            Some(manager) => manager.clear(
                webkit6::WebsiteDataTypes::ALL,
                webkit6::glib::TimeSpan::from_seconds(0),
                None::<&webkit6::gio::Cancellable>,
                move |_| remove(),
            ),
            None => remove(),
        }
    }

    /// A new page view in `space`. `private`: its own throwaway network
    /// session, so its cookies and sign-ins exist only while it does.
    pub fn view(&self, private: bool, space: u32) -> WebView {
        let session = if private {
            let own = NetworkSession::new_ephemeral();
            harden(&own);
            if let Some(hook) = self.on_download.borrow().clone() {
                own.connect_download_started(move |_, d| hook(d));
            }
            own
        } else {
            self.session_for(space)
        };
        WebView::builder()
            .web_context(&self.context)
            .network_session(&session)
            .settings(&self.settings)
            .user_content_manager(&self.content)
            .hexpand(true)
            .vexpand(true)
            .build()
    }

    /// A view for a page that asked for a new window (`window.open`, a
    /// link with target=_blank). WebKit requires it be related to the
    /// view that asked: same session, same process rules.
    pub fn related_view(&self, opener: &WebView) -> WebView {
        WebView::builder()
            .related_view(opener)
            .settings(&self.settings)
            .user_content_manager(&self.content)
            .hexpand(true)
            .vexpand(true)
            .build()
    }
}
