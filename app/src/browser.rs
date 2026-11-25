//! The window: a column with the one field and the tabs, and the page.
//!
//! Tabs live in `rippl_core::tabs::Tabs`. A tab has a `WebView` (and so a
//! web process) only while it is Awake; the views are kept in `views`, and
//! the one in front is the visible child of `stage`. Switching tabs only
//! changes the visible child, so nothing reloads.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use rippl_core::address::{self, Destination};
use rippl_core::places::Places;
use rippl_core::session::Session;
use rippl_core::tabs::{self, Life, Pressure, TabId, Tabs};
use webkit6::prelude::*;
use webkit6::{gdk, gio, glib, gtk, LoadEvent, MediaCaptureState, WebProcessTerminationReason, WebView};

use crate::config::Config;
use crate::downloads::Downloads;
use crate::engine::{self, Engine};
use crate::omnibox::Offer;
use crate::permissions::Asking;
use crate::shield::{self, Shield};
use crate::pressure;

/// RIPPL_DEBUG=1: say what the tabs are doing on stderr.
macro_rules! debug {
    ($($t:tt)*) => {
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!("rippl: {}", format!($($t)*));
        }
    };
}

pub struct Browser {
    /// Settings; rippl://settings changes them while rippl runs.
    pub(crate) config: RefCell<Config>,
    /// HTTPS-only: hosts whose https failed, offered over http on their warning page.
    pub(crate) https_failed: RefCell<HashSet<String>>,
    /// HTTPS-only: per tab, the https addresses of http navigations seen
    /// lately (any frame), and the main-frame upgrade being waited for.
    pub(crate) upgrades_seen: RefCell<HashMap<TabId, Vec<String>>>,
    pub(crate) upgrading: RefCell<HashMap<TabId, String>>,
    pub(crate) shield: Rc<Shield>,
    pub(crate) engine: Engine,
    /// History, bookmarks, permission answers (places.sqlite).
    pub(crate) places: Places,
    pub(crate) downloads: Rc<Downloads>,
    pub(crate) tabs: RefCell<Tabs>,
    pub(crate) views: RefCell<HashMap<TabId, WebView>>,
    /// Tabs opened by a page (window.open, target=_blank) share their
    /// opener's web process: same family. A family's process may only be
    /// ended when none of its tabs is awake.
    pub(crate) family: RefCell<HashMap<TabId, TabId>>,
    /// Tabs whose next finished load was typed (counts more in history).
    pub(crate) typed_next: RefCell<HashSet<TabId>>,
    pub(crate) window: gtk::ApplicationWindow,
    pub(crate) column: gtk::Box,
    pub(crate) field: gtk::Entry,
    pub(crate) list: gtk::ListBox,
    /// Pinned tabs, as letter squares above the list.
    pub(crate) pins: gtk::Grid,
    pub(crate) new_tab: gtk::Button,
    pub(crate) settings_button: gtk::Button,
    /// One button per space, at the column's foot (spaces.rs).
    pub(crate) spaces_box: gtk::Box,
    pub(crate) tabs_area: gtk::ScrolledWindow,
    // The field's offers (omnibox.rs).
    pub(crate) suggest_area: gtk::ScrolledWindow,
    pub(crate) suggest_list: gtk::ListBox,
    pub(crate) offers: RefCell<Vec<Offer>>,
    pub(crate) typed: RefCell<String>,
    pub(crate) completing: Cell<bool>,
    /// Engine suggestions: (what they were for, the suggestions), and a
    /// counter that drops answers to older keystrokes.
    pub(crate) engine_says: RefCell<(String, Vec<String>)>,
    pub(crate) typing_gen: Cell<u32>,
    pub(crate) stage: gtk::Stack,
    // Split view (split.rs).
    pub(crate) paned: gtk::Paned,
    pub(crate) partner: RefCell<HashMap<TabId, TabId>>,
    pub(crate) shown_split: RefCell<Option<(TabId, TabId)>>,
    pub(crate) blank: gtk::Box,
    /// The new tab's field, offers and Home grid, in the blank page (home.rs).
    pub(crate) home: gtk::Box,
    pub(crate) home_grid: gtk::FlowBox,
    pub(crate) home_mode: Cell<bool>,
    /// Scripts switched by settings: YouTube's ads, video keys (pagescripts.rs).
    pub(crate) page_scripts: RefCell<Vec<webkit6::UserScript>>,
    /// Peek: the layer over the page, the panel's frame, its buttons, and
    /// what is in it (peek.rs).
    pub(crate) peek_dim: gtk::Box,
    pub(crate) peek_frame: gtk::Box,
    pub(crate) peek_close: gtk::Button,
    pub(crate) peek_keep: gtk::Button,
    pub(crate) peeking: RefCell<Option<crate::peek::Peeking>>,
    /// The player at the column's foot (player.rs): tabs by when their
    /// sound started, the ones it paused, and the one it shows.
    pub(crate) player_row: gtk::Box,
    pub(crate) sounding: RefCell<Vec<TabId>>,
    pub(crate) stilled: RefCell<HashSet<TabId>>,
    pub(crate) player_for: RefCell<Option<TabId>>,
    // Find in page (find.rs).
    pub(crate) find_bar: gtk::Box,
    pub(crate) find_entry: gtk::Entry,
    pub(crate) find_count: gtk::Label,
    /// Where the link under the pointer goes, at the foot of the page.
    pub(crate) status: gtk::Label,
    pub(crate) found: Cell<u32>,
    // Permission questions (permissions.rs).
    pub(crate) ask_bar: gtk::Box,
    pub(crate) ask_label: gtk::Label,
    pub(crate) ask_allow: gtk::Button,
    pub(crate) ask_deny: gtk::Button,
    pub(crate) asking: Asking,
    /// Tabs in reading mode, and the page each is reading (reader.rs).
    pub(crate) reading: RefCell<HashMap<TabId, String>>,
    /// A short message over the bottom of the page.
    pub(crate) toast: gtk::Label,
    pub(crate) toast_gen: Cell<u32>,
    /// This launch's secret for rippl:// page buttons (internal_pages.rs).
    pub(crate) token: String,
    // Passwords (signin.rs, keyring.rs).
    pub(crate) keyring: Rc<crate::keyring::Keyring>,
    pub(crate) pending: RefCell<HashMap<TabId, crate::signin::Pending>>,
    pub(crate) save_question: RefCell<Option<crate::signin::SaveQuestion>>,
    pub(crate) save_bar: gtk::Box,
    pub(crate) save_label: gtk::Label,
    pub(crate) save_never: gtk::Button,
    pub(crate) save_later: gtk::Button,
    pub(crate) save_yes: gtk::Button,
    pub(crate) accounts_popover: gtk::Popover,
    pub(crate) accounts_list: gtk::ListBox,
    pub(crate) offered: RefCell<Vec<crate::keyring::Account>>,
    // Site card, tab menu, blocked pop-ups (sitecard.rs).
    pub(crate) card: gtk::Popover,
    pub(crate) tab_menu: gtk::Popover,
    pub(crate) blocked_popups: RefCell<HashMap<TabId, Vec<String>>>,
    pub(crate) popup_bar: gtk::Box,
    pub(crate) popup_label: gtk::Label,
    pub(crate) popup_open: gtk::Button,
    pub(crate) popup_always: gtk::Button,
    pub(crate) popup_dismiss: gtk::Button,
    /// The page and what floats over it.
    pub(crate) page_area: gtk::Overlay,
    pub(crate) dirty: Cell<bool>,
    /// The GTK theme the desktop gave rippl at start (look.rs).
    pub(crate) desktop_theme: std::cell::OnceCell<Option<String>>,
    /// Tabs closed lately (closed.rs).
    pub(crate) closed: RefCell<Vec<crate::closed::Closed>>,
    /// Site icons known by address, and addresses already asked for (pagelook.rs).
    pub(crate) icons: RefCell<HashMap<String, gdk::Texture>>,
    pub(crate) icons_asked: RefCell<HashSet<String>>,
    me: RefCell<Weak<Browser>>,
}

impl Browser {
    pub fn new(app: &gtk::Application) -> Rc<Browser> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("rippl")
            .default_width(1280)
            .default_height(820)
            .build();
        // A tiling compositor draws no title bar and wants none; elsewhere
        // GTK's own keeps the window movable and closable.
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
        if ["hyprland", "sway", "river", "niri", "i3"].iter().any(|d| desktop.contains(d)) {
            window.set_decorated(false);
        }

