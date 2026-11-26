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

/// Site keywords: "yt cats" searches YouTube for cats, whatever the
/// default engine is. `%s` is where the words go.
pub const KEYWORDS: &[(&str, &str)] = &[
    ("w", "https://en.wikipedia.org/w/index.php?search=%s"),
    ("yt", "https://www.youtube.com/results?search_query=%s"),
    ("gh", "https://github.com/search?q=%s"),
    ("mdn", "https://developer.mozilla.org/en-US/search?q=%s"),
    ("osm", "https://www.openstreetmap.org/search?query=%s"),
    ("r", "https://www.reddit.com/search/?q=%s"),
];

/// Sites whose search the field offers by name ("red" → Search Reddit),
/// through their keyword.
pub const SITE_NAMES: &[(&str, &str)] = &[
    ("Wikipedia", "w"),
    ("YouTube", "yt"),
    ("GitHub", "gh"),
    ("MDN", "mdn"),
    ("OpenStreetMap", "osm"),
    ("Reddit", "r"),
];

/// The sites whose name starts with what was typed (two letters at least,
/// no space): offered as "Search <site>".
pub fn site_search(typed: &str) -> Vec<(&'static str, &'static str)> {
    let t = typed.trim().to_lowercase();
    if t.chars().count() < 2 || t.contains(' ') {
        return Vec::new();
    }
    SITE_NAMES.iter().filter(|(name, _)| name.to_lowercase().starts_with(&t)).copied().collect()
}

/// Whether `template` may be a keyword's address: https (or http), one
/// `%s`, and not in the host, so the words decide what is looked for, never
/// where you end up.
pub fn keyword_template_ok(template: &str) -> bool {
    let Some(rest) = template.strip_prefix("https://").or_else(|| template.strip_prefix("http://")) else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    template.matches("%s").count() == 1 && !host.contains("%s") && !host.contains('@') && !host.is_empty()
}

/// "yt cats" → YouTube's search for cats, from `keywords` (word, template).
pub fn keyword(typed: &str, keywords: &[(String, String)]) -> Option<String> {
    let (word, words) = typed.trim().split_once(' ')?;
    let words = words.trim();
    if words.is_empty() {
        return None;
    }
    let (_, template) = keywords.iter().find(|(w, t)| w.eq_ignore_ascii_case(word) && keyword_template_ok(t))?;
    Some(template.replacen("%s", &encode_query(words), 1))
}

/// Where the search engine answers "what might I be typing?", for the
/// engines that offer it (OpenSearch suggestions). Only used when
/// `suggestions = on`: it sends each keystroke to the engine.
pub fn suggest_endpoint(search_template: &str) -> Option<String> {
    if search_template.starts_with("https://duckduckgo.com/") {
        Some("https://duckduckgo.com/ac/?type=list&q={}".into())
    } else if search_template.starts_with("https://search.brave.com/") {
        Some("https://search.brave.com/api/suggest?q={}".into())
    } else {
        None
    }
}

/// An OpenSearch suggestions answer, `["typed", ["one", "two", …]]`.
pub fn parse_suggestions(json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    value
        .get(1)
        .and_then(|v| v.as_array())
        .map(|list| list.iter().filter_map(|s| s.as_str().map(str::to_string)).take(5).collect())
        .unwrap_or_default()
}

/// The built-in keywords, as owned pairs.
pub fn default_keywords() -> Vec<(String, String)> {
    KEYWORDS.iter().map(|(w, t)| (w.to_string(), t.to_string())).collect()
}

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
    let scheme = if is_local_host(&host) { "http" } else { "https" };
    // Dev servers print 0.0.0.0; WebKit refuses to go there, so use localhost.
    let rest = if host == "0.0.0.0" { text.replacen("0.0.0.0", "localhost", 1) } else { text.to_string() };
    Some(format!("{scheme}://{rest}"))
}

