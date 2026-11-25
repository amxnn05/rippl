//! WebKit, set up once: one web context for every tab, one persistent
//! network session (cookies, cache, sign-ins), and the settings every view
//! shares. Nothing else in rippl creates WebKit objects except `view` here.

use std::path::PathBuf;

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
}

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

/// `~/.local/share/rippl` (or `$XDG_DATA_HOME/rippl`).
pub fn data_dir() -> PathBuf {
    webkit6::glib::user_data_dir().join("rippl")
}

/// `~/.cache/rippl` (or `$XDG_CACHE_HOME/rippl`).
pub fn cache_dir() -> PathBuf {
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
    pub fn new(content: UserContentManager) -> Engine {
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
        // <a ping> (a request to a tracker on every click): WebKitGTK 2.54
        // says its setting "is deprecated and does nothing", so Shield
        // blocks third-party pings instead (rippl_core::shield). DNS
        // prefetching, which tells the DNS server every link on a page, is
        // off by default.
        // Pages may not read the clipboard without a paste.
        settings.set_javascript_can_access_clipboard(false);
        // The user agent is left as WebKitGTK writes it: it already claims
        // to be Safari, which is what sites test against. Adding a name to
        // it is what made Google serve a degraded page.

        Engine { context, session, settings, content, on_download: Default::default() }
    }

    /// A new page view. `private`: its own throwaway network session, so
    /// its cookies and sign-ins exist only while it does.
    pub fn view(&self, private: bool) -> WebView {
        let session = if private {
            let own = NetworkSession::new_ephemeral();
            harden(&own);
            if let Some(hook) = self.on_download.borrow().clone() {
                own.connect_download_started(move |_, d| hook(d));
            }
            own
        } else {
            self.session.clone()
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
