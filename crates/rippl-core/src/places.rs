//! Places: where you have been (history), what you kept (bookmarks), and
//! what you allowed sites to use (permissions). One SQLite file,
//! `~/.local/share/rippl/places.sqlite`, on this machine only.
//!
//! SQLite rather than JSON files: writes are small and crash-safe (WAL), and
//! a query for the address field over years of history stays in the
//! low milliseconds without loading it all into memory.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

pub use rusqlite::Error;

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A page from history or bookmarks.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub url: String,
    pub title: String,
    pub visits: i64,
    /// Unix seconds of the last visit (or when bookmarked).
    pub last: i64,
    pub bookmarked: bool,
}

/// A site permission kind, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Camera,
    Microphone,
    CameraAndMicrophone,
    Screen,
    Location,
    Notifications,
    Clipboard,
    Drm,
    Storage,
}

impl Permission {
    pub fn key(self) -> &'static str {
        match self {
            Permission::Camera => "camera",
            Permission::Microphone => "microphone",
            Permission::CameraAndMicrophone => "camera+microphone",
            Permission::Screen => "screen",
            Permission::Location => "location",
            Permission::Notifications => "notifications",
            Permission::Clipboard => "clipboard",
            Permission::Drm => "drm",
            Permission::Storage => "storage",
        }
    }

    /// How a prompt says it: "<site> wants to <ask>".
    pub fn ask(self) -> &'static str {
        match self {
            Permission::Camera => "use your camera",
            Permission::Microphone => "use your microphone",
            Permission::CameraAndMicrophone => "use your camera and microphone",
            Permission::Screen => "share your screen",
            Permission::Location => "know your location",
            Permission::Notifications => "send you notifications",
            Permission::Clipboard => "read your clipboard",
            Permission::Drm => "play protected (DRM) content",
            Permission::Storage => "use its cookies inside other sites",
        }
    }
}

pub struct Places {
    db: Connection,
}