/// This machine, the local network, or a name with no dot (an intranet
/// host like `router`): places that rarely have a certificate.
/// Where pages and rippl's own requests go out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proxy {
    /// The desktop's own proxy settings (none, unless it has some).
    System,
    /// Straight out, whatever the desktop says.
    Direct,
    /// Through this proxy: `http://host:port`, `socks5://host:port`, …
    Through(String),
}

/// Hosts never sent through a proxy: this machine and `.local` names, or
/// a proxy would cut you off from them.
pub const PROXY_DIRECT: [&str; 4] = ["localhost", "127.0.0.1", "::1", "*.local"];

/// The `proxy` setting, checked: `system`, `direct`, or a proxy's address
/// with its scheme and port. No password in it: the config is plain text.
pub fn proxy(value: &str) -> Option<Proxy> {
    let value = value.trim();
    match value {
        "system" | "" => return Some(Proxy::System),
        "direct" | "off" | "none" => return Some(Proxy::Direct),
        _ => {}
    }
    let (scheme, rest) = value.split_once("://")?;
    if !["http", "https", "socks", "socks4", "socks4a", "socks5"].contains(&scheme) {
        return None;
    }
    let rest = rest.trim_end_matches('/');
    let host_port = match rest.rsplit_once('@') {
        Some((user, hp)) if !user.contains(':') && !user.is_empty() => hp,
        Some(_) => return None,
        None => rest,
    };
    let (host, port) = host_port.rsplit_once(':')?;
    port.parse::<u16>().ok().filter(|p| *p > 0)?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() || host.contains(['/', ' ', '?', '#']) {
        return None;
    }
    Some(Proxy::Through(format!("{scheme}://{rest}")))
}

pub fn is_local_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".lan")
        || host == "127.0.0.1"
        || host == "0.0.0.0"
        || host == "::1"
        || host.contains(':') // any other IPv6 literal: leave it alone
        || !host.contains('.')
        || private_range(&host)
}

/// HTTPS-only: the https address to use instead of `url`, when `url` is
/// plain http to a public host. `None`: leave it as it is.
pub fn https_upgrade(url: &str) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let hostport = authority.rsplit('@').next()?;
    let host = if hostport.starts_with('[') {
        hostport.split(']').next()?.trim_start_matches('[')
    } else {
        hostport.split(':').next()?
    };
    if host.is_empty() || is_local_host(host) {
        return None;
    }
    // An explicit port other than 80 is a server of its own: http on :8080
    // doesn't mean https on :8080.
    if let Some((_, port)) = hostport.rsplit_once(':')
        && !hostport.ends_with(']')
        && port != "80"
    {
        return None;
    }
    let rest = rest.replacen(":80", "", if hostport.ends_with(":80") { 1 } else { 0 });
    Some(format!("https://{rest}"))
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

