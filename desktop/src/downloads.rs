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
    /// A video through yt-dlp: what it keeps, its title, how far along.
    pub grab: Option<rippl_core::ytdlp::Format>,
    pub title: String,
    pub fraction: f64,
    pub line: String,
    pub private: bool,
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
        self.live.borrow_mut().push(Live { id, url, path, started: Instant::now(), bytes: 0, done: None, why: String::new(), grab: None, title: String::new(), fraction: -1.0, line: String::new(), private: false });
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
        for d in self.live.borrow_mut().iter_mut().filter(|d| d.done.is_none() && d.grab.is_none()) {
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

    // ---- videos, through yt-dlp ------------------------------------------

    pub fn grab_started(&self, url: &str, format: rippl_core::ytdlp::Format, private: bool) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        self.live.borrow_mut().push(Live {
            id,
            url: url.to_string(),
            path: PathBuf::new(),
            started: Instant::now(),
            bytes: 0,
            done: None,
            why: String::new(),
            grab: Some(format),
            title: String::new(),
            fraction: -1.0,
            line: String::new(),
            private,
        });
        id
    }

    pub fn grab_line(&self, id: u64, line: rippl_core::ytdlp::Line) {
        use rippl_core::ytdlp::Line;
        let mut live = self.live.borrow_mut();
        let Some(d) = live.iter_mut().find(|d| d.id == id) else { return };
        match line {
            Line::Title(t) => {
                if t.ends_with('…') { d.line = t } else { d.title = t }
            }
            Line::Progress(p) => {
                if let Some(f) = p.fraction() {
                    d.fraction = f;
                }
                d.bytes = p.got;
                let mut s = match p.total {
                    Some(t) => format!("{} of {}", size(p.got), size(t)),
                    None => size(p.got),
                };
                if let Some(v) = p.speed {
                    s.push_str(&format!(" · {}/s", size(v as u64)));
                }
                if let Some(e) = p.eta {
                    s.push_str(&if e >= 60 { format!(" · {} min left", e / 60) } else { format!(" · {e} s left") });
                }
                d.line = s;
            }
            Line::Finishing => d.line = "Putting it together…".into(),
            Line::File(path) => d.path = path,
            Line::Error(e) => d.why = e,
            Line::Other => {}
        }
    }

    /// yt-dlp ended (or never began: `why`).
    pub fn grab_ended(&self, id: u64, ok: bool, why: &str) -> Option<String> {
        let mut live = self.live.borrow_mut();
        let d = live.iter_mut().find(|d| d.id == id && d.done.is_none())?;
        d.done = Some(ok);
        if !why.is_empty() {
            d.why = why.to_string();
        }
        if !ok && d.why.is_empty() {
            d.why = "yt-dlp stopped".into();
        }
        if ok {
            d.fraction = 1.0;
            d.bytes = std::fs::metadata(&d.path).map(|m| m.len()).unwrap_or(d.bytes);
        }
        let name = Self::name(d);
        let format = d.grab.map(|f| f.key()).unwrap_or("video");
        let entry = Entry {
            when: rippl_core::places::now(),
            done: ok,
            name: name.clone(),
            path: if ok { d.path.to_string_lossy().to_string() } else { String::new() },
            url: d.url.clone(),
            note: if ok { format!("{format} (yt-dlp)") } else { format!("{} (yt-dlp)", d.why) },
        };
        let private = d.private;
        drop(live);
        if !private && self.keep_list.get() {
            self.kept.borrow_mut().push(entry);
            self.save();
        }
        Some(name)
    }

    pub fn is_grab(&self, id: u64) -> bool {
        self.live.borrow().iter().any(|d| d.id == id && d.grab.is_some() && d.done.is_none())
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

    fn name(d: &Live) -> String {
        match d.path.file_name() {
            Some(n) if !n.is_empty() => n.to_string_lossy().to_string(),
            _ if !d.title.is_empty() => d.title.clone(),
            _ => d.url.clone(),
        }
    }

    fn status(d: &Live) -> String {
        if d.grab.is_some() && d.done.is_none() {
            return if d.line.is_empty() { "Starting yt-dlp…".into() } else { d.line.clone() };
        }
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
                        "name": Self::name(d),
                        "status": Self::status(d), "fraction": d.fraction,
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
                name: Self::name(d),
                status: Self::status(d),
                fraction: d.fraction,
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
