//! rippl's web process extension: WebKit loads this library into every page
//! process rippl starts, and it sees every request before it is sent.
//!
//! - **Global Privacy Control**: adds `Sec-GPC: 1` to every request, the
//!   header that tells sites (and, under laws like the CCPA and GDPR, obliges
//!   many of them) not to sell or share your data.
//! - **HTTPS-only**: rewrites a request for `http://` to a public host into
//!   `https://` before it leaves (`rippl_core::address::https_upgrade`).
//!   Pages, frames, images and scripts alike: nothing goes out in plain text
//!   to a site that has https. Hosts the user chose to visit over http
//!   anyway (the browser sends `rippl-allow-http`) are left alone for this
//!   session.
//!
//! The browser passes `(gpc, https_only, hosts allowed on http)` when the
//! process starts, `rippl-settings` with the two switches when they
//! change, and `rippl-allow-http` for a host. The allowed hosts travel in
//! the start-up data too: WebKit often starts a new page process for a
//! navigation to another site, and that process missed earlier messages.
//!
//! Nothing here sends anything anywhere; it only edits requests the page
//! was already making.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use webkit_web_process_extension6 as wpe;
use wpe::glib::translate::{from_glib_borrow, Borrowed, FromGlibPtrBorrow, FromGlibPtrNone};
use wpe::{glib, URIRequest, UserMessage, WebPage, WebProcessExtension};

#[derive(Default)]
struct State {
    gpc: Cell<bool>,
    https_only: Cell<bool>,
    /// Hosts to leave on http this session (the user said so).
    allowed_http: RefCell<HashSet<String>>,
    /// RIPPL_DEBUG, read once.
    debug: Cell<bool>,
}

impl State {
    fn on_request(&self, request: &URIRequest) {
        if self.https_only.get()
            && let Some(uri) = request.uri()
            && let Some(secure) = rippl_core::address::https_upgrade(&uri)
        {
            let host = rippl_core::passwords::origin_of(&uri)
                .map(|o| rippl_core::passwords::host_of_origin(&o).to_string())
                .unwrap_or_default();
            let allowed = self.allowed_http.borrow().contains(&host);
            if self.debug.get() {
                eprintln!("rippl extension [{}]: {uri} -> {}", std::process::id(), if allowed { "allowed on http" } else { &secure });
            }
            if !allowed {
                request.set_uri(&secure);
            }
        }
        if self.gpc.get()
            && let Some(headers) = request.http_headers()
        {
            headers.replace("Sec-GPC", "1");
        }
    }

    fn on_message(&self, message: &UserMessage) {
        let name = message.name().map(|n| n.to_string()).unwrap_or_default();
        let parameters = message.parameters();
        match name.as_str() {
            "rippl-allow-http" => {
                if self.debug.get() {
                    eprintln!("rippl extension [{}]: allow-http message", std::process::id());
                }
                if let Some(host) = parameters.and_then(|p| p.get::<String>()) {
                    self.allowed_http.borrow_mut().insert(host);
                }
            }
            "rippl-settings" => {
                if let Some((gpc, https)) = parameters.and_then(|p| p.get::<(bool, bool)>()) {
                    self.gpc.set(gpc);
                    self.https_only.set(https);
                }
            }
            _ => {}
        }
    }
}

thread_local! {
    static STATE: Rc<State> = Rc::new(State::default());
}

/// `send-request`, connected by hand: the generated binding wraps the
/// redirect response in a non-null type, but WebKit passes NULL when there
/// was no redirect. Here it stays a raw pointer and is never touched.
unsafe extern "C" fn send_request(
    _page: *mut wpe::ffi::WebKitWebPage,
    request: *mut wpe::ffi::WebKitURIRequest,
    _redirected_response: *mut wpe::ffi::WebKitURIResponse,
    _data: glib::ffi::gpointer,
) -> glib::ffi::gboolean {
    if request.is_null() {
        return glib::ffi::GFALSE;
    }
    // SAFETY: WebKit hands a valid request for the length of the call.
    let request: Borrowed<URIRequest> = unsafe { URIRequest::from_glib_borrow(request) };
    STATE.with(|s| s.on_request(&request));
    glib::ffi::GFALSE // FALSE: let the request go (with our changes)
}

fn page_created(page: &WebPage) {
    // SAFETY: connecting a C callback whose signature matches WebKit's
    // send-request signal: (page, request, redirected_response, data) -> gboolean.
    unsafe {
        glib::signal::connect_raw(
            glib::prelude::ObjectType::as_ptr(page) as *mut _,
            c"send-request".as_ptr(),
            Some(std::mem::transmute::<*const (), unsafe extern "C" fn()>(send_request as *const ())),
            // connect_raw frees this as a Box when the page goes away, so
            // it must be one, even if empty.
            Box::into_raw(Box::new(())),
        );
    }
}

/// Called by WebKit once per page process.
///
/// # Safety
/// WebKit passes a valid extension and an optional `(bbas)` variant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn webkit_web_process_extension_initialize_with_user_data(
    extension: *mut wpe::ffi::WebKitWebProcessExtension,
    user_data: *const glib::ffi::GVariant,
) {
    let extension: WebProcessExtension = unsafe { WebProcessExtension::from_glib_none(extension) };
    STATE.with(|s| s.debug.set(std::env::var_os("RIPPL_DEBUG").is_some()));
    if !user_data.is_null() {
        let data: Borrowed<glib::Variant> = unsafe { from_glib_borrow(user_data as *mut glib::ffi::GVariant) };
        if let Some((gpc, https, allowed)) = data.get::<(bool, bool, Vec<String>)>() {
            STATE.with(|s| {
                s.gpc.set(gpc);
                s.https_only.set(https);
                s.allowed_http.borrow_mut().extend(allowed);
            });
        }
    }
    extension.connect_page_created(|_, page| page_created(page));
    extension.connect_user_message_received(|_, message| STATE.with(|s| s.on_message(message)));
    // The extension lives as long as the process.
    std::mem::forget(extension);
}
