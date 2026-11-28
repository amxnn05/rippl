//! Where rippl keeps its files on each system. On Linux these are the XDG
//! folders, as GLib finds them (so the GTK build and this agree); on macOS
//! `~/Library/Application Support/rippl` and `~/Library/Caches/rippl`; on
//! Windows `%APPDATA%\rippl` and `%LOCALAPPDATA%\rippl`.

use std::path::PathBuf;

/// The home folder.
pub fn home() -> PathBuf {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() && PathBuf::from(&v).is_absolute() => PathBuf::from(v),
        _ => home().join(fallback),
    }
}

/// rippl's settings folder (the `config` file, the Shield allow list).
pub fn config() -> PathBuf {
    #[cfg(target_os = "macos")]
    return home().join("Library/Application Support/rippl");
    #[cfg(windows)]
    return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home().join("AppData/Roaming")).join("rippl");
    #[cfg(all(unix, not(target_os = "macos")))]
    return xdg("XDG_CONFIG_HOME", ".config").join("rippl");
}

/// rippl's data folder (places.sqlite, the session, the downloads list).
pub fn data() -> PathBuf {
    #[cfg(target_os = "macos")]
    return home().join("Library/Application Support/rippl");
    #[cfg(windows)]
    return std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home().join("AppData/Roaming")).join("rippl");
    #[cfg(all(unix, not(target_os = "macos")))]
    return xdg("XDG_DATA_HOME", ".local/share").join("rippl");
}

/// rippl's cache folder (Shield's lists, the web cache).
pub fn cache() -> PathBuf {
    #[cfg(target_os = "macos")]
    return home().join("Library/Caches/rippl");
    #[cfg(windows)]
    return std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| home().join("AppData/Local")).join("rippl");
    #[cfg(all(unix, not(target_os = "macos")))]
    return xdg("XDG_CACHE_HOME", ".cache").join("rippl");
}

/// The Downloads folder (`RIPPL_DOWNLOADS` moves it, for tests).
pub fn downloads() -> PathBuf {
    if let Some(d) = std::env::var_os("RIPPL_DOWNLOADS").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Ok(text) = std::fs::read_to_string(xdg("XDG_CONFIG_HOME", ".config").join("user-dirs.dirs")) {
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("XDG_DOWNLOAD_DIR=") {
                let v = v.trim_matches('"').replace("$HOME", &home().to_string_lossy());
                return PathBuf::from(v);
            }
        }
    }
    home().join("Downloads")
}
