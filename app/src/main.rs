//! rippl: a small, fast, quiet web browser for Linux, on WebKit.

mod browser;
mod closed;
mod config;
mod control;
mod curtain;
mod downloads;
mod engine;
mod find;
mod internal_pages;
mod keyring;
mod look;
mod omnibox;
mod permissions;
mod pressure;
mod reader;
mod shield;
mod signin;
mod sitecard;
mod split;

use std::cell::RefCell;
use std::rc::Rc;

use webkit6::prelude::*;
use webkit6::{gio, glib, gtk};

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("dev.rippl.Rippl")
        // Arguments are URLs, read by the browser itself.
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    // One window, made once; a second launch just brings it to front.
    let window: Rc<RefCell<Option<Rc<browser::Browser>>>> = Rc::default();
    app.connect_command_line(move |app, _| {
        let mut slot = window.borrow_mut();
        match slot.as_ref() {
            Some(_) => {
                if let Some(w) = app.active_window() {
                    w.present();
                }
            }
            None => {
                let browser = browser::Browser::new(app);
                control::start(&browser);
                *slot = Some(browser);
            }
        }
        glib::ExitCode::SUCCESS
    });
    app.run()
}
