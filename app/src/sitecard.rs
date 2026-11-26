//! The site card, the tab menu, duplicate, print and blocked pop-ups
//!.
//!
//! **Site card** (a click on the tab you're on, or Ctrl+I): whether the
//! connection is private, Copy address, Print, the page's zoom, and the
//! site's switches: blocking ads and trackers, keeping the site awake,
//! allowing its pop-ups. Below them, what the site was allowed or denied
//! and what is hidden on it, each with a way back.
//!
//! **Tab menu** (right-click a tab): Duplicate, Pin, Put beside this tab,
//! Copy address, Reload, Close, Close other tabs.
//!
//! **Blocked pop-ups**: a page may open a window only right after a click
//! or a key (WebKit's user gesture). One it opens on its own is stopped,
//! and a bar says so, with Open (as a tab of its own) and Always allow for
//! the site. Nothing opens by itself from the bar.

use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{gtk, PrintOperation};

use crate::browser::Browser;
use crate::shield::host_of;

pub const AWAKE: &str = "awake";

/// One line of the tab menu: its words, and what it does to the tab.
type MenuItem = (&'static str, fn(&Browser, TabId));

/// A menu line built at the time: its words, and what it does.
type MenuAction = (String, std::rc::Rc<dyn Fn(&Browser, TabId)>);
pub const POPUPS: &str = "popups";

impl Browser {
    /// The popover the card is drawn in, hanging under the field.
    pub(crate) fn card_popover() -> gtk::Popover {
        let popover = gtk::Popover::builder().has_arrow(false).position(gtk::PositionType::Bottom).build();
        popover.add_css_class("card");
        popover
    }

    /// Ctrl+I, or a click on the tab in front.
    pub(crate) fn open_site_card(&self) {
        let Some(id) = self.front() else { return };
        let (url, private) = {
            let tabs = self.tabs.borrow();
            let Some(t) = tabs.get(id) else { return };
            (t.url.clone(), t.private)
        };
        if url.is_empty() {
            return;
        }
        let view = self.front_view();
        let host = host_of(&url).unwrap_or_default();
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.set_margin_top(10);
        card.set_margin_bottom(10);
        card.set_margin_start(12);
        card.set_margin_end(12);
        card.set_size_request(280, -1);

        let title = gtk::Label::builder().label(if host.is_empty() { &url } else { &host }).xalign(0.0).build();
        title.add_css_class("card-title");
        card.append(&title);

        // The connection says it: private, or not.
        let (line, class) = if url.starts_with("rippl://") {
            ("rippl's own page".to_string(), "safe")
        } else if url.starts_with("https://") {
            let errors = view.as_ref().and_then(|v| v.tls_info()).map(|(_, e)| e);
            if errors.is_some_and(|e| !e.is_empty()) {
                ("Not private: the certificate has problems".to_string(), "unsafe")
            } else {
                ("Connection is private".to_string(), "safe")
            }
        } else if url.starts_with("http://") {
            ("Not private: anyone on the network can see and change this page".to_string(), "unsafe")
        } else {
            (String::new(), "safe")
        };
        if !line.is_empty() {
            let l = gtk::Label::builder().label(&line).xalign(0.0).wrap(true).build();
            l.add_css_class(class);
            card.append(&l);
        }

        // Copy, Print, zoom.
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let copy = gtk::Button::with_label("Copy address");
        let print = gtk::Button::with_label("Print…");
        let minus = gtk::Button::with_label("−");
        let zoom = gtk::Button::with_label(&format!(
            "{:.0}%",
            view.as_ref().map_or(100.0, |v| v.zoom_level() * 100.0)
        ));
        let plus = gtk::Button::with_label("+");
        for b in [&copy, &print, &minus, &zoom, &plus] {
            actions.append(b);
        }
        card.append(&actions);
        let weak = self.weak();
        copy.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.copy_address();
                b.card.popdown();
            }
        });
        let weak = self.weak();
        print.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.card.popdown();
                b.print();
            }
        });
        for (button, step) in [(&minus, -0.1), (&zoom, 0.0), (&plus, 0.1)] {
            let weak = self.weak();
            let label = zoom.clone();
            button.connect_clicked(move |_| {
                let Some(v) = weak.upgrade().and_then(|b| b.front_view()) else { return };
                let level = if step == 0.0 { 1.0 } else { (v.zoom_level() + step).clamp(0.3, 3.0) };
                v.set_zoom_level(level);
                label.set_label(&format!("{:.0}%", level * 100.0));
            });
        }

        if !host.is_empty() && !url.starts_with("rippl://") {
            card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            // Blocking ads and trackers here.
            let shield_on = !self.shield.is_allowed(&host);
            let weak = self.weak();
            let h = host.clone();
            card.append(&self.switch_line("Block ads and trackers", shield_on, move |_| {
                if let Some(b) = weak.upgrade() {
                    b.card.popdown();
                    b.toggle_shield_for(&h);
                }
            }));
            // Keep this site awake (never in a private tab: nothing kept).
            if !private {
                let awake = self.places.site_flag(&host, AWAKE).unwrap_or(false);
                let weak = self.weak();
                let h = host.clone();
                card.append(&self.switch_line("Keep this site awake", awake, move |on| {
                    if let Some(b) = weak.upgrade() {
                        let _ = b.places.set_site_flag(&h, AWAKE, on);
                        b.apply_keep_awake();
                    }
                }));
                // Sound by itself: only while videos may start at all.
                if self.config.borrow().autoplay {
                    let sound = self.places.site_flag(&host, crate::pagescripts::SOUND).unwrap_or(false);
                    let weak = self.weak();
                    let h = host.clone();
                    card.append(&self.switch_line("Play sound by itself", sound, move |on| {
                        if let Some(b) = weak.upgrade() {
                            let _ = b.places.set_site_flag(&h, crate::pagescripts::SOUND, on);
                        }
                    }));
                }
                let popups = self.places.site_flag(&host, POPUPS).unwrap_or(false);
                let weak = self.weak();
                let h = host.clone();
                card.append(&self.switch_line("Allow pop-up windows", popups, move |on| {
                    if let Some(b) = weak.upgrade() {
                        let _ = b.places.set_site_flag(&h, POPUPS, on);
                    }
                }));
            }
            // Darkening this site (only offered while it's on).
            if self.config.borrow().dusk {
                let darkened = !self.places.site_flag(&host, crate::pagelook::NO_DUSK).unwrap_or(false);
                let weak = self.weak();
                let h = host.clone();
                card.append(&self.switch_line("Darken this site", darkened, move |on| {
                    if let Some(b) = weak.upgrade() {
                        let _ = b.places.set_site_flag(&h, crate::pagelook::NO_DUSK, !on);
                        if let Some(v) = b.front_view() {
                            v.reload();
                        }
                    }
                }));
            }
            let style = gtk::Button::with_label(if self.places.site_css(&host).ok().flatten().is_some() {
                "Your style for this site (on)…"
            } else {
                "Your style for this site…"
            });
            style.add_css_class("flat");
            let weak = self.weak();
            let h = host.clone();
            style.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.edit_site_css(&h);
                }
            });
            card.append(&style);
            // What the site may use, and what is hidden on it.
            let permissions: Vec<(String, String, bool)> =
                self.places.permissions().unwrap_or_default().into_iter().filter(|(h, _, _)| *h == host).collect();
            let hidden = self.places.hidden().unwrap_or_default().into_iter().find(|(h, _)| *h == host).map(|(_, s)| s.len());
            if !permissions.is_empty() || hidden.is_some() {
                card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            }
            for (_, kind, allow) in &permissions {
                let l = gtk::Label::builder()
                    .label(format!("{kind}: {}", if *allow { "allowed" } else { "denied" }))
                    .xalign(0.0)
                    .build();
                l.add_css_class("dim");
                card.append(&l);
            }
            if !permissions.is_empty() {
                let forget = gtk::Button::with_label("Forget what this site was allowed");
                let weak = self.weak();
                let h = host.clone();
                forget.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        let _ = b.places.forget_permissions(Some(&h));
                        b.card.popdown();
                        b.notice("This site will ask again");
                    }
                });
                card.append(&forget);
            }
            if let Some(n) = hidden {
                let show = gtk::Button::with_label(&format!(
                    "{n} thing{} hidden here · Show again",
                    if n == 1 { "" } else { "s" }
                ));
                let weak = self.weak();
                show.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.card.popdown();
                        b.unhide_here();
                    }
                });
                card.append(&show);
            }
        }
        let more = gtk::Button::with_label("Settings…");
        more.add_css_class("flat");
        let weak = self.weak();
        more.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.card.popdown();
                b.open_internal("settings");
            }
        });
        card.append(&more);

        self.card.set_child(Some(&card));
        // Centred under the column, not under the field's middle: the card
        // is wider than the field and would hang off the window's edge.
        // The card is ~340 px wide with its frame: its middle at 190 px
        // keeps its left edge clear of the window's.
        let x = 190.min(self.field.width().max(1));
        let rect = webkit6::gdk::Rectangle::new(x, self.field.height(), 1, 1);
        self.card.set_pointing_to(Some(&rect));
        self.card.popup();
    }

    fn switch_line(&self, text: &str, on: bool, changed: impl Fn(bool) + 'static) -> gtk::Box {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::builder().label(text).xalign(0.0).hexpand(true).build();
        let switch = gtk::Switch::builder().active(on).valign(gtk::Align::Center).build();
        switch.connect_active_notify(move |s| changed(s.is_active()));
        line.append(&label);
        line.append(&switch);
        line
    }

    /// Shield off (or back on) for `host`, then reload the page.
    pub(crate) fn toggle_shield_for(&self, host: &str) {
        let weak = self.weak();
        self.notice("Updating the blocker for this site…");
        self.shield.toggle(host, move |_| {
            if let Some(b) = weak.upgrade() {
                b.show_front();
                if let Some(v) = b.front_view() {
                    v.reload();
                }
            }
        });
        self.install_page_scripts();
    }

    /// Every tab of a kept-awake site is marked so it never sleeps.
    pub(crate) fn apply_keep_awake(&self) {
        let hosts: Vec<String> = {
            let tabs = self.tabs.borrow();
            tabs.list.iter().map(|t| host_of(&t.url).unwrap_or_default()).collect()
        };
        let flags: Vec<bool> = hosts
            .iter()
            .map(|h| !h.is_empty() && self.places.site_flag(h, AWAKE).unwrap_or(false))
            .collect();
        let mut tabs = self.tabs.borrow_mut();
        for (tab, keep) in tabs.list.iter_mut().zip(flags) {
            tab.keep_awake = keep && !tab.private;
        }
    }

    /// Ctrl+P.
    pub(crate) fn print(&self) {
        if let Some(view) = self.front_view() {
            let op = PrintOperation::new(&view);
            op.run_dialog(Some(&self.window));
        }
    }

    /// Ctrl+Shift+D: the tab again, beside it, with its history and scroll.
    pub(crate) fn duplicate(&self, id: TabId) {
        let (url, title, private, memory) = {
            let tabs = self.tabs.borrow();
            let Some(t) = tabs.get(id) else { return };
            let memory = match self.views.borrow().get(&id) {
                Some(v) => v.session_state().and_then(|s| s.serialize()).map(|b| b.to_vec()),
                None => t.memory.clone(),
            };
            (t.url.clone(), t.title.clone(), t.private, memory)
        };
        if url.is_empty() {
            return;
        }
        self.tabs.borrow_mut().select(id);
        let new = {
            let mut tabs = self.tabs.borrow_mut();
            let new = tabs.open(&url);
            if let Some(t) = tabs.get_mut(new) {
                t.title = title;
                t.private = private;
                t.life = if memory.is_some() { Life::Asleep } else { Life::Lazy };
                t.memory = memory;
            }
            new
        };
        self.dirty.set(true);
        self.select(new);
    }

    /// Right-click on a tab: what can be done with it.
    pub(crate) fn tab_menu(&self, id: TabId, anchor: &impl IsA<gtk::Widget>) {
        let pinned = self.tabs.borrow().get(id).is_some_and(|t| t.pinned);
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (group, space, spaces, groups_here) = {
            let tabs = self.tabs.borrow();
            let tab = tabs.get(id);
            (
                tab.and_then(|t| t.group),
                tab.map_or(0, |t| t.space),
                tabs.spaces.iter().map(|s| (s.id, s.name.clone())).collect::<Vec<_>>(),
                tabs.groups.iter().filter(|g| Some(g.space) == tab.map(|t| t.space)).map(|g| (g.id, g.name.clone())).collect::<Vec<_>>(),
            )
        };
        let mut items: Vec<MenuAction> = vec![
            ("Duplicate".into(), std::rc::Rc::new(|b: &Browser, id| b.duplicate(id))),
            (
                if pinned { "Unpin" } else { "Pin" }.into(),
                std::rc::Rc::new(|b: &Browser, id| {
                    b.select(id);
                    b.toggle_pin();
                }),
            ),
            ("Put beside this tab".into(), std::rc::Rc::new(|b: &Browser, id| b.split_with(id))),
        ];
        if !pinned {
            items.push(("Add to a new group".into(), std::rc::Rc::new(|b: &Browser, id| {
                b.tabs.borrow_mut().new_group(id, "");
                b.dirty.set(true);
                b.render();
            })));
            for (g, name) in groups_here.into_iter().filter(|(g, _)| Some(*g) != group) {
                items.push((format!("Add to “{name}”"), std::rc::Rc::new(move |b: &Browser, id| {
                    b.tabs.borrow_mut().set_group(id, Some(g));
                    b.dirty.set(true);
                    b.render();
                })));
            }
            if group.is_some() {
                items.push(("Remove from group".into(), std::rc::Rc::new(|b: &Browser, id| {
                    b.tabs.borrow_mut().set_group(id, None);
                    b.dirty.set(true);
                    b.render();
                })));
            }
        }
        for (sid, name) in spaces.into_iter().filter(|(s, _)| *s != space) {
            items.push((format!("Move to {name}"), std::rc::Rc::new(move |b: &Browser, id| b.move_tab_to_space(id, sid))));
        }
        let rest: [MenuItem; 4] = [
            ("Copy address", |b, id| {
                b.select(id);
                b.copy_address();
            }),
            ("Reload", |b, id| {
                if let Some(v) = b.views.borrow().get(&id).cloned() {
                    v.reload();
                }
            }),
            ("Close", |b, id| b.close(id)),
            ("Close other tabs", |b, id| {
                // In this tab's space only; pins stay.
                let others: Vec<TabId> = {
                    let tabs = b.tabs.borrow();
                    let space = tabs.get(id).map_or(0, |t| t.space);
                    tabs.list.iter().filter(|t| t.id != id && !t.pinned && t.space == space).map(|t| t.id).collect()
                };
                for o in others {
                    b.close(o);
                }
            }),
        ];
        for (label, run) in rest {
            items.push((label.to_string(), std::rc::Rc::new(run)));
        }
        for (label, run) in items {
            let button = gtk::Button::builder().label(label.as_str()).build();
            button.add_css_class("flat");
            button.add_css_class("menu-item");
            if let Some(l) = button.child().and_downcast::<gtk::Label>() {
                l.set_xalign(0.0);
            }
            let weak = self.weak();
            button.connect_clicked(move |_| {
                let Some(b) = weak.upgrade() else { return };
                b.tab_menu.popdown();
                // After the popover has gone: these redraw the column.
                let run = run.clone();
                webkit6::glib::idle_add_local_once(move || run(&b, id));
            });
            menu.append(&button);
        }
        self.tab_menu.set_child(Some(&menu));
        let Some(parent) = self.tab_menu.parent() else { return };
        let Some(bounds) = anchor.compute_bounds(&parent) else { return };
        let rect = webkit6::gdk::Rectangle::new(
            (bounds.x() + 24.0) as i32,
            (bounds.y() + bounds.height()) as i32,
            1,
            1,
        );
        self.tab_menu.set_pointing_to(Some(&rect));
        self.tab_menu.popup();
    }

    /// A page asked for a window without a click just before it.
    pub(crate) fn popup_blocked(&self, id: TabId, url: String) {
        let mut blocked = self.blocked_popups.borrow_mut();
        let list = blocked.entry(id).or_default();
        if !list.contains(&url) {
            list.push(url);
        }
        if list.len() > 5 {
            list.remove(0);
        }
        drop(blocked);
        self.show_popup_bar();
    }

    /// The bar for the front tab's last blocked pop-up, or none.
    pub(crate) fn show_popup_bar(&self) {
        let front = self.front();
        let last = front.and_then(|f| self.blocked_popups.borrow().get(&f).and_then(|l| l.last().cloned()));
        match last {
            Some(url) => {
                let host = front
                    .and_then(|f| self.tabs.borrow().get(f).map(|t| t.url.clone()))
                    .and_then(|u| host_of(&u))
                    .unwrap_or_else(|| "This page".into());
                let target = if url.is_empty() { String::new() } else { format!(" ({})", rippl_core::suggest::bare(&url)) };
                self.popup_label.set_label(&format!("{host} tried to open a window{target}"));
                self.popup_bar.set_visible(true);
            }
            None => self.popup_bar.set_visible(false),
        }
    }

    pub(crate) fn wire_popup_bar(&self) {
        for (button, always) in [(&self.popup_open, false), (&self.popup_always, true)] {
            let weak = self.weak();
            button.connect_clicked(move |_| {
                let Some(b) = weak.upgrade() else { return };
                let Some(front) = b.front() else { return };
                let url = b.blocked_popups.borrow_mut().get_mut(&front).and_then(|l| l.pop());
                if always {
                    let site = b.tabs.borrow().get(front).and_then(|t| host_of(&t.url));
                    if let Some(h) = site {
                        let _ = b.places.set_site_flag(&h, POPUPS, true);
                    }
                }
                b.show_popup_bar();
                if let Some(url) = url.filter(|u| !u.is_empty()) {
                    let private = b.tabs.borrow().get(front).is_some_and(|t| t.private);
                    b.open(&url, private, true);
                }
            });
        }
        let weak = self.weak();
        self.popup_dismiss.connect_clicked(move |_| {
            let Some(b) = weak.upgrade() else { return };
            if let Some(front) = b.front() {
                b.blocked_popups.borrow_mut().remove(&front);
            }
            b.show_popup_bar();
        });
    }

    pub(crate) fn popup_bar() -> (gtk::Box, gtk::Label, gtk::Button, gtk::Button, gtk::Button) {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("ask");
        bar.set_halign(gtk::Align::Center);
        bar.set_valign(gtk::Align::Start);
        bar.set_margin_top(10);
        let label = gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::Middle).max_width_chars(60).build();
        let open = gtk::Button::with_label("Open");
        open.add_css_class("suggested-action");
        let always = gtk::Button::with_label("Always allow");
        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.add_css_class("flat");
        bar.append(&label);
        bar.append(&open);
        bar.append(&always);
        bar.append(&dismiss);
        bar.set_visible(false);
        (bar, label, open, always, dismiss)
    }
}
