//! Sync through a folder: `sync_folder = <path>`,
//! any folder something else keeps the same on your devices (Syncthing,
//! Nextcloud, Dropbox, a shared disk). rippl writes
//! `<folder>/rippl sync/<profile>/<device>.json` and reads the other
//! devices' files there, at start, every two minutes, and on demand.
//!
//! What syncs: bookmarks (address, title, folder), pinned tabs (address,
//! title, space, pin folder), spaces (name and colour; a space is never
//! deleted by sync, as that would take its tabs and sign-ins), and the
//! settings that are about how you like rippl, not about this machine.
//! What never does: history, passwords, cookies and sign-ins, open tabs
//! that aren't pinned, private tabs, the proxy, the sync folder itself.
//! The merge is `rippl_core::sync`: item by item, the newest wins, and a
//! deletion travels as a tombstone.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use rippl_core::sync::{self, Change, Item};
use rippl_core::tabs::Life;
use serde_json::json;
use webkit6::glib;

use crate::browser::Browser;

/// The settings that travel.
const SETTINGS: [&str; 22] = [
    "search", "shield", "ublock", "dusk", "icons", "home_grid", "video_keys", "peek", "player", "autoplay", "autoscroll",
    "close_after", "bookmarks_bar", "tabs", "column", "tab_switcher", "frame_rate", "home_theme", "home_colour", "webmail",
    "theme", "suggestions",
];

/// How often the folder is looked at again.
const EVERY: Duration = Duration::from_secs(120);

/// This device's id, made once (random), and its name.
fn device() -> (String, String) {
    let file = crate::engine::data_dir().join("device");
    let id = std::fs::read_to_string(&file).ok().map(|s| s.trim().to_string()).filter(|s| s.len() == 16);
    let id = id.unwrap_or_else(|| {
        let made: String = (0..16).map(|_| format!("{:x}", glib::random_int_range(0, 16))).collect();
        let _ = std::fs::create_dir_all(crate::engine::data_dir());
        let _ = std::fs::write(&file, &made);
        made
    });
    (id, glib::host_name().to_string())
}

fn mirror_file() -> PathBuf {
    crate::engine::data_dir().join("sync-mirror.json")
}

