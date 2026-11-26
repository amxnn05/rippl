//! Several windows: Ctrl+N opens another, with
//! tabs of its own. They share one engine (sign-ins, cache), Shield,
//! history and bookmarks, passwords and settings, so a second window costs
//! a window, not a second browser.
//!
//! Each window keeps its own session file (`session` for the first,
//! `session-2`, `session-3`, … for the others) and comes back after a
//! restart as it was. Closing a window while others stay closes its tabs,
//! as Ctrl+W would close each (Ctrl+Shift+T brings them back); the last
//! window closed, or rippl stopped, keeps everything for next time.
//!
//! Downloads are listed in the first window; the control socket speaks to
//! the window in front.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use webkit6::prelude::*;
use webkit6::{glib, gtk};



use crate::browser::Browser;

thread_local! {
    static WINDOWS: RefCell<Vec<Rc<Browser>>> = const { RefCell::new(Vec::new()) };
    static SIGTERM: Cell<bool> = const { Cell::new(false) };
}

/// Window `no`'s session file.
pub(crate) fn session_file(no: usize) -> PathBuf {
    let dir = crate::engine::data_dir();
    if no == 0 { dir.join("session") } else { dir.join(format!("session-{}", no + 1)) }
}

/// Every open window, the first first.
pub(crate) fn all() -> Vec<Rc<Browser>> {
    WINDOWS.with(|w| w.borrow().clone())
}

/// The window in front, or the first.
pub(crate) fn front() -> Option<Rc<Browser>> {
    let all = all();
    all.iter().find(|b| b.window.is_active()).cloned().or_else(|| all.first().cloned())
}

/// The first window of all, at launch, then the others that were open last
/// time.
pub(crate) fn start(app: &gtk::Application) -> Rc<Browser> {
    let first = Browser::new(app, None, 0);
    WINDOWS.with(|w| w.borrow_mut().push(first.clone()));
    let mut kept: Vec<usize> = std::fs::read_dir(crate::engine::data_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_prefix("session-")?.parse::<usize>().ok())
        .filter(|n| *n >= 2)
        .map(|n| n - 1)
        .collect();
    kept.sort();
    for no in kept {
        open_numbered(app, no);
    }
    first.window.present();
    first
}

/// rippl started again while it runs: a link from another app (opened in
/// a new tab of the window in front), `--new-window`, `--private-window`;
/// with nothing, the window in front comes forward.
pub(crate) fn launched_again(app: &gtk::Application, args: &[String]) {
    let mut target = front();
    if args.iter().any(|a| a == "--new-window") {
        target = open(app).or(target);
    }
    let Some(b) = target else { return };
    if args.iter().any(|a| a == "--private-window") {
        b.open("", true, true);
    }
    let search = b.config.borrow().search.clone();
    let mut skip = false;
    let mut first = true;
    for a in args {
        if std::mem::take(&mut skip) {
            continue;
        }
        if a == "--profile" {
            skip = true;
            continue;
        }
        if a.starts_with('-') {
            continue;
        }
        if let rippl_core::address::Destination::Page(url) = rippl_core::address::resolve(a, &search) {
            b.open(&url, false, first);
            first = false;
        }
    }
    b.window.present();
}

/// Ctrl+N: a new window, with a new tab.
pub(crate) fn open(app: &gtk::Application) -> Option<Rc<Browser>> {
    let taken: Vec<usize> = all().iter().map(|b| b.window_no).collect();
    let no = (1..).find(|n| !taken.contains(n))?;
    let _ = std::fs::remove_file(session_file(no));
    open_numbered(app, no)
}

fn open_numbered(app: &gtk::Application, no: usize) -> Option<Rc<Browser>> {
    let from = all().into_iter().find(|b| b.primary)?;
    let window = Browser::new(app, Some(from), no);
    WINDOWS.with(|w| w.borrow_mut().push(window.clone()));
    Some(window)
}

/// A window is closing. The last one keeps its tabs for next time; one of
/// several closes them, its session file with them.
pub(crate) fn closing(b: &Rc<Browser>) {
    let others = all().iter().filter(|o| !Rc::ptr_eq(o, b)).count();
    if others == 0 {
        b.save();
    } else {
        // Its tabs go as Ctrl+W would close each: Ctrl+Shift+T in the
        // window left in front brings them back.
        let ids: Vec<_> = b.tabs.borrow().list.iter().map(|t| t.id).collect();
        for id in &ids {
            b.remember_closed(*id);
        }
        if let Some(to) = all().into_iter().find(|o| !Rc::ptr_eq(o, b)) {
            let moved: Vec<_> = b.closed.borrow_mut().drain(..).collect();
            to.closed.borrow_mut().extend(moved);
        }
        for v in b.views.borrow().values() {
            v.terminate_web_process();
        }
        let _ = std::fs::remove_file(b.session_path());
    }
    // The first window holds what the others share: it stays alive (its
    // window gone) while they do.
    if !b.primary || others == 0 {
        WINDOWS.with(|w| w.borrow_mut().retain(|o| !Rc::ptr_eq(o, b)));
    } else {
        WINDOWS.with(|w| {
            let mut w = w.borrow_mut();
            if let Some(i) = w.iter().position(|o| Rc::ptr_eq(o, b)) {
                let first = w.remove(i);
                // Kept, out of the list, so the shared handlers live on.
                KEPT.with(|k| k.borrow_mut().push(first));
            }
        });
    }
}

thread_local! {
    static KEPT: RefCell<Vec<Rc<Browser>>> = const { RefCell::new(Vec::new()) };
}

/// Stopped politely (logout, `kill`): every window saved first. Once.
pub(crate) fn save_on_sigterm() {
    if SIGTERM.with(|s| s.replace(true)) {
        return;
    }
    glib_unix::unix_signal_add_local(libc::SIGTERM, || {
        let windows = all();
        for b in &windows {
            b.save();
        }
        if let Some(app) = windows.first().and_then(|b| b.window.application()) {
            app.quit();
        }
        glib::ControlFlow::Break
    });
}
