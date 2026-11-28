//! rippl's commands, by name (commands.rs has their keys), and the native
//! menus: a tab's, a group's, a space's, and the site card.

use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use rippl_core::tabs::TabId;

use crate::browser::Browser;
use crate::platform;

impl Browser {
    /// Runs after this event is done (nothing borrowed then).
    pub fn after(&self, f: impl FnOnce(&Browser) + 'static) {
        self.pending.borrow_mut().push(Box::new(f));
    }

    pub fn run_pending(&self) {
        loop {
            let next: Vec<_> = self.pending.borrow_mut().drain(..).collect();
            if next.is_empty() {
                break;
            }
            for f in next {
                f(self);
            }
        }
    }

    pub fn command(&self, name: &str, repeat: bool) {
        let front = self.front();
        let view_do = |f: &dyn Fn(&wry::WebView)| {
            if let Some(id) = front
                && let Some(v) = self.views.borrow().get(&id)
            {
                f(v);
            }
        };
        match name {
            "focus-field" => self.eval_chrome_pub("rippl.focusField()"),
            "new-tab" => {
                self.open("", false, false);
            }
            "private-tab" => {
                self.open("", true, false);
                self.notice("A private tab: nothing it does is kept");
            }
            "new-window" => self.spawn_window(),
            "close-tab" => {
                if !repeat && let Some(id) = front {
                    self.close(id);
                }
            }
            "reopen" => {
                let last = self.closed.borrow_mut().pop();
                if let Some((url, _)) = last {
                    self.open(&url, false, false);
                }
            }
            "next-tab" | "prev-tab" => {
                let step = if name == "next-tab" { 1 } else { -1 };
                let next = self.tabs.borrow().neighbour(step);
                if let Some(n) = next {
                    self.select(n);
                }
            }
            "back" => view_do(&|v| {
                let _ = v.evaluate_script("history.back()");
            }),
            "forward" => view_do(&|v| {
                let _ = v.evaluate_script("history.forward()");
            }),
            "reload" => view_do(&|v| {
                let _ = v.reload();
            }),
            "reload-hard" => view_do(&|v| {
                let _ = v.evaluate_script("location.reload()");
            }),
            "stop" => view_do(&|v| {
                let _ = v.evaluate_script("window.stop()");
            }),
            "find" => {
                if front.is_some_and(|id| self.views.borrow().contains_key(&id)) {
                    *self.find.borrow_mut() = Some((String::new(), String::new()));
                    self.render_due.set(true);
                    self.after(|b| b.eval_chrome_pub("rippl.find(true)"));
                }
            }
            "find-next" | "find-previous" => {
                let text = self.find.borrow().as_ref().map(|f| f.0.clone());
                if let Some(text) = text {
                    let js = serde_json::to_string(&text).unwrap_or_default();
                    self.front_view_eval(&format!("window.__rippl && __rippl.find({js}, {})", name == "find-previous"));
                }
            }
            "bookmark" => self.toggle_bookmark(),
            "history" => self.open_internal("history"),
            "downloads" => self.open_internal("downloads"),
            "bookmarks" => self.open_internal("bookmarks"),
            "settings" => self.open_internal("settings"),
            "passwords" => self.open_internal("passwords"),
            "pin" => {
                if let Some(id) = front {
                    let pinned = self.tabs.borrow().get(id).is_some_and(|t| t.pinned);
                    self.tabs.borrow_mut().set_pinned(id, !pinned);
                    self.dirty.set(true);
                    self.render_due.set(true);
                }
            }
            "duplicate" => {
                let url = self.front_url();
                if !url.is_empty() {
                    let private = front.is_some_and(|id| self.is_private(id));
                    self.open(&url, private, false);
                }
            }
            "group-site" => self.group_site_front(),
            "shield-toggle" => {
                let url = self.front_url();
                if let Some(host) = crate::shield::host_of(&url) {
                    let off = !self.shield.is_allowed(&host);
                    self.set_shield_for(&host, off);
                }
            }
            "site-card" => self.site_card(-1.0, -1.0),
            "print" => view_do(&|v| {
                let _ = v.print();
            }),
            "copy-address" => {
                let url = rippl_core::clean::clean_link(&platform::from_engine(&self.front_url()));
                if !url.is_empty() {
                    crate::clipboard::set(&url);
                    self.notice("Address copied, without its tracking tags");
                }
            }
            "paste-go" => {
                if let Some(text) = crate::clipboard::get() {
                    self.go(&text, false, false);
                }
            }
            "toggle-column" => {
                self.column_hidden.set(!self.column_hidden.get());
                self.render_due.set(true);
            }
            "inspector" => view_do(&|v| {
                if v.is_devtools_open() {
                    v.close_devtools();
                } else {
                    v.open_devtools();
                }
            }),
            "zoom-in" => self.set_zoom(Some(0.1)),
            "zoom-out" => self.set_zoom(Some(-0.1)),
            "zoom-reset" => self.set_zoom(None),
            "new-space" => {
                let n = self.tabs.borrow().spaces.len() + 1;
                let space = self.tabs.borrow_mut().add_space(&format!("Space {n}"));
                self.switch_space(space);
            }
            "fullscreen" => {
                let full = self.window.fullscreen().is_some();
                self.window.set_fullscreen(if full { None } else { Some(tao::window::Fullscreen::Borderless(None)) });
            }
            "escape" => {
                if self.find.borrow().is_some() {
                    self.close_find();
                } else if self.window.fullscreen().is_some() {
                    self.window.set_fullscreen(None);
                }
            }
            "switch-tab" => self.eval_chrome_pub("rippl.focusField()"),
            "quit" => self.quit(),
            "capture" => self.capture(),
            "reader" => self.toggle_reader(),
            "hide-element" => self.pick_to_hide(),
            "unhide" => self.unhide_here(),
            "grab" => self.open_grab_bar(),
            "ask-page" | "translate" => self.notice("The AI add-on is set up in Settings › AI"),
            "split" => self.toggle_split(),
            "pane-left" | "pane-right" => {}
            n if n.starts_with("tab-") => {
                let i: usize = n[4..].parse().unwrap_or(1);
                let id = {
                    let tabs = self.tabs.borrow();
                    let here: Vec<TabId> = tabs.here().map(|t| t.id).collect();
                    if i == 9 { here.last().copied() } else { here.get(i - 1).copied() }
                };
                if let Some(id) = id {
                    self.select(id);
                }
            }
            n if n.starts_with("space-") => {
                let i: usize = n[6..].parse().unwrap_or(1);
                let space = self.tabs.borrow().spaces.get(i - 1).map(|s| s.id);
                if let Some(s) = space {
                    self.switch_space(s);
                }
            }
            other => eprintln!("rippl: no command {other}"),
        }
    }

