//! What was typed in the one field: a place to go, or words to search for.
//!
//! . Something only becomes an address
//! when it clearly is one: "hello world" and "todo" are searches, while
//! "example.com", "localhost:3000" and "192.168.1.1" are places.

/// Schemes the window shows itself.
const OURS: &[&str] = &["http", "https", "file", "about", "data", "rippl"];

/// Where Return in the field goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// A URL to load.
    Page(String),
    /// Nothing usable was typed.
    Nothing,
}

/// The search engine, as a template with `{}` where the words go.
///
/// DuckDuckGo by default: no account, no search history kept, no
/// profile, and the query never leaves rippl until Return is pressed
/// (rippl sends no suggestions while you type). See docs/privacy.md for
/// why, and for the others.
pub const DEFAULT_SEARCH: &str = "https://duckduckgo.com/?q={}";

/// Search engines that don't track you, by the name used in the config.
pub const ENGINES: &[(&str, &str)] = &[
    ("duckduckgo", DEFAULT_SEARCH),
    ("brave", "https://search.brave.com/search?q={}"),
    ("startpage", "https://www.startpage.com/do/search?q={}"),
    ("mojeek", "https://www.mojeek.com/search?q={}"),
    ("qwant", "https://www.qwant.com/?q={}"),
];

/// The template for a config value: a name from `ENGINES`, or an https
/// address of your own with `{}` where the words go (a SearXNG, say).
pub fn engine(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some((_, template)) = ENGINES.iter().find(|(name, _)| name.eq_ignore_ascii_case(value)) {
        return Some(template.to_string());
    }
    (value.starts_with("https://") && value.contains("{}") && !value.contains(char::is_whitespace))
        .then(|| value.to_string())
}

/// What Return does with `typed`: open it as an address if it is one,
/// otherwise search for it with `search` (a template containing `{}`).
pub fn resolve(typed: &str, search: &str) -> Destination {
    let text = typed.trim();
    if text.is_empty() {
        return Destination::Nothing;
    }
    if let Some(url) = url_from(text) {
        return Destination::Page(url);
    }
    if let Some(path) = local_file(text) {
        return Destination::Page(path);
    }
    Destination::Page(search.replacen("{}", &encode_query(text), 1))
}

/// `typed` as a URL if it looks like one; `None` means "search for it".
pub fn url_from(typed: &str) -> Option<String> {
    let text = typed.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }

    // Typed with a scheme: taken at its word, if it is one we show.
    if let Some(at) = text.find("://") {
        let scheme = text[..at].to_ascii_lowercase();
        return OURS.contains(&scheme.as_str()).then(|| text.to_string());
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("about:") || lower.starts_with("data:") {
        return Some(text.to_string());
    }

    // Anything else has to look like a host before it gets a scheme.
    let head_end = text.find(['/', '?', '#']).unwrap_or(text.len());
    let head = &text[..head_end];
    if head.contains('@') {
        return None; // an email address
    }
    if head.starts_with("[::1]") {
        return Some(format!("http://{text}"));
    }
    let host = head.split(':').next().unwrap_or(head).to_ascii_lowercase();
    // A port, when there is one, has to be a number: "note:buy" is not a place.
    if let Some(port) = head.split(':').nth(1)
        && (port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    if !looks_like_host(&host) {
        return None;
    }

    // Local servers almost never have a certificate: https there just fails.
    let local = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "127.0.0.1"
        || host == "0.0.0.0"
        || private_range(&host);
    let scheme = if local { "http" } else { "https" };
    // Dev servers print 0.0.0.0; WebKit refuses to go there, so use localhost.
    let rest = if host == "0.0.0.0" { text.replacen("0.0.0.0", "localhost", 1) } else { text.to_string() };
    Some(format!("{scheme}://{rest}"))
}

/// An absolute or home-relative path to a file that exists.
fn local_file(text: &str) -> Option<String> {
    let path = if let Some(rest) = text.strip_prefix("~/") {
        format!("{}/{rest}", std::env::var("HOME").ok()?)
    } else if text.starts_with('/') && !text.starts_with("//") {
        text.to_string()
    } else {
        return None;
    };
    std::fs::metadata(&path).ok()?.is_file().then(|| format!("file://{path}"))
}

fn looks_like_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() == 4 && labels.iter().all(|l| l.parse::<u8>().is_ok()) {
        return true;
    }
    if labels.len() < 2 {
        return false;
    }
    let well_formed = labels.iter().all(|l| {
        !l.is_empty()
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_alphanumeric() || c == '-')
    });
    // The last label carries the weight: letters make a domain, digits a
    // version number ("1.2" is a search).
    let tld = labels[labels.len() - 1];
    well_formed && tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic)
}

