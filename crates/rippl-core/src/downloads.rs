//! Where a download goes: the Downloads folder, under the name the site
//! suggested, made safe and never overwriting what is already there.

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

#[cfg(test)]
mod tests {
    use super::*;

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
