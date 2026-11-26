//! rippl: a small, fast, quiet web browser for Linux, on WebKit.

mod bookmarks_bar;
mod browser;
mod certificates;
mod chrome;
mod closed;
mod closing;
mod config;
mod control;
mod curtain;
mod downloads;
mod engine;
mod find;
mod internal_pages;
mod keyring;
mod look;
mod home;
mod home_theme;
mod omnibox;
mod pagescripts;
mod peek;
mod pin_folders;
mod player;
mod pagelook;
mod permissions;
mod pressure;
mod reader;
mod shield;
mod shortcuts;
mod signin;
mod sitecard;
mod space_look;
mod spaces;
mod split;
mod switcher;
mod sync;
mod windows;

use std::cell::RefCell;
use std::rc::Rc;

use webkit6::prelude::*;
use webkit6::{gio, glib, gtk};

/// The command line without `--profile NAME` (or `--profile=NAME`), and
/// the profile named, if any.
fn split_profile(args: Vec<String>) -> (Vec<String>, Option<String>) {
    let mut rest = Vec::new();
    let mut profile = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--profile" {
            profile = it.next();
        } else if let Some(p) = a.strip_prefix("--profile=") {
            profile = Some(p.to_string());
        } else {
            rest.push(a);
        }
    }
    (rest, profile)
}

fn main() -> glib::ExitCode {
    let (args, profile) = split_profile(std::env::args().collect());
    if let Err(e) = engine::set_profile(profile.as_deref()) {
        eprintln!("rippl: {e}");
        return glib::ExitCode::FAILURE;
    }
    // A profile is a separate browser: its own application id, so it has a
    // window of its own rather than joining the default profile's.
    let id = match engine::profile() {
        Some(p) => format!("dev.rippl.Rippl.profile_{}", p.replace('-', "_")),
        None => "dev.rippl.Rippl".to_string(),
    };
    let app = gtk::Application::builder()
        .application_id(id)
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
                let browser = windows::start(app);
                control::start(&browser);
                *slot = Some(browser);
            }
        }
        glib::ExitCode::SUCCESS
    });
    app.run_with_args(&args)
}