/// 10/8, 172.16/12 and 192.168/16, written as exactly four numbers.
fn private_range(host: &str) -> bool {
    let octets: Vec<u8> = host.split('.').filter_map(|p| p.parse().ok()).collect();
    if octets.len() != 4 || host.split('.').count() != 4 {
        return false;
    }
    octets[0] == 10
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
}

/// Percent-encodes words for a query string (spaces become `+`).
pub fn encode_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// What a tab says before the page has a title: host and path, no `www.`.
pub fn pretty(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return url.to_string() };
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    rest.strip_suffix('/').unwrap_or(rest).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(t: &str) -> String {
        match resolve(t, DEFAULT_SEARCH) {
            Destination::Page(u) => u,
            Destination::Nothing => String::new(),
        }
    }

    #[test]
    fn domains_get_https() {
        assert_eq!(go("example.com"), "https://example.com");
        assert_eq!(go("  news.ycombinator.com/item?id=1 "), "https://news.ycombinator.com/item?id=1");
    }

    #[test]
    fn local_hosts_get_http() {
        assert_eq!(go("localhost:3000"), "http://localhost:3000");
        assert_eq!(go("192.168.1.1"), "http://192.168.1.1");
        assert_eq!(go("printer.local"), "http://printer.local");
        assert_eq!(go("0.0.0.0:8080/x"), "http://localhost:8080/x");
        assert_eq!(go("[::1]:3000"), "http://[::1]:3000");
    }

    #[test]
    fn public_ip_is_https_and_10_io_is_a_site() {
        assert_eq!(go("8.8.8.8"), "https://8.8.8.8");
        assert_eq!(go("10.io"), "https://10.io");
    }

    #[test]
    fn words_are_searches() {
        assert_eq!(go("hello world"), "https://duckduckgo.com/?q=hello+world");
        assert_eq!(go("todo"), "https://duckduckgo.com/?q=todo");
        assert_eq!(go("1.2"), "https://duckduckgo.com/?q=1.2");
        assert_eq!(go("me@mail.com"), "https://duckduckgo.com/?q=me%40mail.com");
        assert_eq!(go("note:buy"), "https://duckduckgo.com/?q=note%3Abuy");
        assert_eq!(go("c++"), "https://duckduckgo.com/?q=c%2B%2B");
    }

    #[test]
    fn schemes_taken_at_their_word() {
        assert_eq!(go("http://x"), "http://x");
        assert_eq!(go("about:blank"), "about:blank");
        assert_eq!(url_from("javascript://alert(1)"), None);
    }

    #[test]
    fn engines() {
        assert_eq!(engine("Brave").as_deref(), Some("https://search.brave.com/search?q={}"));
        assert_eq!(engine("https://searx.me/search?q={}").as_deref(), Some("https://searx.me/search?q={}"));
        assert_eq!(engine("http://plain.example/?q={}"), None, "https only");
        assert_eq!(engine("https://no-placeholder.example/"), None);
        assert_eq!(engine("google"), None);
    }

    #[test]
    fn empty_is_nothing() {
        assert_eq!(resolve("   ", DEFAULT_SEARCH), Destination::Nothing);
    }

    #[test]
    fn pretty_titles() {
        assert_eq!(pretty("https://www.example.com/"), "example.com");
        assert_eq!(pretty("https://example.com/a/b?q=1"), "example.com/a/b");
    }
}