/// Writes `text` to `path` through a temporary file, so a reader never
/// sees half of it.
fn write_whole(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl Browser {
    /// Starts syncing, if a folder is set: now, then every two minutes.
    pub(crate) fn start_sync(&self) {
        if !self.primary {
            return;
        }
        self.sync_now();
        let weak = self.weak();
        glib::timeout_add_local(EVERY, move || {
            let Some(b) = weak.upgrade() else { return glib::ControlFlow::Break };
            b.sync_now();
            glib::ControlFlow::Continue
        });
    }

    /// What this device has now, scope by scope.
    fn sync_current(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        let mut scopes = BTreeMap::new();
        let bookmarks = self
            .places
            .bookmarks()
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.url, json!({"title": p.title, "folder": p.folder}).to_string()))
            .collect();
        scopes.insert("bookmarks".into(), bookmarks);
        let tabs = self.tabs.borrow();
        let space_name = |id| tabs.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        let pins = tabs
            .list
            .iter()
            .filter(|t| t.pinned && !t.private && !t.url.is_empty())
            .map(|t| {
                (
                    t.url.clone(),
                    json!({"title": t.title, "space": space_name(t.space), "folder": t.pin_folder.clone().unwrap_or_default()})
                        .to_string(),
                )
            })
            .collect();
        scopes.insert("pins".into(), pins);
        let spaces = tabs.spaces.iter().map(|s| (s.name.clone(), json!(s.colours).to_string())).collect();
        scopes.insert("spaces".into(), spaces);
        drop(tabs);
        let config = self.config.borrow();
        let mut settings: BTreeMap<String, String> =
            config.rows().into_iter().filter(|r| SETTINGS.contains(&r.key.as_str())).map(|r| (r.key, r.value)).collect();
        for (name, keys) in &config.shortcuts {
            settings.insert(format!("shortcut.{name}"), keys.clone());
        }
        scopes.insert("settings".into(), settings);
        scopes
    }

    /// One sync: read, merge, apply, write. Says why not, if it can't.
    pub(crate) fn sync_now(&self) -> String {
        let folder = self.config.borrow().sync_folder.clone();
        if folder.is_empty() {
            return "sync is off (sync_folder)".into();
        }
        let root = PathBuf::from(&folder);
        if !root.is_dir() {
            return format!("the sync folder {folder} isn't there");
        }
        let dir = root.join("rippl sync").join(crate::engine::profile().unwrap_or("default"));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return format!("can't write in the sync folder: {e}");
        }
        let (me, name) = device();
        let now = rippl_core::places::now();
        let mirror = std::fs::read_to_string(mirror_file()).ok().and_then(|t| sync::read(&t)).map(|(_, s)| s).unwrap_or_default();
        // Every other device's file.
        let mut others: Vec<BTreeMap<String, Vec<Item>>> = Vec::new();
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") || path.file_stem().and_then(|s| s.to_str()) == Some(&me) {
                continue;
            }
            if let Some((device, scopes)) = std::fs::read_to_string(&path).ok().and_then(|t| sync::read(&t))
                && device != me
            {
                others.push(scopes);
            }
        }
        let current = self.sync_current();
        let mut written = BTreeMap::new();
        let mut applied = 0;
        for (scope, now_here) in &current {
            let theirs: Vec<Vec<Item>> = others.iter().filter_map(|o| o.get(scope).cloned()).collect();
            let (kept, changes) = sync::merge(mirror.get(scope).map(Vec::as_slice).unwrap_or(&[]), now_here, &theirs, now);
            applied += changes.len();
            if std::env::var_os("RIPPL_DEBUG").is_some() {
                for c in &changes {
                    eprintln!("rippl: sync {scope}: {c:?}");
                }
            }
            self.apply_sync(scope, &changes);
            written.insert(scope.clone(), kept);
        }
        let text = sync::write(&me, &name, now, &written);
        let _ = write_whole(&mirror_file(), &text);
        if let Err(e) = write_whole(&dir.join(format!("{me}.json")), &text) {
            return format!("can't write in the sync folder: {e}");
        }
        if applied > 0 {
            self.dirty.set(true);
            self.render();
            self.fill_bookmarks_bar();
            self.apply_space_look();
        }
        format!("synced with {} other device{}; {applied} change{} here", others.len(), if others.len() == 1 { "" } else { "s" }, if applied == 1 { "" } else { "s" })
    }

    fn apply_sync(&self, scope: &str, changes: &[Change]) {
        let field = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        for change in changes {
            match (scope, change) {
                ("bookmarks", Change::Set(url, v)) => {
                    let v: serde_json::Value = serde_json::from_str(v).unwrap_or_default();
                    let _ = self.places.put_bookmark(url, &field(&v, "title"), &field(&v, "folder"));
                }
                ("bookmarks", Change::Remove(url)) => {
                    let _ = self.places.remove_bookmark(url);
                }
                ("pins", Change::Set(url, v)) => {
                    let v: serde_json::Value = serde_json::from_str(v).unwrap_or_default();
                    let space_name = field(&v, "space");
                    let folder = field(&v, "folder");
                    let mut tabs = self.tabs.borrow_mut();
                    let space = match tabs.spaces.iter().find(|s| s.name == space_name) {
                        Some(s) => s.id,
                        None if !space_name.is_empty() => tabs.add_space(&space_name),
                        None => tabs.spaces[0].id,
                    };
                    let existing = tabs.list.iter().find(|t| t.pinned && t.url == *url && !t.private).map(|t| t.id);
                    let id = match existing {
                        Some(id) => id,
                        None => {
                            let here = tabs.space;
                            tabs.space = space;
                            let id = tabs.push(url);
                            tabs.space = here;
                            if let Some(t) = tabs.get_mut(id) {
                                t.space = space;
                                t.life = Life::Lazy;
                                t.title = field(&v, "title");
                            }
                            tabs.set_pinned(id, true);
                            id
                        }
                    };
                    tabs.set_pin_folder(id, if folder.is_empty() { None } else { Some(&folder) });
                }
                ("pins", Change::Remove(url)) => {
                    let gone = self.tabs.borrow().list.iter().find(|t| t.pinned && t.url == *url && !t.private).map(|t| t.id);
                    if let Some(id) = gone {
                        self.close(id);
                    }
                }
                ("spaces", Change::Set(name, colours)) => {
                    let colours: Vec<String> = serde_json::from_str(colours).unwrap_or_default();
                    let mut tabs = self.tabs.borrow_mut();
                    let id = match tabs.spaces.iter().find(|s| s.name == *name) {
                        Some(s) => s.id,
                        None => tabs.add_space(name),
                    };
                    tabs.colour_space(id, &colours);
                }
                ("settings", Change::Set(key, value)) => {
                    if let Err(e) = self.change_setting(key, value) {
                        eprintln!("rippl: sync: {key}: {e}");
                    }
                }
                // A space is never deleted by sync, nor a setting unset.
                _ => {}
            }
        }
    }
}
