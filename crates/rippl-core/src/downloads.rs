//! Where a download goes: the Downloads folder, under the name the site
//! suggested, made safe and never overwriting what is already there; and
//! the list of finished downloads rippl://downloads shows (`Entry`, kept
//! as `downloads.log` in rippl's data folder, never from private tabs).

use std::path::{Path, PathBuf};

/// A file name a site suggested, cleaned: no path parts, no hidden-file
/// dot, no control characters, not empty, not absurdly long.
pub fn clean_name(suggested: &str) -> String {
    let last = suggested.rsplit(['/', '\\']).next().unwrap_or("");
    let mut name: String = last.chars().filter(|c| !c.is_control()).collect();
    name = name.trim().trim_start_matches('.').to_string();
    if name.is_empty() {
        name = "download".into();
    }
    // 255 bytes is the usual file-name limit; keep the extension.
    while name.len() > 200 {
        let cut = name.char_indices().nth(name.chars().count() / 2).map_or(0, |(i, _)| i);
        let (stem, ext) = split_ext(&name);
        let stem: String = stem.chars().take(stem.chars().count().saturating_sub(10).max(1)).collect();
        name = format!("{stem}{ext}");
        if cut == 0 {
            break;
        }
    }
    name
}

/// `archive.tar.gz` → (`archive`, `.tar.gz`); `photo.jpg` → (`photo`, `.jpg`).
fn split_ext(name: &str) -> (&str, &str) {
    for double in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"] {
        if name.len() > double.len() && name.to_lowercase().ends_with(double) {
            let at = name.len() - double.len();
            return (&name[..at], &name[at..]);
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// A path in `dir` for `suggested` that `exists` says is free:
/// `report.pdf`, then `report (1).pdf`, `report (2).pdf`, …
pub fn destination(dir: &Path, suggested: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let name = clean_name(suggested);
    let first = dir.join(&name);
    if !exists(&first) {
        return first;
    }
    let (stem, ext) = split_ext(&name);
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !exists(p))
        .expect("an unused name exists")
}

/// The most downloads the list keeps; older ones fall off.
pub const KEPT: usize = 500;

/// One finished download, as the list keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// When it ended, seconds since 1970.
    pub when: i64,
    /// `done`, or `failed` (why is in `note`).
    pub done: bool,
    pub name: String,
    /// Where the file went (done), else empty.
    pub path: String,
    /// Where it came from.
    pub url: String,
    /// Why it failed, or "video"/"audio" for yt-dlp's.
    pub note: String,
}

/// A field made safe for a tab-separated line.
fn field(s: &str) -> String {
    s.chars().map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

impl Entry {
    /// One tab-separated line: when, done|failed, name, path, url, note.
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.when,
            if self.done { "done" } else { "failed" },
            field(&self.name),
            field(&self.path),
            field(&self.url),
            field(&self.note)
        )
    }

    pub fn parse(line: &str) -> Option<Entry> {
        let mut f = line.split('\t');
        let when = f.next()?.parse().ok()?;
        let done = match f.next()? {
            "done" => true,
            "failed" => false,
            _ => return None,
        };
        let name = f.next()?.to_string();
        let path = f.next().unwrap_or("").to_string();
        let url = f.next().unwrap_or("").to_string();
        let note = f.next().unwrap_or("").to_string();
        Some(Entry { when, done, name, path, url, note })
    }
}

/// The list from its file: oldest first, unreadable lines skipped.
pub fn read_log(text: &str) -> Vec<Entry> {
    text.lines().filter_map(Entry::parse).collect()
}

/// The list as its file: the newest `KEPT`, oldest first.
pub fn write_log(entries: &[Entry]) -> String {
    let from = entries.len().saturating_sub(KEPT);
    entries[from..].iter().map(|e| e.line() + "\n").collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_survives_its_file() {
        let e = Entry { when: 1_700_000_000, done: true, name: "a\tb.pdf".into(), path: "/home/x/Downloads/a b.pdf".into(), url: "https://x.org/a".into(), note: String::new() };
        let f = Entry { when: 1_700_000_100, done: false, name: "c.zip".into(), path: String::new(), url: "https://y.org/c".into(), note: "Cancelled".into() };
        let back = read_log(&(write_log(&[e.clone(), f.clone()]) + "junk\n"));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "a b.pdf");
        assert_eq!(back[1], f);
        let many: Vec<Entry> = (0..KEPT as i64 + 10).map(|i| Entry { when: i, ..f.clone() }).collect();
        let kept = read_log(&write_log(&many));
        assert_eq!(kept.len(), KEPT);
        assert_eq!(kept[0].when, 10);
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name(".bashrc"), "bashrc");
        assert_eq!(clean_name("a\u{0}b.txt"), "ab.txt");
        assert_eq!(clean_name(""), "download");
        assert_eq!(clean_name("C:\\Users\\x\\file.exe"), "file.exe");
        let long = format!("{}.pdf", "x".repeat(400));
        let cleaned = clean_name(&long);
        assert!(cleaned.len() <= 200 && cleaned.ends_with(".pdf"), "{cleaned}");
    }

    #[test]
    fn never_overwrites() {
        let dir = Path::new("/dl");
        let taken = ["/dl/report.pdf", "/dl/report (1).pdf", "/dl/a.tar.gz"];
        let exists = |p: &Path| taken.iter().any(|t| Path::new(t) == p);
        assert_eq!(destination(dir, "report.pdf", exists), Path::new("/dl/report (2).pdf"));
        assert_eq!(destination(dir, "a.tar.gz", exists), Path::new("/dl/a (1).tar.gz"));
        assert_eq!(destination(dir, "new.txt", exists), Path::new("/dl/new.txt"));
    }
}
