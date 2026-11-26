//! More brought over from another browser on this machine (the
//! ): history, the tabs it has open, and Chromium's saved
//! passwords. Only reading, from copies: nothing in the other browser
//! changes, and it may be running.
//!
//! - **History**: Chromium's `History` (`urls`), Firefox's `places.sqlite`
//!   (`moz_places`).
//! - **Open tabs**: Chromium's newest `Sessions/Session_*` (its SNSS
//!   format: a log of commands, replayed here); Firefox's
//!   `sessionstore-backups/recovery.jsonlz4` (or `sessionstore.jsonlz4`),
//!   LZ4-compressed JSON.
//! - **Passwords** (Chromium family): `Login Data`, each password AES-128
//!   CBC encrypted under a key from "<Browser> Safe Storage" in the system
//!   keyring (`v11`), or the fixed "peanuts" one (`v10`) when it had no
//!   keyring. The app asks the keyring; this decrypts.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::import::{Kind, Source};

/// A page from another browser's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub visits: i64,
    pub typed: i64,
    /// Unix seconds.
    pub last: i64,
}

/// A tab open in another browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTab {
    pub url: String,
    pub title: String,
    pub pinned: bool,
}

/// A saved sign-in, still encrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub origin: String,
    pub username: String,
    pub sealed: Vec<u8>,
}

/// Seconds between 1601 (Windows' and Chromium's epoch) and 1970.
const CHROMIUM_EPOCH: i64 = 11_644_473_600;

fn profile_dir(source: &Source) -> PathBuf {
    source.path.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn kept(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Copies `db` (and its journal) into `scratch` and runs `read` on the copy.
fn on_copy<T>(db: &Path, scratch: &Path, read: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
    let copy = scratch.join("import-copy.sqlite");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", copy.display()));
    }
    std::fs::copy(db, &copy).map_err(|e| e.to_string())?;
    let wal = PathBuf::from(format!("{}-wal", db.display()));
    if wal.is_file() {
        std::fs::copy(&wal, format!("{}-wal", copy.display())).map_err(|e| e.to_string())?;
    }
    let result = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())
        .and_then(|c| read(&c));
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", copy.display()));
    }
    result
}

// ---- history -------------------------------------------------------------

/// The other browser's history, newest first, at most `most` pages.
pub fn history(source: &Source, scratch: &Path, most: usize) -> Result<Vec<Visit>, String> {
    let dir = profile_dir(source);
    match source.kind {
        Kind::Chromium => on_copy(&dir.join("History"), scratch, |db| {
            let mut stmt = db
                .prepare(
                    "SELECT url, COALESCE(title, ''), visit_count, typed_count, last_visit_time FROM urls
                     WHERE hidden = 0 ORDER BY last_visit_time DESC LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([most as i64], |r| {
                    let micros: i64 = r.get(4)?;
                    Ok(Visit {
                        url: r.get(0)?,
                        title: r.get(1)?,
                        visits: r.get(2)?,
                        typed: r.get(3)?,
                        last: micros / 1_000_000 - CHROMIUM_EPOCH,
                    })
                })
                .map_err(|e| e.to_string())?;
            Ok(rows.flatten().filter(|v| kept(&v.url)).collect())
        }),
        Kind::Firefox => on_copy(&source.path, scratch, |db| {
            let mut stmt = db
                .prepare(
                    "SELECT url, COALESCE(title, ''), visit_count, typed, COALESCE(last_visit_date, 0) FROM moz_places
                     WHERE hidden = 0 AND visit_count > 0 ORDER BY last_visit_date DESC LIMIT ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([most as i64], |r| {
                    let micros: i64 = r.get(4)?;
                    Ok(Visit { url: r.get(0)?, title: r.get(1)?, visits: r.get(2)?, typed: r.get(3)?, last: micros / 1_000_000 })
                })
                .map_err(|e| e.to_string())?;
            Ok(rows.flatten().filter(|v| kept(&v.url)).collect())
        }),
    }
}

// ---- open tabs -------------------------------------------------------------

/// The tabs the other browser has open (or had, when it last closed).
pub fn open_tabs(source: &Source) -> Result<Vec<OpenTab>, String> {
    let dir = profile_dir(source);
    match source.kind {
        Kind::Chromium => {
            let sessions = dir.join("Sessions");
            let newest = std::fs::read_dir(&sessions)
                .map_err(|_| "no saved session in that profile".to_string())?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("Session_")))
                .max()
                .ok_or("no saved session in that profile")?;
            snss(&std::fs::read(newest).map_err(|e| e.to_string())?)
        }
        Kind::Firefox => {
            let file = [dir.join("sessionstore-backups/recovery.jsonlz4"), dir.join("sessionstore.jsonlz4")]
                .into_iter()
                .find(|p| p.is_file())
                .ok_or("no saved session in that profile")?;
            let json = mozlz4(&std::fs::read(file).map_err(|e| e.to_string())?).ok_or("its session file can't be read")?;
            firefox_session(&json)
        }
    }
}

