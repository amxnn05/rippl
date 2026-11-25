//! Passwords: the rules, with no keyring and no page in them.
//!
//! An account belongs to an **origin**: scheme, host and port, exactly
//! (`https://accounts.example.com`). It is only ever offered on that same
//! origin, so `https://example.com` and `https://login.example.com` don't
//! share accounts, and an `http://` page never sees an `https://` account.
//!
//! Passwords are only saved on secure pages: `https`, or `http` on this
//! machine (`localhost`, `127.0.0.1`, `[::1]`) for development. A sign-in
//! sent over plain http was readable by anyone on the network, and
//! remembering it would only encourage using it.

/// `https://Example.com:443/login?x` → `https://example.com`. `None` for
/// anything that isn't http(s).
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let hostport = authority.rsplit('@').next()?.to_ascii_lowercase();
    if hostport.is_empty() {
        return None;
    }
    // The default port says nothing: drop it so both spellings match.
    let hostport = match (scheme.as_str(), hostport.rsplit_once(':')) {
        ("https", Some((h, "443"))) | ("http", Some((h, "80"))) => h.to_string(),
        _ => hostport,
    };
    Some(format!("{scheme}://{hostport}"))
}

/// The host part of an origin, for showing it.
pub fn host_of_origin(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, h)| h)
}

/// Whether a sign-in on `origin` may be saved.
pub fn may_save(origin: &str) -> bool {
    if origin.starts_with("https://") {
        return true;
    }
    let host = host_of_origin(origin);
    let host = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
    origin.starts_with("http://") && matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// What a page's sign-in form held when it was sent, as rippl's form
/// script reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub username: String,
    pub password: String,
    /// A sign-up or password change (`autocomplete=new-password`, or more
    /// than one password box filled).
    pub new_password: bool,
}

impl Capture {
    /// Checked fields, or `None` if this isn't something worth saving.
    pub fn new(username: &str, password: &str, new_password: bool) -> Option<Capture> {
        let username = username.trim();
        if password.is_empty() || password.len() > 1024 || username.len() > 512 {
            return None;
        }
        // A one-time code typed into a password box is not a password.
        if password.len() <= 8 && password.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(Capture { username: username.to_string(), password: password.to_string(), new_password })
    }
}

/// What to ask after a sign-in that worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Nothing: already saved exactly so, or this site is on the never list.
    Nothing,
    /// "Save password for <username>?"
    Save,
    /// "Update the password for <username>?"
    Update,
}

/// `saved`: the password kept for this origin and username, if any.
pub fn what_to_ask(capture: &Capture, saved: Option<&str>, never: bool) -> Ask {
    if never {
        return Ask::Nothing;
    }
    match saved {
        Some(p) if p == capture.password => Ask::Nothing,
        Some(_) => Ask::Update,
        None => Ask::Save,
    }
}

/// The label an account has in the keyring (what Seahorse and other
/// keyring apps show).
pub fn label(origin: &str, username: &str) -> String {
    let host = host_of_origin(origin);
    if username.is_empty() { format!("rippl: {host}") } else { format!("rippl: {username} on {host}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        assert_eq!(origin_of("https://Example.com/login?next=/").as_deref(), Some("https://example.com"));
        assert_eq!(origin_of("https://example.com:443/").as_deref(), Some("https://example.com"));
        assert_eq!(origin_of("https://example.com:8443/").as_deref(), Some("https://example.com:8443"));
        assert_eq!(origin_of("https://user:pw@example.com/").as_deref(), Some("https://example.com"));
        assert_eq!(origin_of("http://localhost:3000/x").as_deref(), Some("http://localhost:3000"));
        assert_eq!(origin_of("file:///etc/passwd"), None);
        assert_eq!(origin_of("rippl://settings"), None);
        assert_ne!(origin_of("https://example.com"), origin_of("https://login.example.com"));
        assert_ne!(origin_of("http://example.com"), origin_of("https://example.com"));
    }

    #[test]
    fn secure_pages_only() {
        assert!(may_save("https://example.com"));
        assert!(may_save("http://localhost:8765"));
        assert!(may_save("http://127.0.0.1"));
        assert!(!may_save("http://example.com"));
        assert!(!may_save("http://localhost.evil.com"));
    }

    #[test]
    fn captures() {
        assert!(Capture::new("bob", "", false).is_none());
        assert!(Capture::new("bob", "123456", false).is_none(), "a one-time code");
        assert!(Capture::new("bob", &"x".repeat(2000), false).is_none());
        let c = Capture::new("  bob@mail.com ", "hunter22", false).unwrap();
        assert_eq!(c.username, "bob@mail.com");
    }

    #[test]
    fn asking() {
        let c = Capture::new("bob", "hunter22", false).unwrap();
        assert_eq!(what_to_ask(&c, None, false), Ask::Save);
        assert_eq!(what_to_ask(&c, Some("hunter22"), false), Ask::Nothing);
        assert_eq!(what_to_ask(&c, Some("old"), false), Ask::Update);
        assert_eq!(what_to_ask(&c, None, true), Ask::Nothing);
    }

    #[test]
    fn labels() {
        assert_eq!(label("https://example.com", "bob"), "rippl: bob on example.com");
        assert_eq!(label("https://example.com", ""), "rippl: example.com");
    }
}
