//! Bookmarks brought over from another browser on this machine.
//!
//! - Chromium family (Chrome, Chromium, Brave, Edge, Vivaldi): the
//!   `Bookmarks` JSON file in each profile.
//! - Firefox family (Firefox, LibreWolf, Zen): `places.sqlite` in each
//!   profile. Firefox keeps it locked while running, so it is copied
//!   (with its `-wal` journal) and the copy is read.
//!
//! Only reading: nothing in the other browser is changed. Folders come
//! along as paths ("Work/Docs"); the browser's own top-level folders
//! (bookmarks bar, other bookmarks, menu, toolbar) are left out of them.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

/// A bookmark from another browser, with its folder path ("" at the top,
/// "Work/Docs" nested).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub folder: String,
}

/// A browser profile found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub browser: &'static str,
    pub profile: String,
    pub path: PathBuf,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chromium,
    Firefox,
}

const CHROMIUM: &[(&str, &str)] = &[
    ("Chrome", ".config/google-chrome"),
    ("Chromium", ".config/chromium"),
    ("Brave", ".config/BraveSoftware/Brave-Browser"),
    ("Edge", ".config/microsoft-edge"),
    ("Vivaldi", ".config/vivaldi"),
];

const FIREFOX: &[(&str, &str)] = &[
    ("Firefox", ".mozilla/firefox"),
    ("Firefox", ".config/mozilla/firefox"),
    ("LibreWolf", ".librewolf"),
    ("Zen", ".zen"),
];

/// Every profile with bookmarks under `home`.
pub fn sources(home: &Path) -> Vec<Source> {
    let mut found = Vec::new();
    for (browser, dir) in CHROMIUM {
        let Ok(entries) = std::fs::read_dir(home.join(dir)) else { continue };
        let mut profiles: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        profiles.sort();
        for profile in profiles {
            let file = profile.join("Bookmarks");
            if file.is_file() {
                let name = profile.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                found.push(Source { browser, profile: name, path: file, kind: Kind::Chromium });
            }
        }
    }
    for (browser, dir) in FIREFOX {
        let Ok(entries) = std::fs::read_dir(home.join(dir)) else { continue };
        let mut profiles: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        profiles.sort();
        for profile in profiles {
            let file = profile.join("places.sqlite");
            if file.is_file() && !found.iter().any(|s: &Source| s.path == file) {
                let name = profile.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                found.push(Source { browser, profile: name, path: file, kind: Kind::Firefox });
            }
        }
    }
    found
}

/// The bookmarks in `source`. `scratch` is a folder to copy a locked
/// Firefox database into.
pub fn read(source: &Source, scratch: &Path) -> Result<Vec<Bookmark>, String> {
    match source.kind {
        Kind::Chromium => {
            let text = std::fs::read_to_string(&source.path).map_err(|e| e.to_string())?;
            chromium(&text)
        }
        Kind::Firefox => firefox(&source.path, scratch),
    }
}

/// Chromium's `Bookmarks` JSON.
pub fn chromium(json: &str) -> Result<Vec<Bookmark>, String> {
    let root: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    fn walk(node: &serde_json::Value, path: &str, out: &mut Vec<Bookmark>) {
        match node.get("type").and_then(|t| t.as_str()) {
            Some("url") => {
                if let Some(url) = node.get("url").and_then(|u| u.as_str()) {
                    let title = node.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    out.push(Bookmark { url: url.into(), title: title.into(), folder: path.into() });
                }
            }
            _ => {
                if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                    for child in children {
                        let name = child.get("name").and_then(|n| n.as_str()).unwrap_or("").replace('/', "-");
                        let sub = match (child.get("type").and_then(|t| t.as_str()), path.is_empty()) {
                            (Some("folder"), true) => name,
                            (Some("folder"), false) => format!("{path}/{name}"),
                            _ => path.to_string(),
                        };
                        walk(child, &sub, out);
                    }
                }
            }
        }
    }
    let roots = root.get("roots").ok_or("not a Chromium bookmarks file")?;
    for name in ["bookmark_bar", "other", "synced"] {
        if let Some(node) = roots.get(name) {
            walk(node, "", &mut out);
        }
    }
    Ok(out)
}

