//! For tests and measuring (`RIPPL_CONTROL=<port>`): one line in, one line
//! out, on 127.0.0.1 only, as the GTK build's control socket. Off unless
//! asked for.
//!
//! - `tabs`: the space's tabs as JSON
//! - `open <url>` / `go <text>` / `cmd <command>` / `select <n>`
//! - `set <key>=<value>`: a setting, as rippl://settings changes it
//! - `eval <js>`: run in the front page (answers "ok")
//! - `quit`

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use crate::browser::{Browser, Ev, Proxy, post};

pub fn start(proxy: Proxy) {
    let Some(port) = std::env::var("RIPPL_CONTROL").ok().and_then(|p| p.parse::<u16>().ok()) else { return };
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
            eprintln!("rippl: control: port {port} is taken");
            return;
        };
        for stream in listener.incoming().flatten() {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let mut out = match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let (tx, rx) = mpsc::channel();
                    post(&proxy, Ev::Control(line, tx));
                    let answer = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| "timeout".into());
                    if writeln!(out, "{answer}").is_err() {
                        return;
                    }
                }
            });
        }
    });
}

impl Browser {
    pub fn control(&self, line: &str) -> String {
        let (verb, rest) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
        match verb {
            "tabs" => {
                let tabs = self.tabs.borrow();
                let list: Vec<serde_json::Value> = tabs
                    .here()
                    .map(|t| {
                        serde_json::json!({"id": t.id, "url": t.url, "title": t.title, "front": Some(t.id) == tabs.front,
                            "awake": self.views.borrow().contains_key(&t.id), "pinned": t.pinned, "loading": t.loading})
                    })
                    .collect();
                serde_json::Value::Array(list).to_string()
            }
            "open" => {
                let id = self.open(rest, false, false);
                id.to_string()
            }
            "go" => {
                self.go(rest, false, false);
                "ok".into()
            }
            "cmd" => {
                self.command(rest, false);
                "ok".into()
            }
            "select" => {
                let n: usize = rest.parse().unwrap_or(1);
                let id = self.tabs.borrow().here().nth(n.saturating_sub(1)).map(|t| t.id);
                match id {
                    Some(id) => {
                        self.select(id);
                        "ok".into()
                    }
                    None => "no such tab".into(),
                }
            }
            "set" => {
                let (k, v) = rest.split_once('=').unwrap_or((rest, ""));
                match self.change_setting(k.trim(), v.trim()) {
                    Ok(note) if note.is_empty() => "ok".into(),
                    Ok(note) => note,
                    Err(e) => format!("error: {e}"),
                }
            }
            "grab" => {
                self.grab(rest, false);
                "ok".into()
            }
            "eval" => {
                self.front_view_eval(rest);
                "ok".into()
            }
            "notice" => self.notice.borrow().as_ref().map(|(n, _)| n.clone()).unwrap_or_default(),
            "downloads" => self.downloads.column().to_string(),
            "quit" => {
                self.after(|b| b.quit());
                "ok".into()
            }
            _ => format!("unknown: {verb}"),
        }
    }
}