/// An LZ4 block, decompressed (Firefox's session files are one).
pub fn lz4_block(src: &[u8], size_hint: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(size_hint);
    let mut i = 0;
    let more = |i: &mut usize, mut n: usize| -> Option<usize> {
        loop {
            let b = *src.get(*i)?;
            *i += 1;
            n += b as usize;
            if b != 255 {
                return Some(n);
            }
        }
    };
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut literals = (token >> 4) as usize;
        if literals == 15 {
            literals = more(&mut i, literals)?;
        }
        out.extend_from_slice(src.get(i..i + literals)?);
        i += literals;
        if i >= src.len() {
            break;
        }
        let offset = u16::from_le_bytes([*src.get(i)?, *src.get(i + 1)?]) as usize;
        i += 2;
        if offset == 0 || offset > out.len() {
            return None;
        }
        let mut length = (token & 15) as usize;
        if length == 15 {
            length = more(&mut i, length)?;
        }
        length += 4;
        let start = out.len() - offset;
        for k in 0..length {
            let b = out[start + k];
            out.push(b);
        }
    }
    Some(out)
}

/// Firefox's `mozLz40` file: a magic, the size, and an LZ4 block.
pub fn mozlz4(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 12 || &bytes[..8] != b"mozLz40\0" {
        return None;
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    String::from_utf8(lz4_block(&bytes[12..], size)?).ok()
}

/// The open tabs in Firefox's session JSON: each tab's current entry.
pub fn firefox_session(json: &str) -> Result<Vec<OpenTab>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for window in v.get("windows").and_then(|w| w.as_array()).into_iter().flatten() {
        for tab in window.get("tabs").and_then(|t| t.as_array()).into_iter().flatten() {
            let entries = tab.get("entries").and_then(|e| e.as_array());
            let index = tab.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
            let entry = entries.and_then(|e| e.get(index.saturating_sub(1)).or_else(|| e.last()));
            let Some(url) = entry.and_then(|e| e.get("url")).and_then(|u| u.as_str()).filter(|u| kept(u)) else { continue };
            out.push(OpenTab {
                url: url.to_string(),
                title: entry.and_then(|e| e.get("title")).and_then(|t| t.as_str()).unwrap_or("").to_string(),
                pinned: tab.get("pinned").and_then(|p| p.as_bool()).unwrap_or(false),
            });
        }
    }
    Ok(out)
}