        let field = gtk::Entry::builder().placeholder_text("Search or enter address").build();
        field.add_css_class("field");
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
        list.add_css_class("tabs");
        // The column draws it: pinned squares, the tabs, a quiet
        // "New tab" row, all in one scrolling area (look.rs).
        // Six to a row. A FlowBox here was given no height inside the
        // scrolling column, and cut its pins in half.
        let pins = gtk::Grid::builder().column_spacing(4).row_spacing(4).column_homogeneous(true).build();
        pins.add_css_class("pins");
        let new_tab = Browser::new_tab_row();
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rows.append(&pins);
        rows.append(&list);
        rows.append(&new_tab);
        let tabs_area = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&rows)
            .build();
        let suggest_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).build();
        suggest_list.add_css_class("offers");
        suggest_list.set_can_focus(false);
        let suggest_area = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&suggest_list)
            .visible(false)
            .build();
        let downloads = Downloads::new();
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.add_css_class("column");
        column.set_size_request(232, -1);
        column.set_hexpand(false);
        column.append(&field);
        column.append(&suggest_area);
        column.append(&tabs_area);
        column.append(&downloads.area);
        let player_row = gtk::Box::builder().visible(false).build();
        player_row.add_css_class("player");
        column.append(&player_row);
        let foot = Browser::column_foot();
        column.append(&foot.0);

        let stage = gtk::Stack::builder().hexpand(true).vexpand(true).build();
        stage.add_css_class("stage");
        // The card's rounded corners cut the page too: without this the
        // page paints square corners over them.
        stage.set_overflow(gtk::Overflow::Hidden);
        let blank = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let home_grid = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(4)
            .max_children_per_line(4)
            .homogeneous(true)
            .column_spacing(8)
            .row_spacing(8)
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        home_grid.add_css_class("home-grid");
        let home = gtk::Box::new(gtk::Orientation::Vertical, 8);
        home.add_css_class("home");
        home.append(&home_grid);
        stage.add_named(&blank, Some("blank"));
        let paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .wide_handle(true)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .build();
        paned.add_css_class("split");
        stage.add_named(&paned, Some("split"));

        // The find bar and the permission question float over the page.
        let (find_bar, find_entry, find_count) = Browser::find_bar();
        let (ask_bar, ask_label, ask_deny, ask_allow) = Browser::permission_bar();
        let overlay = gtk::Overlay::builder().child(&stage).hexpand(true).vexpand(true).build();
        overlay.add_overlay(&find_bar);
        overlay.add_overlay(&ask_bar);
        let (save_bar, save_label, save_never, save_later, save_yes) = Browser::save_bar();
        overlay.add_overlay(&save_bar);
        let (popup_bar, popup_label, popup_open, popup_always, popup_dismiss) = Browser::popup_bar();
        overlay.add_overlay(&popup_bar);
        let (peek_dim, peek_frame, peek_close, peek_keep) = Browser::peek_layer();
        overlay.add_overlay(&peek_dim);
        let accounts_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
        accounts_list.add_css_class("accounts");
        let accounts_popover = gtk::Popover::builder().child(&accounts_list).build();
        let toast = gtk::Label::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .margin_bottom(24)
            .visible(false)
            .build();
        toast.add_css_class("toast");
        overlay.add_overlay(&toast);
        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .margin_start(12)
            .margin_bottom(12)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(80)
            .visible(false)
            .build();
        status.add_css_class("status");
        status.set_can_target(false);
        overlay.add_overlay(&status);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&column);
        body.append(&overlay);
        window.set_child(Some(&body));

        let config = Config::load();
        let shield = Shield::new(config.shield, config.ublock);
        let _ = std::fs::create_dir_all(engine::data_dir());
        let places = Places::open(Some(&engine::data_dir().join("places.sqlite"))).unwrap_or_else(|e| {
            eprintln!("rippl: history and bookmarks unavailable ({e}); keeping them in memory this time");
            Places::open(None).expect("in-memory SQLite")
        });
        let engine = Engine::new(shield.content.clone(), config.gpc, config.https_only);
        // Downloads from every session: the kept one here, private ones as
        // they are made (engine.rs).
        let hook = {
            let downloads = downloads.clone();
            Rc::new(move |d: &webkit6::Download| downloads.started(d)) as engine::DownloadHook
        };
        let kept = hook.clone();
        engine.session.connect_download_started(move |_, d| kept(d));
        *engine.on_download.borrow_mut() = Some(hook);

        let browser = Rc::new(Browser {
            engine,
            shield,
            config: RefCell::new(config),
            https_failed: RefCell::new(HashSet::new()),
            upgrades_seen: RefCell::new(HashMap::new()),
            upgrading: RefCell::new(HashMap::new()),
            places,
            downloads,
            tabs: RefCell::new(Tabs::new()),
            views: RefCell::new(HashMap::new()),
            family: RefCell::new(HashMap::new()),
            typed_next: RefCell::new(HashSet::new()),
            window,
            column,
            field,
            list,
            pins,
            new_tab,
            settings_button: foot.1,
            spaces_box: foot.2,
            tabs_area,
            suggest_area,
            suggest_list,
            offers: RefCell::new(Vec::new()),
            typed: RefCell::new(String::new()),
            completing: Cell::new(false),
            engine_says: RefCell::new((String::new(), Vec::new())),
            typing_gen: Cell::new(0),
            stage,
            paned,
            partner: RefCell::new(HashMap::new()),
            shown_split: RefCell::new(None),
            blank,
            home,
            home_grid,
            home_mode: Cell::new(false),
            page_scripts: RefCell::new(Vec::new()),
            peek_dim,
            peek_frame,
            peek_close,
            peek_keep,
            peeking: RefCell::new(None),
            player_row,
            sounding: RefCell::new(Vec::new()),
            stilled: RefCell::new(HashSet::new()),
            player_for: RefCell::new(None),
            find_bar,
            find_entry,
            find_count,
            status,
            found: Cell::new(0),
            ask_bar,
            ask_label,
            ask_allow,
            ask_deny,
            asking: Asking::default(),
            reading: RefCell::new(HashMap::new()),
            toast,
            toast_gen: Cell::new(0),
            token: crate::internal_pages::new_token(),
            keyring: Rc::new(crate::keyring::Keyring::new()),
            pending: RefCell::new(HashMap::new()),
            save_question: RefCell::new(None),
            save_bar,
            save_label,
            save_never,
            save_later,
            save_yes,
            accounts_popover,
            accounts_list,
            offered: RefCell::new(Vec::new()),
            page_area: overlay.clone(),
            card: Browser::card_popover(),
            tab_menu: gtk::Popover::builder().has_arrow(false).build(),
            blocked_popups: RefCell::new(HashMap::new()),
            popup_bar,
            popup_label,
            popup_open,
            popup_always,
            popup_dismiss,
            dirty: Cell::new(false),
            desktop_theme: std::cell::OnceCell::new(),
            closed: RefCell::new(Vec::new()),
            icons: RefCell::new(HashMap::new()),
            icons_asked: RefCell::new(HashSet::new()),
            me: RefCell::new(Weak::new()),
        });
        *browser.me.borrow_mut() = Rc::downgrade(&browser);

        browser.style();
        browser.apply_look();
        browser.follow_desktop_look();
        browser.wire_look();
        browser.wire_omnibox();
        browser.wire_find();
        browser.wire_permission_bar();
        browser.wire_internal_pages();
        browser.wire_curtain();
        browser.wire_signin();
        browser.wire_popup_bar();
        browser.card.set_parent(&browser.field);
        browser.tab_menu.set_parent(&browser.tabs_area);
        browser.tab_menu.add_css_class("tab-menu");
        let weak = Rc::downgrade(&browser);
        browser.field.connect_icon_press(move |_, position| {
            if let (Some(b), gtk::EntryIconPosition::Primary) = (weak.upgrade(), position) {
                b.toggle_bookmark();
            }
        });
        browser.wire_actions(app);
        browser.wire_window();
        browser.wire_home();
        browser.install_page_scripts();
        browser.wire_peek();
        browser.wire_player();
        browser.watch();
        browser.window.present();
        // Pages load once Shield's filters are in place (a second at most),
        // so the first requests of the restored tab are already blocked.
        let weak = Rc::downgrade(&browser);
        browser.shield.start(move || {
            if let Some(b) = weak.upgrade() {
                b.restore();
            }
        });
        browser
    }

    pub(crate) fn weak(&self) -> Weak<Browser> {
        self.me.borrow().clone()
    }

    // ---- tabs ---------------------------------------------------------

    /// Opens `url` in a new tab after the front one. `url` empty: a blank
    /// tab, with no view until something is typed into it.
    pub fn open(&self, url: &str, private: bool, select: bool) -> TabId {
        let id = {
            let mut tabs = self.tabs.borrow_mut();
            let id = tabs.open(url);
            tabs.get_mut(id).unwrap().private = private;
            if url.is_empty() {
                tabs.get_mut(id).unwrap().title = if private { "Private tab".into() } else { "New tab".into() };
            }
            id
        };
        if select {
            self.select(id);
        } else {
            self.render();
        }
        self.dirty.set(true);
        id
    }

    /// Puts `id` in front, waking it if it has no view.
    pub fn select(&self, id: TabId) {
        debug!("select {id}");
        let (url, life) = {
            let mut tabs = self.tabs.borrow_mut();
            tabs.select(id);
            let Some(tab) = tabs.get(id) else { return };
            (tab.url.clone(), tab.life)
        };
        debug!("select {id}: {life:?} {url}");
        if life != Life::Awake && !url.is_empty() {
            self.wake(id);
        }
        self.place(id);
        // Find belongs to the page it searched.
        if self.find_bar.is_visible() {
            self.find_bar.set_visible(false);
        }
        self.show_front();
        self.show_question();
        self.show_save_question();
        self.show_popup_bar();
        self.accounts_popover.popdown();
        self.card.popdown();
        self.render();
    }

    /// Wakes `id` to be shown beside another tab.
    pub(crate) fn wake_for_split(&self, id: TabId) {
        self.wake(id);
    }

    /// Gives `id` a view and its page back: from what it kept when it fell
    /// asleep (history and scroll), or from its address if it never had one.
    fn wake(&self, id: TabId) {
        debug!("wake {id}");
        let (url, private, memory, space) = {
            let mut tabs = self.tabs.borrow_mut();
            let Some(tab) = tabs.get_mut(id) else { return };
            tab.life = Life::Awake;
            (tab.url.clone(), tab.private, tab.memory.take(), tab.space)
        };
        // Another awake tab of the same site: share its page process (each
        // process costs ~160 MB before any page content). Same site only,
        // same kind of session: a private tab never shares with a kept one.
        let sibling = self.sibling_of(id, &url, private);
        let view = match &sibling {
            Some((other, opener)) => {
                let family = self.family.borrow().get(other).copied().unwrap_or(*other);
                self.family.borrow_mut().insert(id, family);
                debug!("wake {id}: sharing tab {other}'s page process");
                self.engine.related_view(opener)
            }
            None => self.engine.view(private, space),
        };
        self.adopt(id, &view);
        debug!("wake {id}: kept state = {:?} bytes", memory.as_ref().map(Vec::len));
        let restored = memory.is_some_and(|bytes| {
            let state = webkit6::WebViewSessionState::new(&glib::Bytes::from_owned(bytes));
            view.restore_session_state(&state);
            // Restoring brings back the history, not the page: go to it.
            match view.back_forward_list().and_then(|l| l.current_item()) {
                Some(item) => {
                    view.go_to_back_forward_list_item(&item);
                    true
                }
                None => false,
            }
        });
        if !restored && !url.is_empty() {
            view.load_uri(&url);
        }
    }

    /// An awake tab on the same site as `url` (same host, www aside), with
    /// the same privacy, whose process a new view of `id` can share.
    fn sibling_of(&self, id: TabId, url: &str, private: bool) -> Option<(TabId, WebView)> {
        if std::env::var_os("RIPPL_NO_SHARE").is_some() {
            return None;
        }
        let host = shield::host_of(url)?;
        let tabs = self.tabs.borrow();
        let views = self.views.borrow();
        // Only within one space: each space has its own sign-ins.
        let space = tabs.get(id).map(|t| t.space)?;
        tabs.list
            .iter()
            .filter(|t| t.id != id && t.private == private && t.life == Life::Awake && t.space == space)
            .find(|t| shield::host_of(&t.url).as_deref() == Some(host.as_str()))
            .and_then(|t| views.get(&t.id).map(|v| (t.id, v.clone())))
    }

    /// Puts one tab to sleep now (spaces.rs moves a tab between stores).
    pub(crate) fn sleep_tab(&self, id: TabId) {
        self.sleep(id);
    }

    /// Takes `id`'s view away, keeping what it needs to come back. Its web
    /// process goes with the view.
    fn sleep(&self, id: TabId) {
        debug!("sleep {id}");
        let Some(view) = self.views.borrow_mut().remove(&id) else { return };
        let memory = view.session_state().and_then(|s| s.serialize()).map(|b| b.to_vec());
        if self.shown_split.borrow().is_some_and(|(l, r)| l == id || r == id) {
            self.unsplit_layout();
        }
        self.detach(&view);
        self.release(id, &view);
        if let Some(tab) = self.tabs.borrow_mut().get_mut(id) {
            tab.life = Life::Asleep;
            tab.memory = memory;
            tab.loading = false;
            tab.playing_audio = false;
            tab.capturing = false;
        }
        self.render();
    }

    /// Ends `view`'s web process, unless another awake tab of its family
    /// still uses it. Dropping the view alone is not enough: WebKit keeps
    /// a finished page's process cached for reuse, which is exactly the
    /// memory sleeping is meant to give back.
    fn release(&self, id: TabId, view: &WebView) {
        let family = self.family.borrow().get(&id).copied().unwrap_or(id);
        let shared = {
            let families = self.family.borrow();
            self.views
                .borrow()
                .keys()
                .any(|other| *other != id && families.get(other).copied().unwrap_or(*other) == family)
        };
        if !shared {
            debug!("release {id}: ending its web process");
            view.terminate_web_process();
        }
    }

    pub fn close(&self, id: TabId) {
        self.remember_closed(id);
        self.separate(id);
        let view = self.views.borrow_mut().remove(&id);
        if let Some(view) = view {
            self.detach(&view);
            self.release(id, &view);
        }
        self.family.borrow_mut().remove(&id);
        self.typed_next.borrow_mut().remove(&id);
        self.drop_questions(id);
        self.pending.borrow_mut().remove(&id);
        self.blocked_popups.borrow_mut().remove(&id);
        if self.save_question.borrow().as_ref().is_some_and(|q| q.tab == id) {
            self.save_question.borrow_mut().take();
            self.save_bar.set_visible(false);
        }
        let next = self.tabs.borrow_mut().close(id);
        match next {
            Some(n) => self.select(n),
            None => {
                self.stage.set_visible_child(&self.blank);
                self.show_front();
                self.render();
                self.field.grab_focus();
            }
        }
        self.dirty.set(true);
    }

    /// Whether `view` is still `id`'s. Signals from a view being taken
    /// away (a sleeping or closing tab's) must not touch the tab.
    pub(crate) fn ours(&self, id: TabId, view: &WebView) -> bool {
        self.views.borrow().get(&id).is_some_and(|mine| mine == view)
    }

    pub(crate) fn front(&self) -> Option<TabId> {
        self.tabs.borrow().front
    }

    pub(crate) fn front_view(&self) -> Option<WebView> {
        let id = self.front()?;
        self.views.borrow().get(&id).cloned()
    }

    /// Return in the field: an address, or a search.
    pub(crate) fn go(&self, typed: &str) {
        let search = self.config.borrow().search.clone();
        // "yt cats": a site keyword first, then an address or a search.
        let keywords = self.config.borrow().keywords.clone();
        let url = match address::keyword(typed, &keywords) {
            Some(url) => url,
            None => match address::resolve(typed, &search) {
                Destination::Page(url) => url,
                Destination::Nothing => return,
            },
        };
        let front = self.front();
        let Some(id) = front else {
            self.open(&url, false, true);
            return;
        };
        if let Some(tab) = self.tabs.borrow_mut().get_mut(id) {
            tab.url = url.clone();
        }
        self.typed_next.borrow_mut().insert(id);
        match self.front_view() {
            Some(view) => view.load_uri(&url),
            None => self.select(id), // wakes it, and the wake loads `url`
        }
        if self.front_view().is_some() {
            self.place(id);
        }
        self.dirty.set(true);
    }

    // ---- a view's signals ---------------------------------------------

    /// Puts `view` on the stage as `id`'s and listens to it.
    pub(crate) fn adopt(&self, id: TabId, view: &WebView) {
        self.stage.add_named(view, Some(&format!("tab-{id}")));
        self.views.borrow_mut().insert(id, view.clone());

        // In split view, a click into a pane puts its tab in front.
        let focus = gtk::EventControllerFocus::new();
        let weak = self.weak();
        focus.connect_enter(move |_| {
            if let Some(b) = weak.upgrade() {
                b.pane_focused(id);
            }
        });
        view.add_controller(focus);

        let weak = self.weak();
        view.connect_title_notify(move |v| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            let title = v.title().map(|t| t.to_string()).filter(|t| !t.trim().is_empty());
            let (url, private) = {
                let mut tabs = b.tabs.borrow_mut();
                let Some(tab) = tabs.get_mut(id) else { return };
                tab.title = title.clone().unwrap_or_else(|| address::pretty(&tab.url));
                (tab.url.clone(), tab.private)
            };
            if let (Some(title), false, true) = (title, private, b.config.borrow().history) {
                let _ = b.places.retitle(&url, &title);
            }
            b.dirty.set(true);
            b.render();
            if b.front() == Some(id) {
                b.show_front();
            }
        });

        let weak = self.weak();
        view.connect_uri_notify(move |v| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            let Some(uri) = v.uri() else { return };
            if let Some(tab) = b.tabs.borrow_mut().get_mut(id) {
                tab.url = uri.to_string();
            }
            b.reader_followed(id, v);
            b.apply_keep_awake();
            b.dirty.set(true);
            if b.front() == Some(id) {
                b.show_front();
            }
        });

        let weak = self.weak();
        view.connect_is_loading_notify(move |v| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            if let Some(tab) = b.tabs.borrow_mut().get_mut(id) {
                tab.loading = v.is_loading();
            }
        });

        let weak = self.weak();
        view.connect_estimated_load_progress_notify(move |v| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            if b.front() == Some(id) {
                let p = v.estimated_load_progress();
                b.field.set_progress_fraction(if p >= 1.0 || !v.is_loading() { 0.0 } else { p });
            }
        });

        let weak = self.weak();
        view.connect_load_changed(move |v, event| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            match event {
                LoadEvent::Started => {
                    b.watch_upgrade(id, v);
                    return;
                }
                LoadEvent::Committed => {
                    b.upgrading.borrow_mut().remove(&id);
                    return;
                }
                LoadEvent::Finished => {
                    b.upgrading.borrow_mut().remove(&id);
                }
                _ => return,
            }
            if b.front() == Some(id) {
                b.field.set_progress_fraction(0.0);
            }
            b.signin_page_loaded(id, v);
            b.dusk(id, v);
            // Reading mode starts at the top, not where the page was scrolled.
            if b.reading.borrow().contains_key(&id) {
                v.evaluate_javascript("scrollTo(0, 0)", None, None, None::<&gio::Cancellable>, |_| {});
            }
            // History: kept pages only, never a private tab's.
            // A reading-mode view of the page is not another visit.
            let private = b.tabs.borrow().get(id).is_none_or(|t| t.private) || b.reading.borrow().contains_key(&id);
            let typed = b.typed_next.borrow_mut().remove(&id);
            if !private && b.config.borrow().history
                && let Some(uri) = v.uri() {
                    let title = v.title().map(|t| t.to_string()).unwrap_or_default();
                    if let Err(e) = b.places.visit(&uri, &title, typed) {
                        eprintln!("rippl: history: {e}");
                    }
                }
            if b.front() == Some(id) {
                b.show_front();
            }
        });

        // HTTPS-only: the secure address failed. Say so, and offer http as
        // a choice rather than an error page.
        let weak = self.weak();
        view.connect_load_failed(move |v, _, failing, error| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return false };
            b.upgrading.borrow_mut().remove(&id);
            if error.matches(webkit6::NetworkError::Cancelled)
                || error.matches(webkit6::PolicyError::FrameLoadInterruptedByPolicyChange)
            {
                return false;
            }
            b.https_fallback(v, failing, error.message())
        });
        let weak = self.weak();
        view.connect_load_failed_with_tls_errors(move |v, failing, _, _| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return false };
            b.https_fallback(v, failing, "The site's certificate isn't valid for it.")
        });
        // A click on that page's "continue to http" link: allowed for the host.
        let weak = self.weak();
        view.connect_decide_policy(move |v, decision, kind| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return false };
            // Shift+click on a link: a peek, not a navigation or a window.
            if matches!(kind, webkit6::PolicyDecisionType::NavigationAction | webkit6::PolicyDecisionType::NewWindowAction)
                && let Some(action) =
                    decision.downcast_ref::<webkit6::NavigationPolicyDecision>().and_then(|d| d.navigation_action())
                && b.is_peek_click(&action)
                && let Some(url) = action.request().and_then(|r| r.uri()).filter(|u| u.starts_with("http"))
            {
                decision.ignore();
                let url = url.to_string();
                glib::idle_add_local_once(move || b.peek(&url, id));
                return true;
            }
            if kind != webkit6::PolicyDecisionType::NavigationAction {
                return false;
            }
            let uri = decision
                .downcast_ref::<webkit6::NavigationPolicyDecision>()
                .and_then(|d| d.navigation_action())
                .and_then(|a| a.request())
                .and_then(|r| r.uri());
            if let Some(uri) = uri.filter(|u| u.starts_with("http://"))
                && let Some(origin) = rippl_core::passwords::origin_of(&uri)
            {
                let host = rippl_core::passwords::host_of_origin(&origin).to_string();
                if b.https_failed.borrow().contains(&host) {
                    b.engine.allow_http(&host);
                } else if let Some(secure) = address::https_upgrade(&uri) {
                    // The page process will ask for `secure` instead; if it is
                    // the main frame, the load that starts will say so.
                    let mut seen = b.upgrades_seen.borrow_mut();
                    let list = seen.entry(id).or_default();
                    list.push(secure);
                    if list.len() > 8 {
                        list.remove(0);
                    }
                }
            }
            false
        });

        let weak = self.weak();
        view.connect_permission_request(move |v, request| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else {
                request.deny();
                return true;
            };
            b.permission_asked(id, v, request)
        });
        self.wire_find_for(id, view);

        // The site's icon, when the page gives one.
        let weak = self.weak();
        view.connect_favicon_notify(move |v| {
            if let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) {
                b.render();
            }
        });

        // Where a link goes, while the pointer is on it.
        let weak = self.weak();
        view.connect_mouse_target_changed(move |v, hit, _| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            match hit.link_uri().filter(|_| hit.context_is_link()) {
                Some(uri) => {
                    b.status.set_label(rippl_core::suggest::bare(&uri).trim_end_matches('/'));
                    b.status.set_visible(true);
                }
                None => b.status.set_visible(false),
            }
        });

        let weak = self.weak();
        view.connect_is_playing_audio_notify(move |v| {
            let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
            if let Some(tab) = b.tabs.borrow_mut().get_mut(id) {
                tab.playing_audio = v.is_playing_audio();
            }
            b.heard(id, v.is_playing_audio());
            b.render();
        });

        let capture = {
            let weak = self.weak();
            move |v: &WebView| {
                let Some(b) = weak.upgrade().filter(|b| b.ours(id, v)) else { return };
                let on = |s: MediaCaptureState| s != MediaCaptureState::None;
                let capturing =
                    on(v.camera_capture_state()) || on(v.microphone_capture_state()) || on(v.display_capture_state());
                if let Some(tab) = b.tabs.borrow_mut().get_mut(id) {
                    tab.capturing = capturing;
                }
            }
        };
        view.connect_camera_capture_state_notify(capture.clone());
        view.connect_microphone_capture_state_notify(capture.clone());
        view.connect_display_capture_state_notify(capture);

        // A page asking for a new window gets a new tab. A middle-click
        // opens it behind; anything else brings it to front.
        let weak = self.weak();
        view.connect_create(move |opener, action| {
            let b = weak.upgrade()?;
            let url = action.request().and_then(|r| r.uri()).map(|u| u.to_string()).unwrap_or_default();
            // A window without a click or key just before: refused, and said
            // so, unless the site may open pop-ups.
            if !action.is_user_gesture() {
                let site = opener.uri().and_then(|u| shield::host_of(&u));
                let allowed = site.is_some_and(|h| b.places.site_flag(&h, crate::sitecard::POPUPS).unwrap_or(false));
                if !allowed {
                    b.popup_blocked(id, url);
                    return None;
                }
            }
            let behind = action.mouse_button() == 2;
            let private = b.tabs.borrow().get(id).is_some_and(|t| t.private);
            let view = b.engine.related_view(opener);
            let new = {
                let mut tabs = b.tabs.borrow_mut();
                let new = tabs.open(url);
                let tab = tabs.get_mut(new).unwrap();
                tab.life = Life::Awake;
                tab.private = private;
                new
            };
            let family = b.family.borrow().get(&id).copied().unwrap_or(id);
            b.family.borrow_mut().insert(new, family);
            b.adopt(new, &view);
            if behind {
                b.render();
            } else {
                b.select(new);
            }
            b.dirty.set(true);
            Some(view.upcast())
        });

        // window.close() from the page. Only while the view is still this
        // tab's: a view rippl is already taking away is not the page asking.
        let weak = self.weak();
        view.connect_close(move |v| {
            let Some(b) = weak.upgrade() else { return };
            let ours = b.ours(id, v);
            if ours {
                b.close(id);
            }
        });

        // A crashed page comes back by itself if you are looking at it;
        // otherwise it goes to sleep and comes back when you are.
        let weak = self.weak();
        view.connect_web_process_terminated(move |v, reason| {
            let Some(b) = weak.upgrade() else { return };
            let ours = b.ours(id, v);
            if reason == WebProcessTerminationReason::TerminatedByApi || !ours {
                return;
            }
            if b.front() == Some(id) {
                v.reload();
            } else {
                b.sleep(id);
            }
        });
    }

    // ---- the column ---------------------------------------------------

    /// The field and the window title follow the tab in front.
    pub(crate) fn show_front(&self) {
        let (url, title) = {
            let tabs = self.tabs.borrow();
            match tabs.front.and_then(|f| tabs.get(f)) {
                Some(t) => (t.url.clone(), t.title.clone()),
                None => (String::new(), String::new()),
            }
        };
        if !self.field.has_focus() {
            self.field.set_text(&url);
        }
        // A star in the field for a bookmarked page; click it to toggle.
        let starred = !url.is_empty() && self.places.is_bookmarked(&url).unwrap_or(false);
        // No page, nothing to bookmark: the new tab's field is bare.
        self.field.set_primary_icon_name((!url.is_empty()).then_some(if starred { "starred-symbolic" } else { "non-starred-symbolic" }));
        self.field.set_primary_icon_tooltip_text(Some(if starred { "Remove bookmark (Ctrl+D)" } else { "Bookmark (Ctrl+D)" }));
        // A sign in the field while Shield is off for this site.
        let off = shield::host_of(&url).is_some_and(|h| self.shield.is_allowed(&h));
        self.field.set_secondary_icon_name(off.then_some("security-low-symbolic"));
        self.field.set_secondary_icon_tooltip_text(off.then_some("Shield is off for this site (Ctrl+Shift+B)"));
        let base = if title.is_empty() { "rippl".to_string() } else { title };
        self.window.set_title(Some(&match engine::profile() {
            Some(p) => format!("{base} · {p}"),
            None => base,
        }));
    }

    // ---- wiring -------------------------------------------------------

    fn action(&self, app: &gtk::Application, name: &str, keys: &[&str], run: impl Fn(&Browser) + 'static) {
        let action = gio::SimpleAction::new(name, None);
        let weak = self.weak();
        let label = name.to_string();
        action.connect_activate(move |_, _| {
            debug!("action {label}");
            if let Some(b) = weak.upgrade() {
                run(&b);
            }
        });
        self.window.add_action(&action);
        app.set_accels_for_action(&format!("win.{name}"), keys);
    }

    /// One command from the control socket (see control.rs).
    pub fn command(&self, line: &str) -> String {
        let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
        let at = |n: &str| -> Option<TabId> {
            let i: usize = n.trim().parse().ok()?;
            self.tabs.borrow().list.get(i.checked_sub(1)?).map(|t| t.id)
        };
        match verb {
            "list" => {
                let tabs = self.tabs.borrow();
                let lines: Vec<String> = tabs
                    .list
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let front = if tabs.front == Some(t.id) { "front" } else { "-" };
                        // Why it can't sleep, if it can't.
                        let why = tabs::kept_awake(t, &[]).map_or("-".to_string(), |k| format!("{k:?}"));
                        format!("{}\t{}\t{:?}\t{front}\t{}\t{}\t{why}\tspace={}", i + 1, t.id, t.life, t.url, t.title, t.space)
                    })
                    .collect();
                lines.join("\n")
            }
            "open" => match address::resolve(rest, &self.config.borrow().search.clone()) {
                Destination::Page(url) => {
                    let id = self.open(&url, false, true);
                    let i = self.tabs.borrow().index(id).map_or(0, |i| i + 1);
                    format!("ok {i}")
                }
                Destination::Nothing => "error: nothing to open".into(),
            },
            "select" => match at(rest) {
                Some(id) => {
                    self.select(id);
                    "ok".into()
                }
                None => "error: no such tab".into(),
            },
            "close" => match at(rest) {
                Some(id) => {
                    self.close(id);
                    "ok".into()
                }
                None => "error: no such tab".into(),
            },
            "shield" => match rest.split_once(' ').unwrap_or((rest, "")) {
                ("", _) => self.shield.status(),
                ("update", _) => {
                    self.shield.update_now();
                    "ok: fetching".into()
                }
                ("toggle", host) if !host.is_empty() => {
                    self.shield.toggle(host.trim(), |_| {});
                    self.install_page_scripts();
                    "ok: recompiling".into()
                }
                _ => "error: shield [update | toggle <host>]".into(),
            },
            "sleep" => {
                self.sleep_idle(Duration::ZERO);
                "ok".into()
            }
            "type" => {
                self.type_text(rest);
                self.offers_text()
            }
            "offers" => self.offers_text(),
            "enter" => {
                self.field.emit_activate();
                "ok".into()
            }
            "history" => {
                let n = rest.trim().parse().unwrap_or(20);
                let rows = self.places.recent(n).unwrap_or_default();
                rows.iter().map(|p| format!("{}\t{}\t{}", p.visits, p.url, p.title)).collect::<Vec<_>>().join("\n")
            }
            "clear-history" => match self.places.clear_history(None) {
                Ok(n) => format!("ok {n}"),
                Err(e) => format!("error: {e}"),
            },
            "bookmark" => {
                self.toggle_bookmark();
                "ok".into()
            }
            "bookmarks" => {
                let rows = self.places.bookmarks().unwrap_or_default();
                rows.iter().map(|p| format!("{}\t{}", p.url, p.title)).collect::<Vec<_>>().join("\n")
            }
            "find" => {
                self.open_find();
                self.find_entry.set_text(rest);
                "ok".into()
            }
            "found" => self.found.get().to_string(),
            "permissions" => match rest.split_once(' ').unwrap_or((rest, "")) {
                ("", _) => {
                    let rows = self.places.permissions().unwrap_or_default();
                    let asking: Vec<String> =
                        self.asking.borrow().iter().map(|p| format!("asking\t{}\t{}", p.host, p.kind.key())).collect();
                    rows.iter()
                        .map(|(h, k, a)| format!("{}\t{h}\t{k}", if *a { "allow" } else { "deny" }))
                        .chain(asking)
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                ("forget", host) => {
                    let host = host.trim();
                    match self.places.forget_permissions((!host.is_empty()).then_some(host)) {
                        Ok(n) => format!("ok {n}"),
                        Err(e) => format!("error: {e}"),
                    }
                }
                ("answer", yes) => {
                    self.answer_front(yes.trim() == "allow");
                    "ok".into()
                }
                _ => "error: permissions [forget [host] | answer allow|deny]".into(),
            },
            "downloads" => self.downloads.status(),
            "save" => match rest.trim() {
                "" => match self.save_question.borrow().as_ref() {
                    Some(q) => format!("{:?}\t{}\t{}", q.ask, q.pending.origin, q.pending.capture.username),
                    None => "none".into(),
                },
                answer @ ("save" | "later" | "never") => {
                    self.answer_save(answer);
                    "ok".into()
                }
                _ => "error: save [save|later|never]".into(),
            },
            "fill" => {
                let Some(origin) =
                    self.front_view().and_then(|v| v.uri()).and_then(|u| rippl_core::passwords::origin_of(&u))
                else {
                    return "error: no page".into();
                };
                self.fill(crate::keyring::Account { origin, username: rest.trim().to_string() });
                "ok".into()
            }
            "pending" => self.pending.borrow().len().to_string(),
            "card" => {
                self.open_site_card();
                format!("shown={}", self.card.is_visible())
            }
            "dup" => {
                if let Some(id) = self.front() {
                    self.duplicate(id);
                }
                "ok".into()
            }
            "popups" => {
                let blocked = self.blocked_popups.borrow();
                let front = self.front().and_then(|f| blocked.get(&f).cloned()).unwrap_or_default();
                format!("blocked here: {front:?} | bar shown: {}", self.popup_bar.is_visible())
            }
            "popup-open" => {
                self.popup_open.emit_clicked();
                "ok".into()
            }
            "awake" => match self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone())).and_then(|u| shield::host_of(&u)) {
                Some(host) => {
                    let on = rest.trim() != "off";
                    let _ = self.places.set_site_flag(&host, crate::sitecard::AWAKE, on);
                    self.apply_keep_awake();
                    format!("{host} kept awake: {on}")
                }
                None => "error: no site".into(),
            },
            "spaces" => {
                let tabs = self.tabs.borrow();
                tabs.spaces
                    .iter()
                    .map(|s| format!("{}\t{}\t{}{}", s.id, s.name, tabs.list.iter().filter(|t| t.space == s.id).count(), if s.id == tabs.space { "\t*" } else { "" }))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            "space" => match rest.split_once(' ').unwrap_or((rest, "")) {
                ("new", name) => {
                    let id = self.tabs.borrow_mut().add_space(name);
                    self.switch_space(id);
                    format!("ok {id}")
                }
                ("delete", id) => {
                    self.delete_space(id.trim().parse().unwrap_or(u32::MAX));
                    "ok".into()
                }
                (id, _) => {
                    self.switch_space(id.trim().parse().unwrap_or(u32::MAX));
                    "ok".into()
                }
            },
            "move-to-space" => {
                let (n, space) = rest.split_once(' ').unwrap_or((rest, ""));
                match (at(n), space.trim().parse()) {
                    (Some(id), Ok(space)) => {
                        self.move_tab_to_space(id, space);
                        "ok".into()
                    }
                    _ => "error: move-to-space <tab index> <space id>".into(),
                }
            }
            "group" => {
                let (n, name) = rest.split_once(' ').unwrap_or((rest, ""));
                match at(n) {
                    Some(id) => {
                        let g = self.tabs.borrow_mut().new_group(id, name);
                        self.dirty.set(true);
                        self.render();
                        format!("ok {g:?}")
                    }
                    None => "error: no such tab".into(),
                }
            }
            "join" => {
                let (n, g) = rest.split_once(' ').unwrap_or((rest, ""));
                if let (Some(id), Ok(g)) = (at(n), g.trim().parse()) {
                    self.tabs.borrow_mut().set_group(id, Some(g));
                    self.dirty.set(true);
                    self.render();
                }
                "ok".into()
            }
            "fold" => {
                if let Ok(g) = rest.trim().parse() {
                    self.tabs.borrow_mut().toggle_group(g);
                    self.render();
                }
                "ok".into()
            }
            "groups" => {
                let tabs = self.tabs.borrow();
                tabs.groups.iter().map(|g| format!("{}\t{}\t{}\tcollapsed={}", g.id, g.space, g.name, g.collapsed)).collect::<Vec<_>>().join("\n")
            }
            "rows" => {
                // What the column shows, as text.
                let mut out = Vec::new();
                let mut child = self.list.first_child();
                while let Some(w) = child {
                    let mut text = Vec::new();
                    let mut stack = vec![w.clone()];
                    while let Some(x) = stack.pop() {
                        if let Some(l) = x.downcast_ref::<gtk::Label>() {
                            text.push(l.label().to_string());
                        }
                        let mut c = x.last_child();
                        while let Some(cc) = c {
                            stack.push(cc.clone());
                            c = cc.prev_sibling();
                        }
                    }
                    out.push(text.join(" "));
                    child = w.next_sibling();
                }
                out.join("\n")
            }
            "pins" => {
                let b = |w: &gtk::Widget| w.compute_bounds(&self.window).map(|r| format!("{}x{}", r.width(), r.height())).unwrap_or_default();
                let first = self.pins.first_child();
                format!("pins {} | first pin {}", b(self.pins.upcast_ref()), first.map(|p| b(&p)).unwrap_or_default())
            }
            "peek" => match self.front() {
                Some(id) => {
                    self.peek(rest, id);
                    "ok".into()
                }
                None => "error: no tab".into(),
            },
            "player" => match *self.player_for.borrow() {
                Some(id) if self.player_row.is_visible() => {
                    if rest == "toggle" {
                        self.player_toggle(id);
                    }
                    format!("tab {id} stilled={}", self.stilled.borrow().contains(&id))
                }
                _ => "none".into(),
            },
            "archive" => {
                self.save_archive();
                "ok".into()
            }
            "capture" => {
                self.capture();
                "ok".into()
            }
            "dusk" => match self.front_view() {
                Some(v) => {
                    let id = self.front().unwrap_or(0);
                    self.dusk(id, &v);
                    "ok".into()
                }
                None => "error: no page".into(),
            },
            "sitecss" => {
                let url = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone()));
                match url.as_deref().and_then(shield::host_of) {
                    Some(host) => {
                        let _ = self.places.set_site_css(&host, rest);
                        self.apply_site_styles();
                        "ok".into()
                    }
                    None => "error: no site".into(),
                }
            }
            "icon" => {
                let (id, url) = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| (t.id, t.url.clone()))).unwrap_or_default();
                self.icon_for(id, &url).map_or("none".into(), |t| format!("{}x{}", t.width(), t.height()))
            }
            "reopen" => {
                self.reopen_closed();
                "ok".into()
            }
            "split" => match at(rest) {
                Some(id) => {
                    self.split_with(id);
                    "ok".into()
                }
                None => {
                    self.toggle_split();
                    "ok".into()
                }
            },
            "shown" => match *self.shown_split.borrow() {
                Some((l, r)) => format!("split {l} {r} front {}", self.front().unwrap_or(0)),
                None => format!("single {}", self.front().unwrap_or(0)),
            },
            "allow-http" => {
                self.engine.allow_http(rest.trim());
                "ok".into()
            }
            "pane" => {
                self.focus_pane(rest.trim() == "right");
                "ok".into()
            }
            "popover" => self.accounts_popover.is_visible().to_string(),
            "pin" => {
                self.toggle_pin();
                "ok".into()
            }
            "reader" => {
                self.toggle_reader();
                "ok".into()
            }
            "pick" => {
                self.pick_to_hide();
                "ok".into()
            }
            "hide" => {
                let url = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone()));
                match url.as_deref().and_then(shield::host_of) {
                    Some(host) => {
                        self.keep_hidden(&host, rest.trim());
                        "ok".into()
                    }
                    None => "error: no site in front".into(),
                }
            }
            "unhide" => {
                self.unhide_here();
                "ok".into()
            }
            "hidden" => {
                let all = self.places.hidden().unwrap_or_default();
                all.iter().map(|(h, s)| format!("{h}\t{}", s.join(" | "))).collect::<Vec<_>>().join("\n")
            }
            "sources" => {
                let found = rippl_core::import::sources(&glib::home_dir());
                found.iter().enumerate().map(|(i, s)| format!("{i}\t{}\t{}", s.browser, s.profile)).collect::<Vec<_>>().join("\n")
            }
            "import" => match self.import_from(rest.trim().parse().unwrap_or(usize::MAX)) {
                Ok(note) => note,
                Err(e) => format!("error: {e}"),
            },
            "download" => match self.engine.session.download_uri(rest.trim()) {
                Some(_) => "ok".into(),
                None => "error: could not start".into(),
            },
            _ => format!("error: unknown command {verb:?}"),
        }
    }

    /// A control-socket command whose answer may come later: `js <code>`
    /// runs script in the front page and answers with its result as JSON.
    pub fn command_with(&self, line: &str, reply: Box<dyn FnOnce(String)>) {
        // Keyring commands answer when the keyring does.
        if line == "passwords" || line == "accounts" {
            let origin = if line == "accounts" {
                self.front_view().and_then(|v| v.uri()).and_then(|u| rippl_core::passwords::origin_of(&u))
            } else {
                None
            };
            let keyring = self.keyring.clone();
            glib::MainContext::default().spawn_local(async move {
                let list = keyring.accounts(origin.as_deref()).await;
                reply(match list {
                    Ok(list) => list.iter().map(|a| format!("{}\t{}", a.origin, a.username)).collect::<Vec<_>>().join("\n"),
                    Err(e) => format!("error: {e}"),
                });
            });
            return;
        }
        if let Some(rest) = line.strip_prefix("passwords delete ") {
            let (origin, username) = rest.split_once(' ').unwrap_or((rest, ""));
            let (origin, username) = (origin.to_string(), username.to_string());
            let keyring = self.keyring.clone();
            glib::MainContext::default().spawn_local(async move {
                reply(match keyring.delete(&origin, &username).await {
                    Ok(()) => "ok".into(),
                    Err(e) => format!("error: {e}"),
                });
            });
            return;
        }
        let Some(code) = line.strip_prefix("js ") else { return reply(self.command(line)) };
        let Some(view) = self.front_view() else { return reply("error: no page".into()) };
        view.evaluate_javascript(code, None, None, None::<&gio::Cancellable>, move |result| {
            reply(match result {
                Ok(v) => v.to_json(0).map(|j| j.to_string()).unwrap_or_else(|| "undefined".into()),
                Err(e) => format!("error: {e}"),
            })
        });
    }

    /// Goes to rippl://`page`: the tab already showing it, or a new one.
    pub(crate) fn open_internal(&self, page: &str) {
        let url = format!("rippl://{page}");
        let existing = self.tabs.borrow().list.iter().find(|t| t.url.starts_with(&url)).map(|t| t.id);
        match existing {
            Some(id) => {
                self.select(id);
                if let Some(v) = self.front_view() {
                    v.reload();
                }
            }
            None => {
                self.open(&url, false, true);
            }
        }
    }

    /// Ctrl+Shift+P: pin the tab in front, or unpin it.
    pub(crate) fn toggle_pin(&self) {
        let Some(id) = self.front() else { return };
        {
            let mut tabs = self.tabs.borrow_mut();
            let pinned = tabs.get(id).is_some_and(|t| t.pinned);
            tabs.set_pinned(id, !pinned);
        }
        self.dirty.set(true);
        self.render();
    }

    /// A short message over the page, gone after three seconds.
    pub(crate) fn notice(&self, text: &str) {
        self.toast.set_label(text);
        self.toast.set_visible(true);
        let generation = self.toast_gen.get().wrapping_add(1);
        self.toast_gen.set(generation);
        let weak = self.weak();
        glib::timeout_add_local_once(Duration::from_secs(3), move || {
            if let Some(b) = weak.upgrade()
                && b.toast_gen.get() == generation
            {
                b.toast.set_visible(false);
            }
        });
    }

    fn wire_actions(&self, app: &gtk::Application) {
        self.action(app, "focus-field", &["<Ctrl>l", "F6", "<Alt>d"], |b| {
            b.field.grab_focus();
            b.field.select_region(0, -1);
        });
        self.action(app, "new-tab", &["<Ctrl>t"], |b| {
            b.open("", false, true);
        });
        self.action(app, "private-tab", &["<Ctrl><Shift>n"], |b| {
            b.open("", true, true);
        });
        self.action(app, "close-tab", &["<Ctrl>w", "<Ctrl>F4"], |b| {
            if let Some(id) = b.front() {
                b.close(id);
            }
        });
        self.action(app, "next-tab", &["<Ctrl>Tab", "<Ctrl>Page_Down"], |b| {
            if let Some(id) = b.tabs.borrow().neighbour(1) {
                glib::idle_add_local_once({
                    let w = b.weak();
                    move || {
                        if let Some(b) = w.upgrade() {
                            b.select(id)
                        }
                    }
                });
            }
        });
        self.action(app, "prev-tab", &["<Ctrl><Shift>Tab", "<Ctrl><Shift>ISO_Left_Tab", "<Ctrl>Page_Up"], |b| {
            if let Some(id) = b.tabs.borrow().neighbour(-1) {
                glib::idle_add_local_once({
                    let w = b.weak();
                    move || {
                        if let Some(b) = w.upgrade() {
                            b.select(id)
                        }
                    }
                });
            }
        });
        self.action(app, "back", &["<Alt>Left", "<Ctrl>bracketleft"], |b| {
            if let Some(v) = b.front_view() {
                v.go_back();
            }
        });
        self.action(app, "forward", &["<Alt>Right", "<Ctrl>bracketright"], |b| {
            if let Some(v) = b.front_view() {
                v.go_forward();
            }
        });
        self.action(app, "reload", &["<Ctrl>r", "F5"], |b| {
            if let Some(v) = b.front_view() {
                v.reload();
            }
        });
        self.action(app, "reload-hard", &["<Ctrl><Shift>r", "<Shift>F5"], |b| {
            if let Some(v) = b.front_view() {
                v.reload_bypass_cache();
            }
        });
        // Shield off (or back on) for the site in front, then reload it.
        self.action(app, "shield-toggle", &["<Ctrl><Shift>b"], |b| {
            let url = b.front().and_then(|f| b.tabs.borrow().get(f).map(|t| t.url.clone()));
            let Some(host) = url.as_deref().and_then(shield::host_of) else { return };
            b.toggle_shield_for(&host);
        });
        self.action(app, "find", &["<Ctrl>f"], |b| b.open_find());
        self.action(app, "find-next", &["<Ctrl>g", "F3"], |b| {
            if let Some(f) = b.front_view().and_then(|v| v.find_controller()) {
                f.search_next();
            }
        });
        self.action(app, "find-previous", &["<Ctrl><Shift>g", "<Shift>F3"], |b| {
            if let Some(f) = b.front_view().and_then(|v| v.find_controller()) {
                f.search_previous();
            }
        });
        self.action(app, "bookmark", &["<Ctrl>d"], |b| b.toggle_bookmark());
        self.action(app, "history", &["<Ctrl>h"], |b| b.open_internal("history"));
        self.action(app, "bookmarks", &["<Ctrl><Shift>o"], |b| b.open_internal("bookmarks"));
        self.action(app, "settings", &["<Ctrl>comma"], |b| b.open_internal("settings"));
        self.action(app, "pin", &["<Ctrl><Shift>p"], |b| b.toggle_pin());
        self.action(app, "split", &["<Ctrl><Alt>s"], |b| b.toggle_split());
        self.action(app, "reopen", &["<Ctrl><Shift>t"], |b| b.reopen_closed());
        self.action(app, "site-card", &["<Ctrl>i"], |b| b.open_site_card());
        self.action(app, "duplicate", &["<Ctrl><Shift>d"], |b| {
            if let Some(id) = b.front() {
                b.duplicate(id);
            }
        });
        self.action(app, "print", &["<Ctrl>p"], |b| b.print());
        self.action(app, "capture", &["<Ctrl><Shift>s"], |b| b.capture());
        self.action(app, "save-archive", &["<Ctrl><Alt>a"], |b| b.save_archive());
        self.action(app, "copy-address", &["<Ctrl><Shift>c"], |b| b.copy_address());
        self.action(app, "paste-go", &["<Ctrl><Shift>v"], |b| b.paste_and_go());
        // Ctrl+K: the field, where typing finds open tabs.
        self.action(app, "switch-tab", &["<Ctrl>k"], |b| {
            b.field.grab_focus();
            b.field.set_text("");
        });
        self.action(app, "pane-left", &["<Ctrl><Alt>Left"], |b| b.focus_pane(false));
        self.action(app, "pane-right", &["<Ctrl><Alt>Right"], |b| b.focus_pane(true));
        self.action(app, "passwords", &["<Ctrl><Alt>p"], |b| b.open_internal("passwords"));
        self.action(app, "reader", &["F9", "<Ctrl><Alt>r"], |b| b.toggle_reader());
        self.action(app, "hide-element", &["<Ctrl><Shift>h"], |b| b.pick_to_hide());
        self.action(app, "unhide", &["<Ctrl><Shift>u"], |b| b.unhide_here());
        self.action(app, "toggle-column", &["<Ctrl>s"], |b| {
            b.column.set_visible(!b.column.is_visible());
        });
        self.action(app, "inspector", &["<Ctrl><Shift>i", "F12"], |b| {
            if let Some(inspector) = b.front_view().and_then(|v| v.inspector()) {
                inspector.show();
            }
        });
        self.action(app, "zoom-in", &["<Ctrl>plus", "<Ctrl>equal"], |b| {
            if let Some(v) = b.front_view() {
                v.set_zoom_level((v.zoom_level() + 0.1).min(3.0));
            }
        });
        self.action(app, "zoom-out", &["<Ctrl>minus"], |b| {
            if let Some(v) = b.front_view() {
                v.set_zoom_level((v.zoom_level() - 0.1).max(0.3));
            }
        });
        self.action(app, "zoom-reset", &["<Ctrl>0"], |b| {
            if let Some(v) = b.front_view() {
                v.set_zoom_level(1.0);
            }
        });
        // Alt+1 … Alt+9: the spaces, in order.
        for n in 1..=9usize {
            self.action(app, &format!("space-{n}"), &[&format!("<Alt>{n}")], move |b| {
                let id = b.tabs.borrow().spaces.get(n - 1).map(|s| s.id);
                if let Some(id) = id {
                    b.switch_space(id);
                }
            });
        }
        self.action(app, "new-space", &["<Ctrl><Alt>t"], |b| b.new_space());
        // Ctrl+1 … Ctrl+8 go to that tab, Ctrl+9 to the last.
        for n in 1..=9usize {
            self.action(app, &format!("tab-{n}"), &[&format!("<Ctrl>{n}")], move |b| {
                let id = {
                    let tabs = b.tabs.borrow();
                    let here: Vec<TabId> = tabs.here().map(|t| t.id).collect();
                    if n == 9 { here.last().copied() } else { here.get(n - 1).copied() }
                };
                if let Some(id) = id {
                    b.select(id);
                }
            });
        }
    }

    fn wire_window(&self) {
        let weak = self.weak();
        self.window.connect_close_request(move |_| {
            if let Some(b) = weak.upgrade() {
                b.save();
            }
            glib::Propagation::Proceed
        });
        // Killed politely (logout, `pkill`): save what is open first.
        let weak = self.weak();
        glib_unix::unix_signal_add_local(libc::SIGTERM, move || {
            if let Some(b) = weak.upgrade() {
                b.save();
                b.window.application().inspect(|a| a.quit());
            }
            glib::ControlFlow::Break
        });
        // Out of focus: hand freed heap back to the system, a moment later.
        self.window.connect_is_active_notify(|w| {
            if !w.is_active() {
                glib::timeout_add_local_once(Duration::from_secs(2), pressure::trim);
            }
        });
    }

    /// The sleep timer, the memory-pressure check, and the session saver.
    fn watch(&self) {
        let every = (self.config.borrow().sleep_after / 4).clamp(Duration::from_secs(5), Duration::from_secs(30));
        let weak = self.weak();
        glib::timeout_add_local(every, move || {
            let Some(b) = weak.upgrade() else { return glib::ControlFlow::Break };
            let chosen = b.config.borrow().sleep_after;
            let pressure = pressure::read();
            b.sleep_idle(tabs::sleep_wait(chosen, pressure));
            if pressure == Pressure::Critical {
                pressure::trim();
            }
            if b.dirty.replace(false) {
                b.save();
            }
            glib::ControlFlow::Continue
        });
    }

    /// HTTPS-only: a main-frame load started. If it is an upgraded http
    /// address and the site hasn't answered in 8 seconds, stop waiting for
    /// the connection to time out and say why.
    fn watch_upgrade(&self, id: TabId, view: &WebView) {
        let Some(started) = view.uri().map(|u| u.to_string()) else { return };
        // The view still says the http address it asked for; the page
        // process asks for the https one.
        let uri = address::https_upgrade(&started).unwrap_or(started);
        let upgraded = self.upgrades_seen.borrow().get(&id).is_some_and(|l| l.contains(&uri));
        debug!("load started {uri}: upgraded={upgraded}");
        if !upgraded || !self.config.borrow().https_only {
            return;
        }
        self.upgrading.borrow_mut().insert(id, uri.clone());
        let weak = self.weak();
        let view = view.clone();
        glib::timeout_add_local_once(Duration::from_secs(8), move || {
            let Some(b) = weak.upgrade() else { return };
            // Still waiting: no response yet (Committed, Finished and
            // failures all clear the entry).
            let still = b.upgrading.borrow().get(&id) == Some(&uri) && b.ours(id, &view);
            debug!("upgrade timer {uri}: still waiting={still}");
            if still {
                b.upgrading.borrow_mut().remove(&id);
                view.stop_loading();
                b.https_fallback(&view, &uri, "The secure version of the site didn't answer within 8 seconds.");
            }
        });
    }

    /// HTTPS-only: `failing` (an https address) didn't load. If it was an
    /// upgrade of a public http address, show why and offer http.
    pub(crate) fn https_fallback(&self, view: &WebView, failing: &str, reason: &str) -> bool {
        if !self.config.borrow().https_only || !self.engine.extension {
            return false;
        }
        let Some(rest) = failing.strip_prefix("https://") else { return false };
        let http = format!("http://{rest}");
        if address::https_upgrade(&http).is_none() {
            return false; // a local address, never upgraded
        }
        // The host exactly as the extension compares it (www. kept).
        let Some(origin) = rippl_core::passwords::origin_of(&http) else { return false };
        let host = rippl_core::passwords::host_of_origin(&origin).to_string();
        self.https_failed.borrow_mut().insert(host.clone());
        let html = rippl_core::pages::https_failed(&host, &http, reason);
        view.load_alternate_html(&html, failing, Some(failing));
        true
    }

    /// Bookmarks the page in front, or un-bookmarks it.
    pub(crate) fn toggle_bookmark(&self) {
        let (url, title, private) = {
            let tabs = self.tabs.borrow();
            let Some(tab) = tabs.front.and_then(|f| tabs.get(f)) else { return };
            (tab.url.clone(), tab.title.clone(), tab.private)
        };
        if url.is_empty() || private {
            return;
        }
        if let Err(e) = self.places.toggle_bookmark(&url, &title) {
            eprintln!("rippl: bookmarks: {e}");
        }
        self.show_front();
    }

    fn sleep_idle(&self, wait: Duration) {
        let on_screen = self.on_screen();
        let due = {
            let tabs = self.tabs.borrow();
            tabs::due_for_sleep(&tabs.list, &on_screen, Instant::now(), wait)
        };
        for id in due {
            self.sleep(id);
        }
    }

    // ---- session ------------------------------------------------------

    fn session_path() -> std::path::PathBuf {
        engine::data_dir().join("session")
    }

    fn save(&self) {
        let text = Session::of(&self.tabs.borrow()).write();
        let path = Self::session_path();
        let tmp = path.with_extension("tmp");
        let result = std::fs::create_dir_all(engine::data_dir())
            .and_then(|_| std::fs::write(&tmp, text))
            .and_then(|_| std::fs::rename(&tmp, &path));
        if let Err(e) = result {
            eprintln!("rippl: could not save the session: {e}");
        }
    }

    /// Last session's tabs come back Lazy: only the one in front gets a
    /// view. A URL given on the command line opens after them.
    fn restore(&self) {
        let session = std::fs::read_to_string(Self::session_path()).map(|t| Session::read(&t)).unwrap_or_default();
        let front = session.restore_into(&mut self.tabs.borrow_mut());
        // URLs from the command line (not `--profile` and its name).
        let mut args = std::env::args().skip(1);
        let mut asked = Vec::new();
        while let Some(a) = args.next() {
            if a == "--profile" {
                args.next();
            } else if !a.starts_with('-') {
                asked.push(a);
            }
        }
        if let Some(front) = front.filter(|_| asked.is_empty()) {
            self.select(front);
        }
        for (i, a) in asked.iter().enumerate() {
            if let Destination::Page(url) = address::resolve(a, &self.config.borrow().search.clone()) {
                self.open(&url, false, i == 0);
            }
        }
        if self.tabs.borrow().list.is_empty() {
            self.open("", false, true);
        }
        self.apply_keep_awake();
        self.render();
    }

}