/// Firefox's `places.sqlite`, read from a copy.
pub fn firefox(path: &Path, scratch: &Path) -> Result<Vec<Bookmark>, String> {
    std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
    let copy = scratch.join("places-import.sqlite");
    let _ = std::fs::remove_file(copy.with_extension("sqlite-wal"));
    std::fs::copy(path, &copy).map_err(|e| e.to_string())?;
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    if wal.is_file() {
        std::fs::copy(&wal, format!("{}-wal", copy.display())).map_err(|e| e.to_string())?;
    }
    let result = firefox_db(&copy);
    let _ = std::fs::remove_file(&copy);
    let _ = std::fs::remove_file(format!("{}-wal", copy.display()));
    let _ = std::fs::remove_file(format!("{}-shm", copy.display()));
    result
}

fn firefox_db(path: &Path) -> Result<Vec<Bookmark>, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|e| e.to_string())?;
    // Folders: id → (parent, title). Firefox's own roots (root, menu,
    // toolbar, unfiled, mobile, tags) have a guid ending in "_____" and
    // are left out of paths.
    let mut folders: std::collections::HashMap<i64, (i64, String, bool)> = std::collections::HashMap::new();
    {
        let mut stmt = db
            .prepare("SELECT id, COALESCE(parent, 0), COALESCE(title, ''), COALESCE(guid, '') FROM moz_bookmarks WHERE type = 2")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, parent, title, guid) = row.map_err(|e| e.to_string())?;
            folders.insert(id, (parent, title, guid.ends_with("_____") || guid.is_empty() && parent == 0));
        }
    }
    let path_of = |mut id: i64| {
        let mut parts = Vec::new();
        for _ in 0..32 {
            let Some((parent, title, root)) = folders.get(&id) else { break };
            if *root {
                break;
            }
            parts.push(title.replace('/', "-"));
            id = *parent;
        }
        parts.reverse();
        parts.join("/")
    };
    // type 1 = a bookmark (2 is a folder, 3 a separator).
    let mut stmt = db
        .prepare(
            "SELECT p.url, COALESCE(b.title, p.title, ''), COALESCE(b.parent, 0) FROM moz_bookmarks b
             JOIN moz_places p ON p.id = b.fk WHERE b.type = 1 ORDER BY b.position",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (url, title, parent) = row.map_err(|e| e.to_string())?;
        out.push(Bookmark { url, title, folder: path_of(parent) });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_tree_is_flattened() {
        let json = r#"{"roots":{
            "bookmark_bar":{"type":"folder","children":[
                {"type":"url","name":"Rust","url":"https://rust-lang.org/"},
                {"type":"folder","name":"Work","children":[{"type":"url","name":"Docs","url":"https://docs.rs/"}]}]},
            "other":{"type":"folder","children":[{"type":"url","name":"HN","url":"https://news.ycombinator.com/"}]},
            "synced":{"type":"folder","children":[]}},"version":1}"#;
        let got = chromium(json).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[1], Bookmark { url: "https://docs.rs/".into(), title: "Docs".into(), folder: "Work".into() });
        assert_eq!(got[0].folder, "");
        assert!(chromium("{}").is_err());
    }

    #[test]
    fn firefox_bookmarks_only() {
        let dir = std::env::temp_dir().join(format!("rippl-import-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("places.sqlite");
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT);
             CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, title TEXT, position INTEGER, parent INTEGER, guid TEXT);
             INSERT INTO moz_places VALUES (1, 'https://mozilla.org/', 'Mozilla'), (2, 'https://visited.only/', 'V');
             INSERT INTO moz_bookmarks VALUES (1, 2, NULL, 'toolbar', 0, 0, 'toolbar_____'),
                 (3, 2, NULL, 'Work', 0, 1, 'abc'), (2, 1, 1, 'Moz', 0, 3, 'def');",
        )
        .unwrap();
        drop(db);
        let got = firefox(&path, &dir.join("scratch")).unwrap();
        assert_eq!(got, vec![Bookmark { url: "https://mozilla.org/".into(), title: "Moz".into(), folder: "Work".into() }]);
        assert!(path.is_file(), "the original is left alone");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_profiles() {
        let home = std::env::temp_dir().join(format!("rippl-home-test-{}", std::process::id()));
        let chrome = home.join(".config/google-chrome/Default");
        let ff = home.join(".mozilla/firefox/abc.default-release");
        std::fs::create_dir_all(&chrome).unwrap();
        std::fs::create_dir_all(&ff).unwrap();
        std::fs::write(chrome.join("Bookmarks"), "{}").unwrap();
        std::fs::write(ff.join("places.sqlite"), "").unwrap();
        let found = sources(&home);
        assert_eq!(found.iter().map(|s| (s.browser, s.kind)).collect::<Vec<_>>(), vec![
            ("Chrome", Kind::Chromium),
            ("Firefox", Kind::Firefox)
        ]);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