/// Little-endian i32 at `at`.
fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Chromium's session file (SNSS): commands replayed into the tabs open at
/// its end, window by window, in their order.
pub fn snss(bytes: &[u8]) -> Result<Vec<OpenTab>, String> {
    if bytes.len() < 8 || &bytes[..4] != b"SNSS" {
        return Err("not a Chromium session file".into());
    }
    let mut navigations: HashMap<i32, BTreeMap<i32, (String, String)>> = HashMap::new();
    let mut selected: HashMap<i32, i32> = HashMap::new();
    let mut window_of: HashMap<i32, i32> = HashMap::new();
    let mut place: HashMap<i32, i32> = HashMap::new();
    let mut pinned: HashSet<i32> = HashSet::new();
    let mut closed_tabs: HashSet<i32> = HashSet::new();
    let mut closed_windows: HashSet<i32> = HashSet::new();
    let mut order: Vec<i32> = Vec::new();
    let mut i = 8;
    while i + 2 <= bytes.len() {
        let size = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2;
        if size == 0 || i + size > bytes.len() {
            break;
        }
        let id = bytes[i];
        let p = &bytes[i + 1..i + size];
        i += size;
        match id {
            // SetTabWindow: window, tab.
            0 => {
                if let (Some(w), Some(t)) = (i32_at(p, 0), i32_at(p, 4)) {
                    window_of.insert(t, w);
                }
            }
            // SetTabIndexInWindow: tab, index.
            2 => {
                if let (Some(t), Some(x)) = (i32_at(p, 0), i32_at(p, 4)) {
                    place.insert(t, x);
                }
            }
            // UpdateTabNavigation: a pickle (its size, then tab, index, url,
            // title as UTF-16).
            6 => {
                let (Some(t), Some(x), Some(len)) = (i32_at(p, 4), i32_at(p, 8), i32_at(p, 12)) else { continue };
                let len = len.max(0) as usize;
                let Some(url) = p.get(16..16 + len) else { continue };
                let after = 16 + len.div_ceil(4) * 4;
                let chars = i32_at(p, after).unwrap_or(0).max(0) as usize;
                let title: Vec<u16> = p
                    .get(after + 4..after + 4 + chars * 2)
                    .unwrap_or(&[])
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                if !order.contains(&t) {
                    order.push(t);
                }
                navigations
                    .entry(t)
                    .or_default()
                    .insert(x, (String::from_utf8_lossy(url).into_owned(), String::from_utf16_lossy(&title)));
            }
            // SetSelectedNavigationIndex: tab, index.
            7 => {
                if let (Some(t), Some(x)) = (i32_at(p, 0), i32_at(p, 4)) {
                    selected.insert(t, x);
                }
            }
            // SetPinnedState: tab, bool.
            12 => {
                if let Some(t) = i32_at(p, 0) {
                    if p.get(4).copied().unwrap_or(0) != 0 { pinned.insert(t) } else { pinned.remove(&t) };
                }
            }
            16 => closed_tabs.extend(i32_at(p, 0)),
            17 => closed_windows.extend(i32_at(p, 0)),
            _ => {}
        }
    }
    let mut tabs: Vec<(i32, i32, i32)> = order
        .iter()
        .filter(|t| !closed_tabs.contains(t))
        .filter(|t| window_of.get(t).is_none_or(|w| !closed_windows.contains(w)))
        .map(|t| (window_of.get(t).copied().unwrap_or(0), place.get(t).copied().unwrap_or(i32::MAX), *t))
        .collect();
    tabs.sort();
    Ok(tabs
        .into_iter()
        .filter_map(|(_, _, t)| {
            let navs = navigations.get(&t)?;
            let (url, title) = selected.get(&t).and_then(|x| navs.get(x)).or_else(|| navs.values().last())?;
            kept(url).then(|| OpenTab { url: url.clone(), title: title.clone(), pinned: pinned.contains(&t) })
        })
        .collect())
}

// ---- passwords --------------------------------------------------------------

/// The keyring name Chromium-family browsers keep their key under, as the
/// `application` attribute of "<Browser> Safe Storage".
pub fn keyring_application(browser: &str) -> Option<&'static str> {
    Some(match browser {
        "Chrome" => "chrome",
        "Chromium" => "chromium",
        "Brave" => "brave",
        "Edge" => "microsoft-edge",
        "Vivaldi" => "vivaldi",
        _ => return None,
    })
}

/// The saved sign-ins of a Chromium profile, still sealed.
pub fn logins(source: &Source, scratch: &Path) -> Result<Vec<Login>, String> {
    if source.kind != Kind::Chromium {
        return Err("only Chromium-family passwords can be brought over".into());
    }
    on_copy(&profile_dir(source).join("Login Data"), scratch, |db| {
        let mut stmt = db
            .prepare("SELECT origin_url, username_value, password_value FROM logins WHERE blacklisted_by_user = 0")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok(Login { origin: r.get(0)?, username: r.get(1)?, sealed: r.get(2)? }))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().filter(|l| kept(&l.origin) && !l.sealed.is_empty()).collect())
    })
}

fn key(secret: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(secret, b"saltysalt", 1, &mut key);
    key
}

