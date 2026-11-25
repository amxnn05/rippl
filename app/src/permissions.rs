//! What a page may use: camera, microphone, screen, location,
//! notifications, clipboard, DRM, its cookies inside other sites.
//!
//! A page asking gets a bar over the top of the page, "site wants to use
//! your camera", with Allow and Deny. The answer is remembered for that
//! site in places.sqlite, except in a private tab, where it lasts only for
//! the page. Nothing is allowed without asking, with two exceptions that
//! give nothing away:
//! - pointer lock (games hide the cursor; Esc always gives it back);
//! - the names of your cameras and microphones, once the site already has
//!   camera or microphone access.
//!
//! `scripts/ctl.sh permissions` lists the answers; `permissions forget
//! [host]` forgets them.

use rippl_core::places::Permission;
use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{
    gtk, ClipboardPermissionRequest, DeviceInfoPermissionRequest, GeolocationPermissionRequest,
    MediaKeySystemPermissionRequest, NotificationPermissionRequest, PermissionRequest, PointerLockPermissionRequest,
    UserMediaPermissionRequest, WebView, WebsiteDataAccessPermissionRequest,
};

use crate::browser::Browser;
use crate::shield::host_of;

/// A question waiting for an answer.
pub struct Pending {
    pub tab: TabId,
    pub host: String,
    pub kind: Permission,
    pub request: PermissionRequest,
}

/// What kind of question `request` is; `None` for ones always refused
/// (VR headsets and anything newer than this code).
fn kind_of(request: &PermissionRequest) -> Option<Permission> {
    if let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() {
        let screen = webkit6::functions::user_media_permission_is_for_display_device(media);
        let video = media.is_for_video_device();
        let audio = media.is_for_audio_device();
        return Some(match (screen, video, audio) {
            (true, _, _) => Permission::Screen,
            (_, true, true) => Permission::CameraAndMicrophone,
            (_, true, false) => Permission::Camera,
            _ => Permission::Microphone,
        });
    }
    if request.is::<GeolocationPermissionRequest>() {
        Some(Permission::Location)
    } else if request.is::<NotificationPermissionRequest>() {
        Some(Permission::Notifications)
    } else if request.is::<ClipboardPermissionRequest>() {
        Some(Permission::Clipboard)
    } else if request.is::<MediaKeySystemPermissionRequest>() {
        Some(Permission::Drm)
    } else if request.is::<WebsiteDataAccessPermissionRequest>() {
        Some(Permission::Storage)
    } else {
        None
    }
}

impl Browser {
    /// The bar the questions are asked in, over the top of the page.
    pub(crate) fn permission_bar() -> (gtk::Box, gtk::Label, gtk::Button, gtk::Button) {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("ask");
        bar.set_halign(gtk::Align::Center);
        bar.set_valign(gtk::Align::Start);
        bar.set_margin_top(10);
        let label = gtk::Label::new(None);
        label.set_wrap(true);
        let deny = gtk::Button::with_label("Deny");
        let allow = gtk::Button::with_label("Allow");
        allow.add_css_class("suggested-action");
        bar.append(&label);
        bar.append(&deny);
        bar.append(&allow);
        bar.set_visible(false);
        (bar, label, deny, allow)
    }

    pub(crate) fn wire_permission_bar(&self) {
        let weak = self.weak();
        self.ask_allow.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.answer(true);
            }
        });
        let weak = self.weak();
        self.ask_deny.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.answer(false);
            }
        });
    }

    /// WebKit's permission-request signal for tab `id`'s view.
    pub(crate) fn permission_asked(&self, id: TabId, view: &WebView, request: &PermissionRequest) -> bool {
        let host = view.uri().and_then(|u| host_of(&u)).unwrap_or_default();
        let private = self.tabs.borrow().get(id).is_some_and(|t| t.private);
        if request.is::<PointerLockPermissionRequest>() {
            request.allow();
            return true;
        }
        if request.is::<DeviceInfoPermissionRequest>() {
            let known = [Permission::Camera, Permission::Microphone, Permission::CameraAndMicrophone]
                .iter()
                .any(|k| self.places.permission(&host, *k).ok().flatten() == Some(true));
            if known { request.allow() } else { request.deny() }
            return true;
        }
        let Some(kind) = kind_of(request) else {
            request.deny();
            return true;
        };
        if !private {
            match self.places.permission(&host, kind).ok().flatten() {
                Some(true) => {
                    request.allow();
                    return true;
                }
                Some(false) => {
                    request.deny();
                    return true;
                }
                None => {}
            }
        }
        self.asking.borrow_mut().push(Pending { tab: id, host, kind, request: request.clone() });
        self.show_question();
        true
    }

    /// Shows the oldest question from the tab in front, or hides the bar.
    pub(crate) fn show_question(&self) {
        let front = self.front();
        let asking = self.asking.borrow();
        match asking.iter().find(|p| Some(p.tab) == front) {
            Some(p) => {
                let site = if p.host.is_empty() { "This page" } else { &p.host };
                self.ask_label.set_label(&format!("{site} wants to {}", p.kind.ask()));
                self.ask_bar.set_visible(true);
            }
            None => self.ask_bar.set_visible(false),
        }
    }

    /// Answers the question shown (the control socket's way of clicking).
    pub(crate) fn answer_front(&self, allow: bool) {
        self.answer(allow);
    }

    fn answer(&self, allow: bool) {
        let front = self.front();
        let pending = {
            let mut asking = self.asking.borrow_mut();
            let Some(i) = asking.iter().position(|p| Some(p.tab) == front) else { return };
            asking.remove(i)
        };
        if allow { pending.request.allow() } else { pending.request.deny() }
        let private = self.tabs.borrow().get(pending.tab).is_some_and(|t| t.private);
        if !private && !pending.host.is_empty()
            && let Err(e) = self.places.set_permission(&pending.host, pending.kind, allow) {
                eprintln!("rippl: could not remember a permission: {e}");
            }
        // The same site asking the same thing from another tab gets the
        // same answer.
        let same: Vec<Pending> = {
            let mut asking = self.asking.borrow_mut();
            let (same, rest) = asking.drain(..).partition(|p| p.host == pending.host && p.kind == pending.kind && !private);
            *asking = rest;
            same
        };
        for p in same {
            if allow { p.request.allow() } else { p.request.deny() }
        }
        self.show_question();
    }

    /// A tab went away: its questions go unanswered (WebKit denies them).
    pub(crate) fn drop_questions(&self, id: TabId) {
        self.asking.borrow_mut().retain(|p| p.tab != id);
        self.show_question();
    }
}

/// Kept for the browser struct's field type.
pub type Asking = std::cell::RefCell<Vec<Pending>>;
