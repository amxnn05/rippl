//! rippl on the desktop: whether it is installed (its desktop entry, from
//! scripts/install.sh or a package), whether it is the default browser, and
//! making it so, the freedesktop way (`xdg-settings`, `xdg-mime`): links
//! from other apps then come to rippl, as a new tab of the window in front
//! (windows.rs, `launched_again`).

use std::process::Command;

/// The desktop entry's name.
pub(crate) const ENTRY: &str = "dev.rippl.Rippl.desktop";

/// What links from other apps open in, as rippl sees it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Default {
    /// rippl isn't installed (run from a build folder): it can't be.
    NotInstalled,
    Yes,
    No,
}

fn installed() -> bool {
    let mut dirs = vec![webkit6::glib::user_data_dir()];
    dirs.extend(webkit6::glib::system_data_dirs());
    dirs.iter().any(|d| d.join("applications").join(ENTRY).is_file())
}

/// Whether rippl is the default browser.
pub(crate) fn default_browser() -> Default {
    if !installed() {
        return Default::NotInstalled;
    }
    let out = Command::new("xdg-settings").args(["get", "default-web-browser"]).output();
    match out {
        Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == ENTRY => Default::Yes,
        _ => Default::No,
    }
}

/// Makes rippl the default browser: web links, and web pages opened as files.
pub(crate) fn make_default() -> Result<String, String> {
    if !installed() {
        return Err("rippl isn't installed yet: scripts/install.sh, or its package".into());
    }
    let ok = |c: &mut Command| c.status().map(|s| s.success()).unwrap_or(false);
    if !ok(Command::new("xdg-settings").args(["set", "default-web-browser", ENTRY])) {
        return Err("the desktop didn't take it (xdg-settings)".into());
    }
    for kind in ["x-scheme-handler/http", "x-scheme-handler/https", "text/html", "application/xhtml+xml"] {
        ok(Command::new("xdg-mime").args(["default", ENTRY, kind]));
    }
    Ok("rippl is your default browser now.".into())
}