/// A Chromium password opened: `v10` with the fixed key, `v11` with the
/// keyring's `secret`. `None` if it can't be.
pub fn unseal(sealed: &[u8], secret: Option<&[u8]>) -> Option<String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    let key = match sealed.get(..3)? {
        b"v10" => key(b"peanuts"),
        b"v11" => key(secret?),
        _ => return None,
    };
    let mut data = sealed[3..].to_vec();
    let plain = cbc::Decryptor::<aes::Aes128>::new(&key.into(), &[b' '; 16].into())
        .decrypt_padded_mut::<Pkcs7>(&mut data)
        .ok()?;
    String::from_utf8(plain.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz4_and_mozlz4() {
        // "abc" then a match of 6 from 3 back: "abcabcabc".
        assert_eq!(lz4_block(&[0x32, b'a', b'b', b'c', 3, 0], 9).unwrap(), b"abcabcabc");
        assert_eq!(lz4_block(&[0x50, b'h', b'e', b'l', b'l', b'o'], 5).unwrap(), b"hello");
        assert!(lz4_block(&[0x10, b'a', 9, 0], 4).is_none(), "an offset past the start");
        let json = br#"{"windows":[{"tabs":[{"entries":[{"url":"https://a.com/","title":"A"},{"url":"https://b.com/","title":"B"}],"index":1,"pinned":true},{"entries":[{"url":"about:blank"}],"index":1}]}]}"#;
        let mut file = b"mozLz40\0".to_vec();
        file.extend((json.len() as u32).to_le_bytes());
        // All literals: 15 then the rest of the length in 255s.
        let mut block = vec![0xF0];
        let mut n = json.len() - 15;
        while n >= 255 {
            block.push(255);
            n -= 255;
        }
        block.push(n as u8);
        block.extend_from_slice(json);
        file.extend(block);
        let tabs = firefox_session(&mozlz4(&file).unwrap()).unwrap();
        assert_eq!(tabs, vec![OpenTab { url: "https://a.com/".into(), title: "A".into(), pinned: true }]);
    }

    fn command(id: u8, payload: &[u8]) -> Vec<u8> {
        let mut c = ((payload.len() + 1) as u16).to_le_bytes().to_vec();
        c.push(id);
        c.extend_from_slice(payload);
        c
    }

    fn pair(a: i32, b: i32) -> Vec<u8> {
        [a.to_le_bytes(), b.to_le_bytes()].concat()
    }

    fn navigation(tab: i32, index: i32, url: &str, title: &str) -> Vec<u8> {
        let mut body = pair(tab, index);
        body.extend((url.len() as i32).to_le_bytes());
        body.extend(url.as_bytes());
        body.resize(body.len().div_ceil(4) * 4, 0);
        let t: Vec<u16> = title.encode_utf16().collect();
        body.extend((t.len() as i32).to_le_bytes());
        for u in &t {
            body.extend(u.to_le_bytes());
        }
        body.resize(body.len().div_ceil(4) * 4, 0);
        let mut pickle = (body.len() as u32).to_le_bytes().to_vec();
        pickle.extend(body);
        command(6, &pickle)
    }

    #[test]
    fn chromium_session_replayed() {
        let mut f = b"SNSS".to_vec();
        f.extend(3i32.to_le_bytes());
        for c in [
            command(0, &pair(1, 10)),
            command(0, &pair(1, 11)),
            command(0, &pair(2, 12)),
            command(2, &pair(10, 1)),
            command(2, &pair(11, 0)),
            navigation(10, 0, "https://first.com/", "First"),
            navigation(10, 1, "https://second.com/", "Second"),
            command(7, &pair(10, 0)),
            navigation(11, 0, "https://pinned.org/", "Pinned"),
            command(12, &[11i32.to_le_bytes().to_vec(), vec![1, 0, 0, 0]].concat()),
            navigation(12, 0, "https://gone.net/", "Gone"),
            command(17, &2i32.to_le_bytes()),
            navigation(13, 0, "chrome://newtab/", "New Tab"),
        ] {
            f.extend(c);
        }
        let tabs = snss(&f).unwrap();
        assert_eq!(
            tabs,
            vec![
                OpenTab { url: "https://pinned.org/".into(), title: "Pinned".into(), pinned: true },
                OpenTab { url: "https://first.com/".into(), title: "First".into(), pinned: false },
            ]
        );
    }

    #[test]
    fn chromium_passwords_unsealed() {
        use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
        let seal = |prefix: &[u8], secret: &[u8], text: &str| {
            let k = key(secret);
            let mut buf = text.as_bytes().to_vec();
            let n = buf.len();
            buf.resize(n + 16, 0);
            let sealed = cbc::Encryptor::<aes::Aes128>::new(&k.into(), &[b' '; 16].into())
                .encrypt_padded_mut::<Pkcs7>(&mut buf, n)
                .unwrap()
                .to_vec();
            [prefix.to_vec(), sealed].concat()
        };
        assert_eq!(unseal(&seal(b"v10", b"peanuts", "hunter2"), None).as_deref(), Some("hunter2"));
        let v11 = seal(b"v11", b"from-the-keyring", "s3cret pass");
        assert_eq!(unseal(&v11, Some(b"from-the-keyring")).as_deref(), Some("s3cret pass"));
        assert_eq!(unseal(&v11, Some(b"wrong")), None);
        assert_eq!(unseal(&v11, None), None);
        assert_eq!(unseal(b"plain", None), None);
    }
}