/// The one letter that stands for a page: the site's
/// first letter, without `www.`; for rippl's own pages, the page's; for
/// anything without a host, the title's.
pub fn monogram(url: &str, title: &str) -> String {
    let from = |s: &str| s.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect::<String>());
    if let Some(page) = url.strip_prefix("rippl://") {
        return from(page).unwrap_or_else(|| "R".into());
    }
    let host = url.split_once("://").map(|(_, r)| r.split(['/', '?', '#', ':']).next().unwrap_or("")).unwrap_or("");
    // The name people know the site by: "wikipedia" in en.wikipedia.org,
    // "bbc" in www.bbc.co.uk (a short second-to-last label is a suffix).
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let name = match labels.len() {
        0 => "",
        1 => labels[0],
        n if n >= 3 && labels[n - 2].len() <= 3 && labels[n - 1].len() == 2 => labels[n - 3],
        n => labels[n - 2],
    };
    from(name).or_else(|| from(title)).unwrap_or_else(|| "·".into())
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

    #[test]
    fn proxies_are_checked() {
        assert_eq!(proxy("system"), Some(Proxy::System));
        assert_eq!(proxy("direct"), Some(Proxy::Direct));
        assert_eq!(proxy("socks5://127.0.0.1:9050"), Some(Proxy::Through("socks5://127.0.0.1:9050".into())));
        assert_eq!(proxy("http://me@proxy.example:3128/"), Some(Proxy::Through("http://me@proxy.example:3128".into())));
        assert_eq!(proxy("http://[::1]:8080"), Some(Proxy::Through("http://[::1]:8080".into())));
        assert_eq!(proxy("http://me:secret@proxy.example:3128"), None, "no passwords in the config");
        assert_eq!(proxy("ftp://proxy:21"), None);
        assert_eq!(proxy("socks5://proxy"), None, "a port is needed");
        assert_eq!(proxy("socks5://proxy:0"), None);
    }

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
    fn https_upgrades() {
        assert_eq!(https_upgrade("http://example.com/a?b#c").as_deref(), Some("https://example.com/a?b#c"));
        assert_eq!(https_upgrade("http://example.com:80/x").as_deref(), Some("https://example.com/x"));
        assert_eq!(https_upgrade("https://example.com/"), None, "already https");
        assert_eq!(https_upgrade("http://localhost:3000/"), None);
        assert_eq!(https_upgrade("http://192.168.1.1/"), None);
        assert_eq!(https_upgrade("http://router/"), None, "intranet name");
        assert_eq!(https_upgrade("http://printer.local/"), None);
        assert_eq!(https_upgrade("http://[::1]:8080/"), None);
        assert_eq!(https_upgrade("http://example.com:8080/"), None, "its own port");
        assert_eq!(https_upgrade("ftp://example.com/"), None);
    }

    #[test]
    fn keywords() {
        let k = default_keywords();
        assert_eq!(keyword("yt lo-fi beats", &k).as_deref(), Some("https://www.youtube.com/results?search_query=lo-fi+beats"));
        assert_eq!(keyword("YT cats", &k).as_deref(), Some("https://www.youtube.com/results?search_query=cats"));
        assert_eq!(keyword("yt", &k), None, "no words");
        assert_eq!(keyword("youtube cats", &k), None, "not a keyword");
        assert!(!keyword_template_ok("https://%s.evil.com/"), "words in the host");
        assert!(!keyword_template_ok("https://a.com/?q=%s&r=%s"));
        assert!(!keyword_template_ok("javascript:%s"));
    }

    #[test]
    fn suggestions() {
        assert!(suggest_endpoint(DEFAULT_SEARCH).is_some());
        assert!(suggest_endpoint("https://www.mojeek.com/search?q={}").is_none());
        assert_eq!(parse_suggestions(r#"["rus",["rust","rust lang"]]"#), vec!["rust", "rust lang"]);
        assert!(parse_suggestions("<html>").is_empty());
    }

    #[test]
    fn site_search_offers() {
        assert_eq!(site_search("red"), vec![("Reddit", "r")]);
        assert_eq!(site_search("YOU"), vec![("YouTube", "yt")]);
        assert!(site_search("r").is_empty(), "one letter is too little");
        assert!(site_search("red dit").is_empty());
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
    fn monograms() {
        assert_eq!(monogram("https://www.github.com/x", "GitHub"), "G");
        assert_eq!(monogram("https://en.wikipedia.org/wiki/Linux", ""), "W");
        assert_eq!(monogram("https://www.bbc.co.uk/news", ""), "B");
        assert_eq!(monogram("http://localhost:3000/", ""), "L");
        assert_eq!(monogram("https://are.na/", ""), "A");
        assert_eq!(monogram("rippl://history", "History"), "H");
        assert_eq!(monogram("about:blank", "blank page"), "B");
        assert_eq!(monogram("https://ελ.example/", ""), "Ε");
    }

    #[test]
    fn pretty_titles() {
        assert_eq!(pretty("https://www.example.com/"), "example.com");
        assert_eq!(pretty("https://example.com/a/b?q=1"), "example.com/a/b");
    }
}
