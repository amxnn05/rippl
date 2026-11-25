//! What was open last time, written as plain text so it needs no parser
//! crate and stays readable by hand:
//!
//! ```text
//! rippl-session 1
//! front 2
//! tab 0 https://example.com/ Example Domain
//! tab 1 https://news.ycombinator.com/ Hacker News
//! ```
//!
//! `tab <pinned> <url> <title>`. URLs never contain spaces (WebKit
//! percent-encodes them); titles have line breaks flattened. Private tabs
//! are never written.

use crate::tabs::{Tab, TabId, Tabs};

const HEADER: &str = "rippl-session 1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub url: String,
    pub title: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    pub tabs: Vec<Saved>,
    /// Index into `tabs` of the tab that was in front.
    pub front: Option<usize>,
}

impl Session {
    /// The tabs worth keeping from `tabs`: everything but private tabs.
    pub fn of(tabs: &Tabs) -> Session {
        let kept: Vec<&Tab> = tabs.list.iter().filter(|t| !t.private).collect();
        let front = tabs.front.and_then(|f| kept.iter().position(|t| t.id == f));
        Session {
            tabs: kept
                .iter()
                .map(|t| Saved { url: t.url.clone(), title: t.title.clone(), pinned: t.pinned })
                .collect(),
            front,
        }
    }

    pub fn write(&self) -> String {
        let mut out = format!("{HEADER}\n");
        if let Some(f) = self.front {
            out.push_str(&format!("front {f}\n"));
        }
        for t in &self.tabs {
            let title: String = t.title.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
            out.push_str(&format!("tab {} {} {}\n", u8::from(t.pinned), t.url, title.trim()));
        }
        out
    }

    /// Reads what `write` wrote. Lines it does not understand are skipped,
    /// so a damaged file loses those tabs, not the whole session.
    pub fn read(text: &str) -> Session {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return Session::default();
        }
        let mut session = Session::default();
        for line in lines {
            if let Some(f) = line.strip_prefix("front ") {
                session.front = f.trim().parse().ok();
            } else if let Some(rest) = line.strip_prefix("tab ") {
                let mut parts = rest.splitn(3, ' ');
                let (Some(pin), Some(url)) = (parts.next(), parts.next()) else { continue };
                if url.is_empty() {
                    continue;
                }
                let title = parts.next().unwrap_or("").to_string();
                session.tabs.push(Saved { url: url.to_string(), title, pinned: pin == "1" });
            }
        }
        if session.front.is_some_and(|f| f >= session.tabs.len()) {
            session.front = None;
        }
        session
    }

    /// Puts the saved tabs into `tabs` as Lazy tabs: no view, no process,
    /// until each is first shown. Returns the id that should come to front.
    pub fn restore_into(&self, tabs: &mut Tabs) -> Option<TabId> {
        let mut front = None;
        for (i, saved) in self.tabs.iter().enumerate() {
            let id = tabs.push(&saved.url);
            if let Some(t) = tabs.get_mut(id) {
                if !saved.title.is_empty() {
                    t.title = saved.title.clone();
                }
                t.pinned = saved.pinned;
            }
            if Some(i) == self.front {
                front = Some(id);
            }
        }
        front.or_else(|| tabs.list.last().map(|t| t.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_skips_private_and_keeps_front() {
        let mut tabs = Tabs::new();
        let a = tabs.open("https://a.com/");
        let p = tabs.open("https://secret.com/");
        tabs.get_mut(p).unwrap().private = true;
        let b = tabs.open("https://b.com/");
        tabs.get_mut(b).unwrap().title = "B\nsite".into();
        tabs.get_mut(a).unwrap().pinned = true;
        tabs.select(b);

        let text = Session::of(&tabs).write();
        assert!(!text.contains("secret"));
        let back = Session::read(&text);
        assert_eq!(back.tabs.len(), 2);
        assert_eq!(back.front, Some(1));
        assert_eq!(back.tabs[1].title, "B site");
        assert!(back.tabs[0].pinned);

        let mut fresh = Tabs::new();
        let front = back.restore_into(&mut fresh).unwrap();
        assert_eq!(fresh.get(front).unwrap().url, "https://b.com/");
        assert!(fresh.list.iter().all(|t| t.life == crate::tabs::Life::Lazy));
    }

    #[test]
    fn junk_is_empty_and_bad_lines_are_skipped() {
        assert_eq!(Session::read("nonsense"), Session::default());
        let s = Session::read("rippl-session 1\nfront 9\ntab 0\ntab 0 https://ok.com/ Ok\n");
        assert_eq!(s.tabs.len(), 1);
        assert_eq!(s.front, None);
    }
}
