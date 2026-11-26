//! Sync through a folder: no
//! account and no server. Each device writes one file into a folder you
//! choose — synced by Syncthing, Nextcloud, Dropbox, a shared disk — and
//! reads the others'. Only that device ever writes its file, so two never
//! write the same one.
//!
//! What syncs is items: a key (a bookmark's address, a pin's address, a
//! setting's name), a value, the time it last changed (its stamp), and
//! whether it was deleted (a tombstone, so a deletion travels too). Each
//! device keeps what it last saw (its mirror), finds what changed here since
//! by comparing, and takes, item by item, the newest of everything every
//! device has. Tombstones are dropped after 60 days.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// One synced thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub key: String,
    /// JSON; empty for a tombstone.
    pub value: String,
    /// Unix seconds of its last change.
    pub stamp: i64,
    pub gone: bool,
}

/// What a merge asks of this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Set(String, String),
    Remove(String),
}

/// How long a deletion is remembered.
const TOMBSTONE_DAYS: i64 = 60;

/// One scope's merge. `mirror`: what this device last synced; `current`:
/// what it has now (key → value); `others`: the other devices' items.
/// Returns the new mirror (to keep, and to write as this device's file),
/// and what to change here.
///
/// A device's first sync (no mirror yet) stamps what it has as oldest of
/// all: a new device's untouched defaults must not overwrite the settings
/// the others already share. What only it has still travels.
pub fn merge(mirror: &[Item], current: &BTreeMap<String, String>, others: &[Vec<Item>], now: i64) -> (Vec<Item>, Vec<Change>) {
    let stamp_here = if mirror.is_empty() { 0 } else { now };
    // What changed here since the last sync.
    let mut mine: BTreeMap<String, Item> = BTreeMap::new();
    let old: BTreeMap<&str, &Item> = mirror.iter().map(|i| (i.key.as_str(), i)).collect();
    for (key, value) in current {
        let item = match old.get(key.as_str()) {
            Some(i) if !i.gone && i.value == *value => (*i).clone(),
            _ => Item { key: key.clone(), value: value.clone(), stamp: stamp_here, gone: false },
        };
        mine.insert(key.clone(), item);
    }
    for i in mirror {
        if current.contains_key(&i.key) {
            continue;
        }
        let gone = if i.gone { i.clone() } else { Item { key: i.key.clone(), value: String::new(), stamp: now, gone: true } };
        mine.insert(i.key.clone(), gone);
    }
    // The newest of each, everywhere. A tie goes to a tombstone, then to
    // the larger value, so every device picks the same.
    let mut winners = mine.clone();
    for list in others {
        for i in list {
            let better = match winners.get(&i.key) {
                None => true,
                Some(w) => (i.stamp, i.gone, &i.value) > (w.stamp, w.gone, &w.value),
            };
            if better {
                winners.insert(i.key.clone(), i.clone());
            }
        }
    }
    let mut changes = Vec::new();
    for (key, w) in &winners {
        match (w.gone, current.get(key)) {
            (true, Some(_)) => changes.push(Change::Remove(key.clone())),
            (false, Some(v)) if *v != w.value => changes.push(Change::Set(key.clone(), w.value.clone())),
            (false, None) => changes.push(Change::Set(key.clone(), w.value.clone())),
            _ => {}
        }
    }
    let kept = winners.into_values().filter(|i| !i.gone || now - i.stamp < TOMBSTONE_DAYS * 86_400).collect();
    (kept, changes)
}

/// A device's file: `{"device", "name", "written", "scopes": {scope: [items]}}`.
pub fn write(device: &str, name: &str, now: i64, scopes: &BTreeMap<String, Vec<Item>>) -> String {
    let scopes: serde_json::Map<String, Value> = scopes
        .iter()
        .map(|(s, items)| {
            let list: Vec<Value> = items
                .iter()
                .map(|i| json!({"key": i.key, "value": i.value, "stamp": i.stamp, "gone": i.gone}))
                .collect();
            (s.clone(), Value::Array(list))
        })
        .collect();
    serde_json::to_string_pretty(&json!({"device": device, "name": name, "written": now, "scopes": scopes})).unwrap_or_default()
}

/// A device's file read: its id and its scopes. Anything malformed is
/// skipped, item by item.
pub fn read(text: &str) -> Option<(String, BTreeMap<String, Vec<Item>>)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let device = v.get("device")?.as_str()?.to_string();
    let mut scopes = BTreeMap::new();
    for (scope, list) in v.get("scopes")?.as_object()? {
        let items = list
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| {
                Some(Item {
                    key: i.get("key")?.as_str()?.to_string(),
                    value: i.get("value")?.as_str()?.to_string(),
                    stamp: i.get("stamp")?.as_i64()?,
                    gone: i.get("gone")?.as_bool()?,
                })
            })
            .collect();
        scopes.insert(scope.clone(), items);
    }
    Some((device, scopes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn two_devices_converge() {
        // Device A has a and b; B has c. First syncs.
        let (a1, ch) = merge(&[], &map(&[("a", "1"), ("b", "1")]), &[], 100);
        assert!(ch.is_empty());
        let (b1, ch) = merge(&[], &map(&[("c", "1")]), std::slice::from_ref(&a1), 110);
        assert_eq!(ch, vec![Change::Set("a".into(), "1".into()), Change::Set("b".into(), "1".into())]);
        // A deletes b and changes a; B changed nothing (it applied the set).
        let (a2, ch) = merge(&a1, &map(&[("a", "2")]), std::slice::from_ref(&b1), 200);
        assert_eq!(ch, vec![Change::Set("c".into(), "1".into())]);
        let (_, ch) = merge(&b1, &map(&[("a", "1"), ("b", "1"), ("c", "1")]), std::slice::from_ref(&a2), 210);
        assert_eq!(ch, vec![Change::Set("a".into(), "2".into()), Change::Remove("b".into())]);
    }

    #[test]
    fn a_new_device_gives_way() {
        let shared = vec![Item { key: "theme".into(), value: "dark".into(), stamp: 50, gone: false }];
        let (kept, ch) = merge(&[], &map(&[("theme", "system"), ("only-here", "1")]), &[shared], 1000);
        assert_eq!(ch, vec![Change::Set("theme".into(), "dark".into())]);
        assert!(kept.iter().any(|i| i.key == "only-here"), "what only it has still travels");
    }

    #[test]
    fn newer_wins_and_files_round_trip() {
        let mine = vec![Item { key: "k".into(), value: "old".into(), stamp: 10, gone: false }];
        let theirs = vec![Item { key: "k".into(), value: "new".into(), stamp: 20, gone: false }];
        let (_, ch) = merge(&mine, &map(&[("k", "old")]), std::slice::from_ref(&theirs), 30);
        assert_eq!(ch, vec![Change::Set("k".into(), "new".into())]);
        // Changed here after theirs: mine wins, nothing to do here.
        let (kept, ch) = merge(&mine, &map(&[("k", "mine")]), &[theirs], 30);
        assert!(ch.is_empty());
        assert_eq!(kept[0].value, "mine");
        let scopes = BTreeMap::from([("bookmarks".to_string(), kept.clone())]);
        let (device, back) = read(&write("dev1", "laptop", 30, &scopes)).unwrap();
        assert_eq!((device.as_str(), &back["bookmarks"]), ("dev1", &kept));
        // Old tombstones go.
        let old = vec![Item { key: "x".into(), value: String::new(), stamp: 0, gone: true }];
        let (kept, _) = merge(&old, &map(&[]), &[], 100 * 86_400);
        assert!(kept.is_empty());
    }
}
