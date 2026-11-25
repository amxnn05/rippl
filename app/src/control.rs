//! Driving rippl from a script  does: for tests,
//! measurements and, later, agents. Off unless RIPPL_CONTROL=1.
//!
//! A Unix socket at `$XDG_RUNTIME_DIR/rippl.sock`, mode 0600 inside a
//! directory only this user can enter. One command per line in, the reply
//! out, then the connection closes:
//!
//! ```text
//! list                one line per tab: index, id, life, front, url, title
//! open <url|words>    opens a tab in front; replies with its index
//! select <index>      1-based, as Ctrl+1..9
//! close <index>
//! sleep               puts every tab that may sleep to sleep now
//! js <code>           runs <code> in the front page, answers with JSON
//! ```
//!
//! (and the rest in docs/testing.md). `js` can read any open page, which
//! is why the socket is off by default and only this user can reach it.
//!
//! `scripts/ctl.sh` speaks it from the shell.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::mpsc;

use webkit6::glib;

use crate::browser::Browser;

thread_local! {
    static BROWSER: RefCell<Weak<Browser>> = const { RefCell::new(Weak::new()) };
}

/// `rippl.sock`, or `rippl-<profile>.sock` for a profile's.
pub fn socket_path() -> PathBuf {
    let name = match crate::engine::profile() {
        Some(p) => format!("rippl-{p}.sock"),
        None => "rippl.sock".to_string(),
    };
    webkit6::glib::user_runtime_dir().join(name)
}

/// Starts listening, if RIPPL_CONTROL is set.
pub fn start(browser: &Rc<Browser>) {
    if std::env::var_os("RIPPL_CONTROL").is_none() {
        return;
    }
    BROWSER.with(|b| *b.borrow_mut() = Rc::downgrade(browser));
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => return eprintln!("rippl: control socket: {e}"),
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    eprintln!("rippl: control socket at {}", path.display());

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            // The browser lives on the main thread: hand the command over
            // and wait for its answer.
            let (tx, rx) = mpsc::channel();
            let command = line.trim().to_string();
            glib::MainContext::default().invoke(move || {
                BROWSER.with(|b| match b.borrow().upgrade() {
                    Some(browser) => browser.command_with(&command, Box::new(move |reply| {
                        let _ = tx.send(reply);
                    })),
                    None => {
                        let _ = tx.send("error: no browser".into());
                    }
                });
            });
            let reply = rx.recv().unwrap_or_else(|_| "error: no reply".into());
            let mut stream = stream;
            let _ = writeln!(stream, "{}", reply.trim_end());
        }
    });
}