    pub fn open_internal(&self, page: &str) {
        let url = format!("rippl://{page}");
        // The page if it's open already, else in a new tab (or this one, if blank).
        let open = self.tabs.borrow().here().find(|t| rippl_core::pages::page_name(&t.url) == page && t.url.starts_with("rippl://")).map(|t| t.id);
        if let Some(id) = open {
            self.select(id);
            if let Some(v) = self.views.borrow().get(&id) {
                let _ = v.reload();
            }
            return;
        }
        let blank = self.front().and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.is_empty())).unwrap_or(false);
        if blank && let Some(id) = self.front() {
            self.load(id, &url);
            self.select(id);
        } else {
            self.open(&url, false, false);
        }
    }

    pub fn toggle_bookmark(&self) {
        let Some(id) = self.front() else { return };
        let (url, title) = match self.tabs.borrow().get(id) {
            Some(t) => (t.url.clone(), t.title.clone()),
            None => return,
        };
        if url.is_empty() {
            return;
        }
        match self.places.toggle_bookmark(&url, &title) {
            Ok(true) => self.notice("Bookmarked"),
            Ok(false) => self.notice("Bookmark removed"),
            Err(e) => self.notice(&format!("That didn't work: {e}")),
        }
        self.render_due.set(true);
    }

    pub fn set_shield_for(&self, host: &str, off: bool) {
        self.shield.set_allowed(host, off);
        self.reapply_shield();
        self.notice(&if off { format!("Shield is off for {host}") } else { format!("Shield is on for {host}") });
        // The page again, with its rules as they are now.
        let ids: Vec<TabId> = self
            .tabs
            .borrow()
            .list
            .iter()
            .filter(|t| crate::shield::host_of(&t.url).as_deref() == Some(host))
            .map(|t| t.id)
            .collect();
        for id in ids {
            if let Some(v) = self.views.borrow().get(&id) {
                let _ = v.reload();
            }
        }
        self.render_due.set(true);
    }

    pub fn reapply_shield(&self) {
        let tabs = self.tabs.borrow();
        for (id, v) in self.views.borrow().iter() {
            let url = tabs.get(*id).map(|t| t.url.clone()).unwrap_or_default();
            self.shield.follow(v, &url);
        }
    }

    /// Your hidden elements and your CSS for the page's site.
    pub fn style_page(&self, id: TabId, url: &str) {
        let Some(host) = crate::shield::host_of(url) else { return };
        let mut css = String::new();
        if let Ok(hidden) = self.places.hidden() {
            for (h, selectors) in hidden {
                if h == host {
                    for s in selectors {
                        css.push_str(&format!("{s} {{ display: none !important; }}\n"));
                    }
                }
            }
        }
        if let Ok(Some(mine)) = self.places.site_css(&host) {
            css.push_str(&mine);
        }
        if css.is_empty() {
            return;
        }
        let js = format!(
            "(() => {{ let s = document.getElementById('rippl-site-style'); if (!s) {{ s = document.createElement('style'); s.id = 'rippl-site-style'; (document.head || document.documentElement).appendChild(s); }} s.textContent = {}; }})()",
            serde_json::to_string(&css).unwrap_or_default()
        );
        if let Some(v) = self.views.borrow().get(&id) {
            let _ = v.evaluate_script(&js);
        }
    }

    pub fn apply_site_styles(&self) {
        let pairs: Vec<(TabId, String)> = self.tabs.borrow().list.iter().map(|t| (t.id, t.url.clone())).collect();
        for (id, url) in pairs {
            if self.views.borrow().contains_key(&id) {
                self.style_page(id, &url);
            }
        }
    }

    // ---- menus ----------------------------------------------------------

    fn item(&self, menu: &Menu, label: &str, f: impl Fn(&Browser) + 'static) {
        let item = MenuItem::new(label, true, None);
        self.menu_actions.borrow_mut().insert(item.id().0.clone(), Box::new(f));
        let _ = menu.append(&item);
    }

    fn sep(menu: &Menu) {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }

    fn show(&self, menu: &Menu, x: f64, y: f64) {
        let pos = (x >= 0.0).then(|| muda::dpi::Position::Logical(muda::dpi::LogicalPosition::new(x, y)));
        crate::menus::show(menu, &self.window, pos);
    }

    pub fn tab_menu(&self, id: TabId, x: f64, y: f64) {
        self.menu_actions.borrow_mut().clear();
        let (pinned, url, group) = match self.tabs.borrow().get(id) {
            Some(t) => (t.pinned, t.url.clone(), t.group),
            None => return,
        };
        let menu = Menu::new();
        self.item(&menu, if pinned { "Unpin" } else { "Pin" }, move |b| {
            b.tabs.borrow_mut().set_pinned(id, !pinned);
            b.dirty.set(true);
            b.render_due.set(true);
        });
        self.item(&menu, "Duplicate", move |b| {
            let url = b.tabs.borrow().get(id).map(|t| t.url.clone()).unwrap_or_default();
            if !url.is_empty() {
                b.open(&url, b.is_private(id), false);
            }
        });
        self.item(&menu, "Reload", move |b| {
            if let Some(v) = b.views.borrow().get(&id) {
                let _ = v.reload();
            }
        });
        if !url.is_empty() {
            self.item(&menu, "Copy address", move |b| {
                let url = b.tabs.borrow().get(id).map(|t| t.url.clone()).unwrap_or_default();
                crate::clipboard::set(&rippl_core::clean::clean_link(&url));
                b.notice("Address copied");
            });
        }
        Self::sep(&menu);
        if let Some(site) = crate::shield::host_of(&url) {
            self.item(&menu, &format!("Group tabs from {site}"), move |b| {
                let r = b.tabs.borrow_mut().group_site(id);
                let _ = r;
                b.dirty.set(true);
                b.render_due.set(true);
            });
        }
        if group.is_some() {
            self.item(&menu, "Take out of its group", move |b| {
                b.tabs.borrow_mut().set_group(id, None);
                b.dirty.set(true);
                b.render_due.set(true);
            });
        } else {
            self.item(&menu, "New group with this tab", move |b| {
                let n = b.tabs.borrow().groups.len() + 1;
                b.tabs.borrow_mut().new_group(id, &format!("Group {n}"));
                b.dirty.set(true);
                b.render_due.set(true);
            });
        }
        let spaces: Vec<(u32, String)> = self.tabs.borrow().spaces.iter().map(|s| (s.id, s.name.clone())).collect();
        if spaces.len() > 1 {
            let sub = Submenu::new("Move to space", true);
            for (sid, sname) in spaces {
                let item = MenuItem::new(&sname, true, None);
                self.menu_actions.borrow_mut().insert(
                    item.id().0.clone(),
                    Box::new(move |b: &Browser| {
                        b.tabs.borrow_mut().move_to_space(id, sid);
                        if b.front() == Some(id) || b.tabs.borrow().front.is_none() {
                            let next = b.tabs.borrow().here().next().map(|t| t.id);
                            match next {
                                Some(n) => b.select(n),
                                None => {
                                    let n = b.tabs.borrow_mut().open("");
                                    b.select(n);
                                }
                            }
                        }
                        b.dirty.set(true);
                        b.render_due.set(true);
                    }),
                );
                let _ = sub.append(&item);
            }
            let _ = menu.append(&sub);
        }
        Self::sep(&menu);
        self.item(&menu, "Close tabs below", move |b| {
            let below: Vec<TabId> = {
                let tabs = b.tabs.borrow();
                let here: Vec<TabId> = tabs.here().filter(|t| !t.pinned).map(|t| t.id).collect();
                match here.iter().position(|t| *t == id) {
                    Some(i) => here[i + 1..].to_vec(),
                    None => Vec::new(),
                }
            };
            for t in below {
                b.close(t);
            }
        });
        self.item(&menu, "Close other tabs", move |b| {
            let others: Vec<TabId> = b.tabs.borrow().here().filter(|t| t.id != id && !t.pinned).map(|t| t.id).collect();
            for t in others {
                b.close(t);
            }
        });
        self.item(&menu, "Close", move |b| b.close(id));
        self.show(&menu, x, y);
    }

    pub fn group_menu(&self, group: u32, x: f64, y: f64) {
        self.menu_actions.borrow_mut().clear();
        let menu = Menu::new();
        self.item(&menu, "Fold or unfold", move |b| {
            b.tabs.borrow_mut().toggle_group(group);
            b.render_due.set(true);
        });
        self.item(&menu, "Ungroup", move |b| {
            b.tabs.borrow_mut().ungroup(group);
            b.dirty.set(true);
            b.render_due.set(true);
        });
        self.item(&menu, "Close the group's tabs", move |b| {
            let ids: Vec<TabId> = b.tabs.borrow().list.iter().filter(|t| t.group == Some(group)).map(|t| t.id).collect();
            for t in ids {
                b.close(t);
            }
        });
        self.show(&menu, x, y);
    }

    pub fn space_menu(&self, space: u32, x: f64, y: f64) {
        self.menu_actions.borrow_mut().clear();
        let menu = Menu::new();
        let colours = [("Blue", "#4285f5"), ("Green", "#33a873"), ("Orange", "#f59e33"), ("Red", "#e65466"), ("Purple", "#9e66e6")];
        let sub = Submenu::new("Colour", true);
        for (name, hex) in colours {
            let item = MenuItem::new(name, true, None);
            self.menu_actions.borrow_mut().insert(
                item.id().0.clone(),
                Box::new(move |b: &Browser| {
                    b.tabs.borrow_mut().colour_space(space, &[hex.to_string()]);
                    b.dirty.set(true);
                    b.render_due.set(true);
                }),
            );
            let _ = sub.append(&item);
        }
        let _ = menu.append(&sub);
        if self.tabs.borrow().spaces.len() > 1 {
            self.item(&menu, "Delete space (its tabs close)", move |b| {
                let closing = b.tabs.borrow_mut().remove_space(space);
                for id in closing {
                    b.views.borrow_mut().remove(&id);
                }
                let front = b.tabs.borrow().front;
                match front {
                    Some(f) => b.select(f),
                    None => {
                        let n = b.tabs.borrow_mut().open("");
                        b.select(n);
                    }
                }
                b.dirty.set(true);
                b.render_due.set(true);
            });
        }
        self.show(&menu, x, y);
    }

    /// The site card, as a menu: what the site may do, and Shield.
    pub fn site_card(&self, x: f64, y: f64) {
        self.menu_actions.borrow_mut().clear();
        let url = self.front_url();
        let Some(host) = crate::shield::host_of(&url) else { return };
        let menu = Menu::new();
        let secure = url.starts_with("https://") || url.starts_with("rippl://");
        let _ = menu.append(&MenuItem::new(&host, false, None));
        let _ = menu.append(&MenuItem::new(if secure { "Connection is secure" } else { "Connection is not secure" }, false, None));
        Self::sep(&menu);
        let off = self.shield.is_allowed(&host);
        let h = host.clone();
        self.item(&menu, if off { "Turn Shield on for this site" } else { "Turn Shield off for this site" }, move |b| b.set_shield_for(&h, !off));
        let h = host.clone();
        self.item(&menu, "Forget what this site may do", move |b| {
            let _ = b.places.forget_permissions(Some(&h));
            b.notice("Forgotten");
        });
        let h = host.clone();
        self.item(&menu, "Show hidden elements again", move |b| {
            let _ = b.places.unhide(Some(&h));
            b.apply_site_styles();
        });
        self.show(&menu, x, y);
    }
}
