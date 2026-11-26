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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Place {
    pub url: String,
    pub title: String,
    pub visits: i64,
    /// Unix seconds of the last visit (or when bookmarked).
    pub last: i64,
    pub bookmarked: bool,
    /// The bookmark's folder ("" for none; "Work/Docs" for nested).
    pub folder: String,
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

/// The archive keeps a month, and 500 tabs at most.
const ARCHIVE_DAYS: i64 = 30;
const ARCHIVE_CAP: i64 = 500;

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
             CREATE TABLE IF NOT EXISTS hidden (
                 host TEXT NOT NULL, selector TEXT NOT NULL, added INTEGER NOT NULL,
                 PRIMARY KEY (host, selector));
             CREATE TABLE IF NOT EXISTS site_css (host TEXT PRIMARY KEY, css TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS site_flags (host TEXT NOT NULL, flag TEXT NOT NULL, PRIMARY KEY (host, flag));
             CREATE TABLE IF NOT EXISTS archive (url TEXT PRIMARY KEY, title TEXT NOT NULL, closed INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS never_save (origin TEXT PRIMARY KEY, added INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS permissions (
                 host TEXT NOT NULL, kind TEXT NOT NULL, allow INTEGER NOT NULL,
                 PRIMARY KEY (host, kind));",
        )?;
        // Added in Phase 7: bookmark folders. Old databases get the column.
        let has_folder: bool = db
            .prepare("SELECT 1 FROM pragma_table_info('bookmarks') WHERE name = 'folder'")?
            .exists([])?;
        if !has_folder {
            db.execute_batch("ALTER TABLE bookmarks ADD COLUMN folder TEXT NOT NULL DEFAULT ''")?;
        }
        Ok(Places { db })
    }

    // ---- the archive: tabs rippl closed by itself ------------------------

    /// A tab rippl just closed. A page already
    /// here moves to the top; past a month, or the last 500, they go.
    pub fn archive_tab(&self, url: &str, title: &str) -> Result<(), Error> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Ok(());
        }
        let now = now();
        self.db.execute(
            "INSERT INTO archive (url, title, closed) VALUES (?1, ?2, ?3)
             ON CONFLICT(url) DO UPDATE SET title = ?2, closed = ?3",
            params![url, title, now],
        )?;
        self.db.execute("DELETE FROM archive WHERE closed < ?1", params![now - ARCHIVE_DAYS * 86_400])?;
        self.db.execute(
            "DELETE FROM archive WHERE url NOT IN (SELECT url FROM archive ORDER BY closed DESC LIMIT ?1)",
            params![ARCHIVE_CAP],
        )?;
        Ok(())
    }

    /// The archive, newest first; with `q`, what has every word of it.
    pub fn archived(&self, q: &str) -> Result<Vec<Place>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT url, title, closed FROM archive WHERE closed >= ?1 ORDER BY closed DESC")?;
        let since = now() - ARCHIVE_DAYS * 86_400;
        let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
        let rows = stmt.query_map(params![since], |r| {
            Ok(Place {
                url: r.get(0)?,
                title: r.get(1)?,
                visits: 0,
                last: r.get(2)?,
                bookmarked: false,
                folder: String::new(),
            })
        })?;
        Ok(rows
            .filter_map(Result::ok)
            .filter(|p| {
                let hay = format!("{} {}", p.url, p.title).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect())
    }

    /// One out of the archive (opened again, or forgotten); `None`: all.
    pub fn unarchive(&self, url: Option<&str>) -> Result<usize, Error> {
        match url {
            Some(u) => self.db.execute("DELETE FROM archive WHERE url = ?1", params![u]),
            None => self.db.execute("DELETE FROM archive", []),
        }
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
        // The archive is history too.
        self.db.execute("DELETE FROM archive WHERE closed >= ?1", params![since.unwrap_or(i64::MIN)])?;
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
                folder: String::new(),
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
            Ok(Place {
                url: r.get(0)?,
                title: r.get(1)?,
                visits: r.get(2)?,
                last: r.get(3)?,
                bookmarked: r.get(4)?,
                folder: String::new(),
            })
        })?;
        rows.collect()
    }

    /// Forgets one page from history.
    pub fn forget(&self, url: &str) -> Result<usize, Error> {
        self.db.execute("DELETE FROM history WHERE url = ?1", [url])
    }

    // ---- bookmarks -----------------------------------------------------

    pub fn remove_bookmark(&self, url: &str) -> Result<usize, Error> {
        self.db.execute("DELETE FROM bookmarks WHERE url = ?1", [url])
    }

    /// Puts a bookmark in `folder` ("" for none).
    pub fn move_bookmark(&self, url: &str, folder: &str) -> Result<usize, Error> {
        let folder: String = folder.split('/').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("/");
        self.db.execute("UPDATE bookmarks SET folder = ?2 WHERE url = ?1", params![url, folder])
    }

    /// Adds bookmarks brought over from another browser, with their
    /// folders, skipping ones already kept and anything that isn't
    /// http(s). Returns how many were new.
    pub fn import_bookmarks(&self, list: &[crate::import::Bookmark]) -> Result<usize, Error> {
        let tx = self.db.unchecked_transaction()?;
        let mut added = 0;
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO bookmarks (url, title, added, folder) VALUES (?1, ?2, ?3, ?4)")?;
            let t = now();
            for b in list {
                if b.url.starts_with("https://") || b.url.starts_with("http://") {
                    added += stmt.execute(params![b.url, b.title, t, b.folder])?;
                }
            }
        }
        tx.commit()?;
        Ok(added)
    }

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
        let mut stmt =
            self.db.prepare_cached("SELECT url, title, added, folder FROM bookmarks ORDER BY folder, added DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Place { url: r.get(0)?, title: r.get(1)?, visits: 0, last: r.get(2)?, bookmarked: true, folder: r.get(3)? })
        })?;
        rows.collect()
    }

    // ---- hidden elements -------------------------------------------------

    /// Keeps `selector` hidden on `host` from now on.
    pub fn hide(&self, host: &str, selector: &str) -> Result<(), Error> {
        self.db.execute(
            "INSERT OR IGNORE INTO hidden (host, selector, added) VALUES (?1, ?2, ?3)",
            params![host, selector, now()],
        )?;
        Ok(())
    }

    /// Shows everything again on `host` (or on every site).
    pub fn unhide(&self, host: Option<&str>) -> Result<usize, Error> {
        match host {
            Some(h) => self.db.execute("DELETE FROM hidden WHERE host = ?1", [h]),
            None => self.db.execute("DELETE FROM hidden", []),
        }
    }

    /// Every site with something hidden, and its selectors.
    pub fn hidden(&self) -> Result<Vec<(String, Vec<String>)>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT host, selector FROM hidden ORDER BY host, rowid")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        for row in rows {
            let (host, selector) = row?;
            match out.last_mut() {
                Some((h, list)) if *h == host => list.push(selector),
                _ => out.push((host, vec![selector])),
            }
        }
        Ok(out)
    }

    // ---- per-site switches: keep awake, pop-ups allowed -------------------

    pub fn site_flag(&self, host: &str, flag: &str) -> Result<bool, Error> {
        Ok(self
            .db
            .query_row("SELECT 1 FROM site_flags WHERE host = ?1 AND flag = ?2", params![host, flag], |_| Ok(()))
            .optional()?
            .is_some())
    }

    pub fn set_site_flag(&self, host: &str, flag: &str, on: bool) -> Result<(), Error> {
        if on {
            self.db.execute("INSERT OR IGNORE INTO site_flags (host, flag) VALUES (?1, ?2)", params![host, flag])?;
        } else {
            self.db.execute("DELETE FROM site_flags WHERE host = ?1 AND flag = ?2", params![host, flag])?;
        }
        Ok(())
    }

    /// Every (host, flag) set, for rippl://settings.
    pub fn site_flags(&self) -> Result<Vec<(String, String)>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT host, flag FROM site_flags ORDER BY host, flag")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    // ---- your own style for a site -----------------------

    pub fn site_css(&self, host: &str) -> Result<Option<String>, Error> {
        self.db.query_row("SELECT css FROM site_css WHERE host = ?1", [host], |r| r.get(0)).optional()
    }

    /// Sets `host`'s CSS; empty removes it.
    pub fn set_site_css(&self, host: &str, css: &str) -> Result<(), Error> {
        if css.trim().is_empty() {
            self.db.execute("DELETE FROM site_css WHERE host = ?1", [host])?;
        } else {
            self.db.execute(
                "INSERT INTO site_css (host, css) VALUES (?1, ?2) ON CONFLICT(host) DO UPDATE SET css = ?2",
                params![host, css],
            )?;
        }
        Ok(())
    }

    pub fn all_site_css(&self) -> Result<Vec<(String, String)>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT host, css FROM site_css ORDER BY host")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    // ---- passwords: sites never to offer saving on ----------------------

    pub fn never_save(&self, origin: &str) -> Result<bool, Error> {
        Ok(self.db.query_row("SELECT 1 FROM never_save WHERE origin = ?1", [origin], |_| Ok(())).optional()?.is_some())
    }

    pub fn set_never_save(&self, origin: &str, never: bool) -> Result<(), Error> {
        if never {
            self.db.execute("INSERT OR IGNORE INTO never_save (origin, added) VALUES (?1, ?2)", params![origin, now()])?;
        } else {
            self.db.execute("DELETE FROM never_save WHERE origin = ?1", [origin])?;
        }
        Ok(())
    }

    pub fn never_save_list(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self.db.prepare_cached("SELECT origin FROM never_save ORDER BY origin")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
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
    fn hidden_by_host() {
        let p = Places::open(None).unwrap();
        p.hide("news.com", ".promo").unwrap();
        p.hide("news.com", "#popup").unwrap();
        p.hide("news.com", ".promo").unwrap();
        p.hide("blog.org", "aside").unwrap();
        let all = p.hidden().unwrap();
        assert_eq!(all, vec![
            ("blog.org".to_string(), vec!["aside".to_string()]),
            ("news.com".to_string(), vec![".promo".to_string(), "#popup".to_string()]),
        ]);
        p.unhide(Some("news.com")).unwrap();
        assert_eq!(p.hidden().unwrap().len(), 1);
    }

    #[test]
    fn import_skips_duplicates_and_odd_schemes() {
        let p = Places::open(None).unwrap();
        p.toggle_bookmark("https://a.com/", "A").unwrap();
        let b = |url: &str, folder: &str| crate::import::Bookmark { url: url.into(), title: "t".into(), folder: folder.into() };
        let list = vec![b("https://a.com/", ""), b("https://b.com/", "Work/Docs"), b("javascript:alert(1)", ""), b("place:sort=8", "")];
        assert_eq!(p.import_bookmarks(&list).unwrap(), 1);
        let all = p.bookmarks().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all.iter().find(|x| x.url == "https://b.com/").unwrap().folder, "Work/Docs");
        p.move_bookmark("https://a.com/", " Reading / Later ").unwrap();
        assert_eq!(p.bookmarks().unwrap().iter().find(|x| x.url == "https://a.com/").unwrap().folder, "Reading/Later");
    }

    #[test]
    fn forget_one_page() {
        let p = Places::open(None).unwrap();
        p.visit("https://a.com/", "", false).unwrap();
        p.visit("https://b.com/", "", false).unwrap();
        p.forget("https://a.com/").unwrap();
        assert_eq!(p.recent(5).unwrap().len(), 1);
    }

    #[test]
    fn site_css_set_and_cleared() {
        let p = Places::open(None).unwrap();
        p.set_site_css("news.com", "body { font-size: 20px }").unwrap();
        assert_eq!(p.site_css("news.com").unwrap().as_deref(), Some("body { font-size: 20px }"));
        p.set_site_css("news.com", "body { color: red }").unwrap();
        assert_eq!(p.all_site_css().unwrap().len(), 1);
        p.set_site_css("news.com", "  ").unwrap();
        assert_eq!(p.site_css("news.com").unwrap(), None);
    }

    #[test]
    fn site_flags_by_host() {
        let p = Places::open(None).unwrap();
        p.set_site_flag("docs.example", "awake", true).unwrap();
        p.set_site_flag("hark.com", "popups", true).unwrap();
        assert!(p.site_flag("docs.example", "awake").unwrap());
        assert!(!p.site_flag("docs.example", "popups").unwrap());
        p.set_site_flag("docs.example", "awake", false).unwrap();
        assert_eq!(p.site_flags().unwrap(), vec![("hark.com".to_string(), "popups".to_string())]);
    }

    #[test]
    fn never_save_list() {
        let p = Places::open(None).unwrap();
        p.set_never_save("https://bank.com", true).unwrap();
        assert!(p.never_save("https://bank.com").unwrap());
        assert!(!p.never_save("https://other.com").unwrap());
        assert_eq!(p.never_save_list().unwrap(), vec!["https://bank.com".to_string()]);
        p.set_never_save("https://bank.com", false).unwrap();
        assert!(p.never_save_list().unwrap().is_empty());
    }

    #[test]
    fn archive_keeps_newest_once() {
        let p = Places::open(None).unwrap();
        p.archive_tab("https://a.com/", "A").unwrap();
        p.archive_tab("https://b.com/", "Bee").unwrap();
        p.archive_tab("https://a.com/", "A again").unwrap();
        p.archive_tab("rippl://history", "no").unwrap();
        let all = p.archived("").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(p.archived("bee").unwrap().len(), 1);
        p.unarchive(Some("https://b.com/")).unwrap();
        assert_eq!(p.archived("").unwrap().len(), 1);
        p.clear_history(None).unwrap();
        assert!(p.archived("").unwrap().is_empty());
    }

    #[test]
    fn clear_history() {
        let p = Places::open(None).unwrap();
        p.visit("https://a.com/", "", false).unwrap();
        assert_eq!(p.clear_history(None).unwrap(), 1);
        assert!(p.recent(5).unwrap().is_empty());
    }
}