impl Places {
    /// Opens (or creates) the database at `path`; `None` for in memory.
    pub fn open(path: Option<&std::path::Path>) -> Result<Places, Error> {
        let db = match path {
            Some(p) => Connection::open(p)?,
            None => Connection::open_in_memory()?,
        };
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS history (
                 url TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
                 visits INTEGER NOT NULL DEFAULT 0, typed INTEGER NOT NULL DEFAULT 0,
                 last INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS history_last ON history(last);
             CREATE TABLE IF NOT EXISTS bookmarks (
                 url TEXT PRIMARY KEY, title TEXT NOT NULL DEFAULT '', added INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS permissions (
                 host TEXT NOT NULL, kind TEXT NOT NULL, allow INTEGER NOT NULL,
                 PRIMARY KEY (host, kind));",
        )?;
        Ok(Places { db })
    }

    // ---- history -------------------------------------------------------

    /// A page finished loading. Only http(s) pages are kept.
    pub fn visit(&self, url: &str, title: &str, typed: bool) -> Result<(), Error> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Ok(());
        }
        self.db.execute(
            "INSERT INTO history (url, title, visits, typed, last) VALUES (?1, ?2, 1, ?3, ?4)
             ON CONFLICT(url) DO UPDATE SET visits = visits + 1, typed = typed + ?3, last = ?4,
                 title = CASE WHEN ?2 = '' THEN title ELSE ?2 END",
            params![url, title, typed as i64, now()],
        )?;
        Ok(())
    }

    /// The page's title arrived after it was recorded.
    pub fn retitle(&self, url: &str, title: &str) -> Result<(), Error> {
        if title.is_empty() {
            return Ok(());
        }
        self.db.execute("UPDATE history SET title = ?2 WHERE url = ?1", params![url, title])?;
        self.db.execute("UPDATE bookmarks SET title = ?2 WHERE url = ?1 AND title = ''", params![url, title])?;
        Ok(())
    }

    /// Forgets history: all of it, or what was visited since `since`.
    pub fn clear_history(&self, since: Option<i64>) -> Result<usize, Error> {
        self.db.execute("DELETE FROM history WHERE last >= ?1", params![since.unwrap_or(i64::MIN)])
    }

    /// Up to `limit` candidates for what was typed: every word must be in
    /// the URL or the title. Ranked in Rust by `rank`.
    pub fn search(&self, typed: &str, limit: usize) -> Result<Vec<Place>, Error> {
        let words: Vec<String> = typed
            .split_whitespace()
            .map(|w| format!("%{}%", w.to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")))
            .collect();
        let mut clause = String::new();
        for i in 0..words.len() {
            clause.push_str(&format!(
                " AND (lower(url) LIKE ?{n} ESCAPE '\\' OR lower(title) LIKE ?{n} ESCAPE '\\')",
                n = i + 1
            ));
        }
        let sql = format!(
            "SELECT url, title, visits + 2 * typed, last, 0 FROM history WHERE 1 {clause}
             UNION ALL
             SELECT url, title, 3, added, 1 FROM bookmarks WHERE 1 {clause}
             ORDER BY 4 DESC LIMIT 400"
        );
        let mut stmt = self.db.prepare_cached(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(words.iter()), |r| {
            Ok(Place {
                url: r.get(0)?,
                title: r.get(1)?,
                visits: r.get(2)?,
                last: r.get(3)?,
                bookmarked: r.get::<_, i64>(4)? == 1,
            })
        })?;
        // A page both visited and bookmarked: one entry, bookmarked.
        let mut out: Vec<Place> = Vec::new();
        for place in rows {
            let place = place?;
            match out.iter_mut().find(|p| p.url == place.url) {
                Some(existing) => {
                    existing.bookmarked |= place.bookmarked;
                    existing.visits = existing.visits.max(place.visits);
                    existing.last = existing.last.max(place.last);
                    if existing.title.is_empty() {
                        existing.title = place.title;
                    }
                }
                None => out.push(place),
            }
        }
        Ok(crate::suggest::rank(typed, out, now(), limit))
    }

    /// The most recently visited pages.
    pub fn recent(&self, limit: usize) -> Result<Vec<Place>, Error> {
        let mut stmt = self.db.prepare_cached(
            "SELECT h.url, h.title, h.visits, h.last, b.url IS NOT NULL
             FROM history h LEFT JOIN bookmarks b ON b.url = h.url ORDER BY h.last DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(Place { url: r.get(0)?, title: r.get(1)?, visits: r.get(2)?, last: r.get(3)?, bookmarked: r.get(4)? })
        })?;
        rows.collect()
    }

    // ---- bookmarks -----------------------------------------------------

    pub fn is_bookmarked(&self, url: &str) -> Result<bool, Error> {
        Ok(self.db.query_row("SELECT 1 FROM bookmarks WHERE url = ?1", [url], |_| Ok(())).optional()?.is_some())
    }

    /// Bookmarks `url`, or removes it if it is one. Returns whether it is now.
    pub fn toggle_bookmark(&self, url: &str, title: &str) -> Result<bool, Error> {
        if self.is_bookmarked(url)? {
            self.db.execute("DELETE FROM bookmarks WHERE url = ?1", [url])?;
            Ok(false)
        } else {
            self.db.execute("INSERT INTO bookmarks (url, title, added) VALUES (?1, ?2, ?3)", params![url, title, now()])?;
            Ok(true)
        }
    }

    pub fn bookmarks(&self) -> Result<Vec<Place>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT url, title, added FROM bookmarks ORDER BY added DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Place { url: r.get(0)?, title: r.get(1)?, visits: 0, last: r.get(2)?, bookmarked: true })
        })?;
        rows.collect()
    }

    // ---- permissions ---------------------------------------------------

    /// What was answered for `host` and `kind`, if anything.
    pub fn permission(&self, host: &str, kind: Permission) -> Result<Option<bool>, Error> {
        self.db
            .query_row("SELECT allow FROM permissions WHERE host = ?1 AND kind = ?2", params![host, kind.key()], |r| {
                r.get::<_, bool>(0)
            })
            .optional()
    }

    pub fn set_permission(&self, host: &str, kind: Permission, allow: bool) -> Result<(), Error> {
        self.db.execute(
            "INSERT INTO permissions (host, kind, allow) VALUES (?1, ?2, ?3)
             ON CONFLICT(host, kind) DO UPDATE SET allow = ?3",
            params![host, kind.key(), allow],
        )?;
        Ok(())
    }

    /// Forgets every answer for `host` (or for every site).
    pub fn forget_permissions(&self, host: Option<&str>) -> Result<usize, Error> {
        match host {
            Some(h) => self.db.execute("DELETE FROM permissions WHERE host = ?1", [h]),
            None => self.db.execute("DELETE FROM permissions", []),
        }
    }

    pub fn permissions(&self) -> Result<Vec<(String, String, bool)>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT host, kind, allow FROM permissions ORDER BY host, kind")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_counts_and_titles() {
        let p = Places::open(None).unwrap();
        p.visit("https://example.com/", "", false).unwrap();
        p.retitle("https://example.com/", "Example").unwrap();
        p.visit("https://example.com/", "", true).unwrap();
        p.visit("about:blank", "", false).unwrap();
        p.visit("file:///etc/passwd", "", false).unwrap();
        let all = p.recent(10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "Example");
        assert_eq!(all[0].visits, 2);
    }

    #[test]
    fn search_needs_every_word_and_escapes_like() {
        let p = Places::open(None).unwrap();
        p.visit("https://docs.rs/gtk4", "gtk4 Rust docs", false).unwrap();
        p.visit("https://example.com/100%", "Percent", false).unwrap();
        p.visit("https://rust-lang.org/", "Rust", false).unwrap();
        assert_eq!(p.search("rust docs", 10).unwrap().len(), 1);
        assert_eq!(p.search("rust", 10).unwrap().len(), 2);
        assert_eq!(p.search("%", 10).unwrap().len(), 1, "% is literal");
        assert_eq!(p.search("_", 10).unwrap().len(), 0, "_ is literal");
    }

    #[test]
    fn bookmarks_merge_into_search() {
        let p = Places::open(None).unwrap();
        p.visit("https://news.ycombinator.com/", "Hacker News", false).unwrap();
        assert!(p.toggle_bookmark("https://news.ycombinator.com/", "HN").unwrap());
        let found = p.search("news", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].bookmarked);
        assert!(!p.toggle_bookmark("https://news.ycombinator.com/", "").unwrap());
        assert!(p.bookmarks().unwrap().is_empty());
    }

    #[test]
    fn permissions_remembered_per_host() {
        let p = Places::open(None).unwrap();
        assert_eq!(p.permission("meet.example", Permission::Camera).unwrap(), None);
        p.set_permission("meet.example", Permission::Camera, true).unwrap();
        p.set_permission("ads.example", Permission::Notifications, false).unwrap();
        assert_eq!(p.permission("meet.example", Permission::Camera).unwrap(), Some(true));
        assert_eq!(p.permission("ads.example", Permission::Notifications).unwrap(), Some(false));
        p.forget_permissions(Some("meet.example")).unwrap();
        assert_eq!(p.permission("meet.example", Permission::Camera).unwrap(), None);
    }

    #[test]
    fn clear_history() {
        let p = Places::open(None).unwrap();
        p.visit("https://a.com/", "", false).unwrap();
        assert_eq!(p.clear_history(None).unwrap(), 1);
        assert!(p.recent(5).unwrap().is_empty());
    }
}
