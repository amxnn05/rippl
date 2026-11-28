//! Downloads: what is coming in, at the foot of the column, and the list
//! kept for rippl://downloads (`downloads.log`, as the GTK build keeps it:
//! never from a private tab, and not while `history = off`).
//!
//! The engines say when a download starts and ends; how far along it is
//! comes from the file's size on disk, read once a second.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Instant;

use rippl_core::downloads::{self as dl, Entry};
use rippl_core::pages::LiveDownload;
use serde_json::{Value, json};

pub struct Live {
    pub id: u64,
    pub url: String,
    pub path: PathBuf,
    pub started: Instant,
    pub bytes: u64,
    pub done: Option<bool>,
    pub why: String,
}

pub struct Downloads {
    pub live: RefCell<Vec<Live>>,
    kept: RefCell<Vec<Entry>>,
    pub keep_list: Cell<bool>,
    next: Cell<u64>,
}

fn log_path() -> PathBuf {
    rippl_core::dirs::data().join("downloads.log")
}

fn size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} kB", b / 1e3)
    } else {
        format!("{bytes} B")
    }
}

impl Downloads {
    pub fn new(keep_list: bool) -> Downloads {
        let kept = dl::read_log(&std::fs::read_to_string(log_path()).unwrap_or_default());
        Downloads { live: RefCell::new(Vec::new()), kept: RefCell::new(kept), keep_list: Cell::new(keep_list), next: Cell::new(1) }
    }

    pub fn started(&self, url: String, path: PathBuf) {
        let id = self.next.get();
        self.next.set(id + 1);
        self.live.borrow_mut().push(Live { id, url, path, started: Instant::now(), bytes: 0, done: None, why: String::new() });
    }

    /// The engine's word that one ended. `private`: from a private tab.
    pub fn finished(&self, url: &str, path: Option<PathBuf>, ok: bool, private: bool) -> Option<String> {
        let mut live = self.live.borrow_mut();
        let item = live.iter_mut().rev().find(|d| d.url == url && d.done.is_none())?;
        if let Some(p) = path {
            item.path = p;
        }
        item.done = Some(ok);
        item.bytes = std::fs::metadata(&item.path).map(|m| m.len()).unwrap_or(item.bytes);
        if !ok {
            item.why = "It didn't finish".into();
        }
        let name = item.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let entry = Entry {
            when: rippl_core::places::now(),
            done: ok,
            name: name.clone(),
            path: if ok { item.path.to_string_lossy().to_string() } else { String::new() },
            url: url.to_string(),
            note: item.why.clone(),
        };
        drop(live);
        if !private && self.keep_list.get() {
            self.kept.borrow_mut().push(entry);
            self.save();
        }
        Some(name)
    }

    /// How far each is, from its file.
    pub fn poll(&self) -> bool {
        let mut changed = false;
        for d in self.live.borrow_mut().iter_mut().filter(|d| d.done.is_none()) {
            let mut b = std::fs::metadata(&d.path).map(|m| m.len()).unwrap_or(0);
            for ext in ["crdownload", "download"] {
                let mut part = d.path.clone().into_os_string();
                part.push(format!(".{ext}"));
                b = b.max(std::fs::metadata(PathBuf::from(part)).map(|m| m.len()).unwrap_or(0));
            }
            if b != d.bytes {
                d.bytes = b;
                changed = true;
            }
        }
        changed
    }

    fn save(&self) {
        let _ = std::fs::write(log_path(), dl::write_log(&self.kept.borrow()));
    }

    pub fn forget_live(&self, id: u64) {
        self.live.borrow_mut().retain(|d| d.id != id || d.done.is_none());
    }

    pub fn path_of(&self, id: u64) -> Option<PathBuf> {
        self.live.borrow().iter().find(|d| d.id == id && d.done == Some(true)).map(|d| d.path.clone())
    }

    fn status(d: &Live) -> String {
        match d.done {
            Some(true) => format!("{} · done", size(d.bytes)),
            Some(false) => d.why.clone(),
            None => {
                let secs = d.started.elapsed().as_secs_f64().max(0.5);
                if d.bytes == 0 {
                    "Starting…".into()
                } else {
                    format!("{} · {}/s", size(d.bytes), size((d.bytes as f64 / secs) as u64))
                }
            }
        }
    }

    pub fn column(&self) -> Value {
        Value::Array(
            self.live
                .borrow()
                .iter()
                .map(|d| {
                    json!({
                        "id": d.id,
                        "name": d.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        "status": Self::status(d), "fraction": -1.0,
                        "done": d.done == Some(true), "failed": d.done == Some(false),
                    })
                })
                .collect(),
        )
    }

    pub fn running(&self) -> Vec<LiveDownload> {
        self.live
            .borrow()
            .iter()
            .filter(|d| d.done.is_none())
            .map(|d| LiveDownload {
                name: d.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                status: Self::status(d),
                fraction: -1.0,
                paused: false,
            })
            .collect()
    }

    pub fn kept(&self) -> Vec<Entry> {
        let mut k = self.kept.borrow().clone();
        k.reverse();
        k
    }

    pub fn find_kept(&self, key: &str) -> Option<Entry> {
        let (when, url) = key.split_once('\n')?;
        self.kept.borrow().iter().find(|e| e.when.to_string() == when && e.url == url).cloned()
    }

    pub fn forget_kept(&self, key: &str) {
        if let Some((when, url)) = key.split_once('\n') {
            self.kept.borrow_mut().retain(|e| !(e.when.to_string() == when && e.url == url));
            self.save();
        }
    }

    pub fn clear_kept(&self) {
        self.kept.borrow_mut().clear();
        self.save();
    }
}
