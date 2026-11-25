//! Shield: Adblock Plus / EasyList filter syntax turned into WebKit content
//! blocker rules (the JSON format Safari uses, which WebKitGTK compiles
//! with `UserContentFilterStore`).
//!
//! WebKit enforces those rules inside its own networking and style code,
//! before a request leaves and before a page draws. That is why Shield costs
//! nothing while browsing, unlike a JavaScript blocker running in every page.
//!
//! What is converted:
//! - network rules: `||ads.example^`, `|https://x/`, `/banner/*.gif`, with
//!   the options `third-party`, resource types, `domain=`, `match-case`,
//!   `popup`
//! - exceptions: `@@…` become `ignore-previous-rules`; `@@…$document`
//!   turns blocking off on a whole site
//! - element hiding: `##.ad`, `example.com##.ad`, with `#@#` exceptions
//!
//! What is skipped (counted in `Stats::skipped`): regex rules, scriptlets,
//! `$csp`, `$redirect`, `$removeparam` and the other options WebKit has
//! no rule for, and extended CSS (`:has-text`, `:-abp-has`, …). Skipping a
//! rule only ever means blocking less, never breaking a page.
//!
//! Patterns are converted approximately. ABP's `^` separator means "a
//! character that is not a letter, digit or one of `_-.%`, or the end of
//! the URL". WebKit's regex has no `|` alternation, so the "end of the URL"
//! case is lost. A rule like `||x.com^` still works, because WebKit
//! normalises URLs to always have a `/` after the host.

use std::collections::{BTreeMap, BTreeSet};

/// Bump when the conversion changes, so compiled filters are rebuilt.
pub const CONVERTER_VERSION: u32 = 2;

/// Generic hiding selectors per rule. One rule per selector would multiply
/// the rule count; one huge rule makes one bad selector hide nothing.
const SELECTORS_PER_RULE: usize = 100;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub block: usize,
    pub allow: usize,
    pub hide: usize,
    pub skipped: usize,
}

impl Stats {
    pub fn rules(&self) -> usize {
        self.block + self.allow + self.hide
    }
}

/// One WebKit rule, written out by `json`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    url_filter: String,
    case_sensitive: bool,
    resource_type: Vec<&'static str>,
    load_type: Option<&'static str>,
    load_context: Option<&'static str>,
    if_domain: Vec<String>,
    unless_domain: Vec<String>,
    action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Block,
    Ignore,
    Hide(String),
}

impl Rule {
    fn new(url_filter: impl Into<String>, action: Action) -> Rule {
        Rule {
            url_filter: url_filter.into(),
            case_sensitive: false,
            resource_type: Vec::new(),
            load_type: None,
            load_context: None,
            if_domain: Vec::new(),
            unless_domain: Vec::new(),
            action,
        }
    }

    fn json(&self, out: &mut String) {
        out.push_str("{\"trigger\":{\"url-filter\":");
        push_str(out, &self.url_filter);
        if self.case_sensitive {
            out.push_str(",\"url-filter-is-case-sensitive\":true");
        }
        if !self.resource_type.is_empty() {
            out.push_str(",\"resource-type\":");
            push_list(out, self.resource_type.iter().copied());
        }
        if let Some(t) = self.load_type {
            out.push_str(",\"load-type\":[");
            push_str(out, t);
            out.push(']');
        }
        if let Some(c) = self.load_context {
            out.push_str(",\"load-context\":[");
            push_str(out, c);
            out.push(']');
        }
        if !self.if_domain.is_empty() {
            out.push_str(",\"if-domain\":");
            push_list(out, self.if_domain.iter().map(String::as_str));
        }
        if !self.unless_domain.is_empty() {
            out.push_str(",\"unless-domain\":");
            push_list(out, self.unless_domain.iter().map(String::as_str));
        }
        out.push_str("},\"action\":{\"type\":");
        match &self.action {
            Action::Block => push_str(out, "block"),
            Action::Ignore => push_str(out, "ignore-previous-rules"),
            Action::Hide(selector) => {
                push_str(out, "css-display-none");
                out.push_str(",\"selector\":");
                push_str(out, selector);
            }
        }
        out.push_str("}}");
    }
}

fn push_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_list<'a>(out: &mut String, items: impl Iterator<Item = &'a str>) {
    out.push('[');
    for (i, item) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_str(out, item);
    }
    out.push(']');
}

/// An ABP URL pattern as a WebKit url-filter regex. `None` when it can't be.
fn url_filter(pattern: &str) -> Option<String> {
    if !pattern.is_ascii() {
        return None;
    }
    let mut p = pattern;
    let mut out = String::with_capacity(p.len() * 2);
    if let Some(rest) = p.strip_prefix("||") {
        out.push_str("^[^:]+:(//)?([^/]+\\.)?");
        p = rest;
    } else if let Some(rest) = p.strip_prefix('|') {
        out.push('^');
        p = rest;
    }
    let anchored_end = p.ends_with('|');
    if anchored_end {
        p = &p[..p.len() - 1];
    }
    for c in p.chars() {
        match c {
            '*' => out.push_str(".*"),
            '^' => out.push_str("[^-_.%A-Za-z0-9]"),
            '.' | '+' | '?' | '{' | '}' | '(' | ')' | '[' | ']' | '\\' | '$' | '|' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    if anchored_end {
        out.push('$');
    }
    // Leading and trailing ".*" match anything anyway, and cost WebKit
    // compile time.
    let mut s = out.as_str();
    while let Some(rest) = s.strip_prefix(".*") {
        s = rest;
    }
    while let Some(rest) = s.strip_suffix(".*") {
        s = rest;
    }
    Some(if s.is_empty() { ".*".to_string() } else { s.to_string() })
}

/// A domain list (`a.com|~b.com` or `a.com,~b.com`) as WebKit's if-domain
/// or unless-domain. WebKit can't have both in one rule: `None` then.
fn domains(list: &str, sep: char) -> Option<(Vec<String>, Vec<String>)> {
    let mut yes = Vec::new();
    let mut no = Vec::new();
    for d in list.split(sep).map(str::trim).filter(|d| !d.is_empty()) {
        let (neg, d) = d.strip_prefix('~').map_or((false, d), |d| (true, d));
        // `example.*` (any TLD) has no WebKit equivalent.
        if !d.is_ascii() || d.contains('*') || d.contains('/') {
            return None;
        }
        let entry = format!("*{}", d.to_ascii_lowercase());
        if neg { no.push(entry) } else { yes.push(entry) }
    }
    if !yes.is_empty() && !no.is_empty() {
        return None;
    }
    Some((yes, no))
}

/// A network rule (block or exception), or `None` to skip it.
fn network(line: &str) -> Option<Rule> {
    let (allow, body) = line.strip_prefix("@@").map_or((false, line), |b| (true, b));
    let (pattern, options) = match body.rfind('$') {
        Some(i) if !body[i + 1..].is_empty() && !body[i + 1..].contains('/') => (&body[..i], Some(&body[i + 1..])),
        _ => (body, None),
    };
    if pattern.len() > 2 && pattern.starts_with('/') && pattern.ends_with('/') {
        return None; // a regex rule
    }

    let mut rule = Rule::new(url_filter(pattern)?, if allow { Action::Ignore } else { Action::Block });
    let mut restricted = false; // something besides the pattern narrows the rule
    for opt in options.into_iter().flat_map(|o| o.split(',')) {
        let types: &[&'static str] = match opt {
            "script" => &["script"],
            "image" => &["image"],
            "stylesheet" | "css" => &["style-sheet"],
            "font" => &["font"],
            "media" => &["media"],
            "xmlhttprequest" | "xhr" => &["fetch"],
            "websocket" => &["websocket"],
            "ping" => &["ping"],
            "other" | "object" => &["other"],
            "popup" => &["popup"],
            "subdocument" | "frame" => {
                rule.load_context = Some("child-frame");
                &["document"]
            }
            "document" | "doc" => {
                if allow {
                    // `@@||site^$document`: no blocking on that site's pages.
                    let host = pattern.trim_start_matches("||").trim_end_matches('^').trim_end_matches('/');
                    if host.is_empty() || host.contains(['/', '*', '^']) {
                        return None;
                    }
                    rule.url_filter = ".*".into();
                    rule.if_domain = vec![format!("*{}", host.to_ascii_lowercase())];
                    restricted = true;
                    &[]
                } else {
                    &["document"]
                }
            }
            "third-party" | "3p" => {
                rule.load_type = Some("third-party");
                restricted = true;
                &[]
            }
            "~third-party" | "1p" | "first-party" => {
                rule.load_type = Some("first-party");
                restricted = true;
                &[]
            }
            "match-case" => {
                rule.case_sensitive = true;
                &[]
            }
            "important" | "all" => &[],
            "strict3p" => {
                rule.load_type = Some("third-party");
                restricted = true;
                &[]
            }
            "strict1p" => {
                rule.load_type = Some("first-party");
                restricted = true;
                &[]
            }
            o if o.starts_with("domain=") || o.starts_with("from=") => {
                let list = o.split_once('=').map_or("", |(_, l)| l);
                let (yes, no) = domains(list, '|')?;
                rule.if_domain.extend(yes);
                rule.unless_domain = no;
                restricted = true;
                &[]
            }
            // Anything else WebKit can't express: skip the whole rule
            // rather than apply it wider than written.
            _ => return None,
        };
        if !types.is_empty() {
            restricted = true;
        }
        for t in types {
            if !rule.resource_type.contains(t) {
                rule.resource_type.push(t);
            }
        }
    }
    // A pattern that matches every URL with nothing narrowing it would
    // block the whole web.
    if rule.url_filter == ".*" && !restricted {
        return None;
    }
    Some(rule)
}

/// Extended CSS that only uBlock/AdGuard understand.
fn plain_css(selector: &str) -> bool {
    const EXTENDED: &[&str] = &[
        ":-abp-", ":has-text", ":contains", ":matches-css", ":xpath", ":upward", ":remove", ":style",
        ":min-text-length", ":watch-attr", ":matches-path", ":matches-attr", ":matches-prop", ":others",
        ":if(", ":if-not", ":nth-ancestor", ":-ext-", "+js(", ":matches-media", ":spath",
    ];
    // `^…` is uBlock's HTML filtering, not CSS.
    !selector.is_empty() && !selector.starts_with('^') && !EXTENDED.iter().any(|e| selector.contains(e))
}

/// uBlock's preprocessor: `!#if <cond>` … `!#else` … `!#endif`. rippl
/// is none of the environments uBlock knows (not Chromium, Firefox, Safari,
/// mobile, nor uBlock itself), so every `env_*`, `ext_*` and `cap_*` token
/// is false except `cap_user_stylesheet` (rippl hides elements with user
/// style sheets). `!`, `&&`, `||` and nesting are understood; lines in a
/// false branch are dropped.
pub fn preprocess(text: &str) -> String {
    fn token(t: &str) -> bool {
        let t = t.trim();
        if let Some(rest) = t.strip_prefix('!') {
            return !token(rest);
        }
        t == "cap_user_stylesheet"
    }
    fn cond(c: &str) -> bool {
        c.split("||").any(|and| and.split("&&").all(token))
    }
    let mut out = String::with_capacity(text.len());
    // Each open `!#if`: (whether its branch is taken, whether the
    // enclosing block was).
    let mut stack: Vec<(bool, bool)> = Vec::new();
    let live = |stack: &Vec<(bool, bool)>| stack.iter().all(|(taken, _)| *taken);
    for line in text.lines() {
        let l = line.trim();
        if let Some(c) = l.strip_prefix("!#if ") {
            let outer = live(&stack);
            stack.push((cond(c), outer));
        } else if l == "!#else" {
            if let Some(top) = stack.last_mut() {
                top.0 = !top.0;
            }
        } else if l == "!#endif" {
            stack.pop();
        } else if live(&stack) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The files a uBlock list pulls in with `!#include <name>`.
pub fn includes(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("!#include "))
        .map(|n| n.trim().to_string())
        // Only a plain name beside the list: never another site.
        .filter(|n| !n.is_empty() && !n.contains("://") && !n.contains("..") && !n.starts_with('/'))
        .collect()
}

/// The rules from ABP filter lists (`lists`), followed by an exception for
/// each site in `allowed` (sites the user turned Shield off for). Returns
/// WebKit's JSON and what was converted.
pub fn convert(lists: &[&str], allowed: &[String]) -> (String, Stats) {
    let mut stats = Stats::default();
    let mut blocks = Vec::new();
    let mut allows = Vec::new();
    let mut generic: Vec<String> = Vec::new();
    // Domain-specific hiding, grouped by domain set: many rules share one.
    let mut specific: BTreeMap<(Vec<String>, Vec<String>), Vec<String>> = BTreeMap::new();
    // `site#@#.ad`: generic `.ad` must not apply on those sites.
    let mut unhide: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let lists: Vec<String> = lists.iter().map(|l| preprocess(l)).collect();
    for line in lists.iter().flat_map(|l| l.lines()) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
            continue;
        }
        if let Some(i) = line.find("#@#") {
            let selector = &line[i + 3..];
            for d in line[..i].split(',').filter(|d| !d.is_empty() && !d.starts_with('~')) {
                unhide.entry(selector.to_string()).or_default().insert(format!("*{}", d.to_ascii_lowercase()));
            }
            continue;
        }
        if line.contains("#?#") || line.contains("#$#") || line.contains("#%#") || line.contains("$$") {
            stats.skipped += 1;
            continue;
        }
        if let Some(i) = line.find("##") {
            let (sites, selector) = (&line[..i], &line[i + 2..]);
            if !plain_css(selector) {
                stats.skipped += 1;
            } else if sites.is_empty() {
                generic.push(selector.to_string());
            } else if let Some(key) = domains(sites, ',') {
                specific.entry(key).or_default().push(selector.to_string());
            } else {
                stats.skipped += 1;
            }
            continue;
        }
        match network(line) {
            Some(rule) if rule.action == Action::Ignore => allows.push(rule),
            Some(rule) => blocks.push(rule),
            None => stats.skipped += 1,
        }
    }

    let mut hides = Vec::new();
    let mut plain: Vec<&String> = Vec::new();
    for selector in &generic {
        match unhide.get(selector) {
            Some(sites) => {
                let mut rule = Rule::new(".*", Action::Hide(selector.clone()));
                rule.unless_domain = sites.iter().cloned().collect();
                hides.push(rule);
            }
            None => plain.push(selector),
        }
    }
    for chunk in plain.chunks(SELECTORS_PER_RULE) {
        let joined = chunk.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
        hides.push(Rule::new(".*", Action::Hide(joined)));
    }
    for ((yes, no), selectors) in specific {
        for chunk in selectors.chunks(SELECTORS_PER_RULE) {
            let mut rule = Rule::new(".*", Action::Hide(chunk.join(", ")));
            rule.if_domain = yes.clone();
            rule.unless_domain = no.clone();
            hides.push(rule);
        }
    }
    for site in allowed {
        let mut rule = Rule::new(".*", Action::Ignore);
        rule.if_domain = vec![format!("*{}", site.to_ascii_lowercase())];
        allows.push(rule);
    }

    // Pings (<a ping>, navigator.sendBeacon) to another site are reports
    // to a tracker, whatever list they are on.
    let mut ping = Rule::new(".*", Action::Block);
    ping.resource_type = vec!["ping"];
    ping.load_type = Some("third-party");
    blocks.insert(0, ping);

    stats.block = blocks.len();
    stats.hide = hides.len();
    stats.allow = allows.len();
    // Order matters: an exception only cancels the rules before it.
    let mut out = String::with_capacity((blocks.len() + hides.len() + allows.len()) * 120);
    out.push('[');
    for (i, rule) in blocks.iter().chain(&hides).chain(&allows).enumerate() {
        if i > 0 {
            out.push(',');
        }
        rule.json(&mut out);
    }
    out.push(']');
    (out, stats)
}

/// A few of the largest ad and tracking networks: what blocks on first
/// launch while the full lists download.
pub const SEED: &str = "\
||doubleclick.net^
||googlesyndication.com^
||googleadservices.com^
||google-analytics.com^
||googletagmanager.com^
||googletagservices.com^
||adservice.google.com^
||amazon-adsystem.com^
||adnxs.com^
||criteo.com^
||criteo.net^
||taboola.com^
||outbrain.com^
||scorecardresearch.com^
||quantserve.com^
||pubmatic.com^
||rubiconproject.com^
||openx.net^
||casalemedia.com^
||moatads.com^
||adsrvr.org^
||hotjar.com^
||connect.facebook.net^
||ads-twitter.com^
||analytics.tiktok.com^
";

#[cfg(test)]
mod tests {
    use super::*;

    fn one(line: &str) -> Option<String> {
        network(line).map(|r| {
            let mut s = String::new();
            r.json(&mut s);
            s
        })
    }

    #[test]
    fn host_anchor() {
        let json = one("||ads.example.com^").unwrap();
        assert!(json.contains(r#""url-filter":"^[^:]+:(//)?([^/]+\\.)?ads\\.example\\.com[^-_.%A-Za-z0-9]""#), "{json}");
        assert!(json.contains(r#""type":"block""#));
    }

    #[test]
    fn options() {
        let json = one("||x.com^$script,third-party,domain=a.com|b.org").unwrap();
        assert!(json.contains(r#""resource-type":["script"]"#), "{json}");
        assert!(json.contains(r#""load-type":["third-party"]"#));
        assert!(json.contains(r#""if-domain":["*a.com","*b.org"]"#));
        let json = one("/banner/*.gif$~third-party").unwrap();
        assert!(json.contains(r#""url-filter":"/banner/.*\\.gif""#), "{json}");
        assert!(json.contains(r#""load-type":["first-party"]"#));
    }

    #[test]
    fn skipped() {
        assert_eq!(one("/^https?:\\/\\/ads/"), None, "regex");
        assert_eq!(one("||x.com^$csp=script-src 'none'"), None, "csp");
        assert_eq!(one("||x.com^$removeparam=utm"), None);
        assert_eq!(one("||x.com^$domain=a.com|~b.com"), None, "mixed domains");
        assert_eq!(one("*"), None, "everything");
        assert_eq!(one("||exämple.com^"), None, "non-ascii");
    }

    #[test]
    fn exceptions() {
        let json = one("@@||cdn.site.com^$script").unwrap();
        assert!(json.contains("ignore-previous-rules"));
        let json = one("@@||bank.com^$document").unwrap();
        assert!(json.contains(r#""url-filter":".*""#) && json.contains(r#""if-domain":["*bank.com"]"#), "{json}");
    }

    #[test]
    fn full_list_orders_blocks_hides_exceptions() {
        let list = "! comment\n[Adblock Plus 2.0]\n##.ad-banner\n##.promo\nsite.com#@#.promo\nnews.com##.sidebar-ad\n\
                    example.com#?#div:has-text(Ad)\n||ads.com^\n@@||ads.com/ok.js\n";
        let (json, stats) = convert(&[list], &["mybank.com".into()]);
        assert_eq!(stats, Stats { block: 2, allow: 2, hide: 3, skipped: 1 });
        let block = json.find(r#""block""#).unwrap();
        let hide = json.find("css-display-none").unwrap();
        let ignore = json.find("ignore-previous-rules").unwrap();
        assert!(block < hide && hide < ignore);
        assert!(json.contains(r#""selector":".promo"},"#) || json.contains(r#""selector":".promo"}}"#));
        assert!(json.contains(r#""unless-domain":["*site.com"]"#));
        assert!(json.contains(r#""if-domain":["*news.com"]"#));
        assert!(json.contains(r#""if-domain":["*mybank.com"]"#));
    }

    #[test]
    fn extended_css_is_skipped() {
        assert!(!plain_css("div:-abp-has(.ad)"));
        assert!(!plain_css("a:has-text(Sponsored)"));
        assert!(plain_css("div[id^=\"ad-\"] > .x"));
    }

    #[test]
    fn ublock_preprocessor() {
        let text = "a\n!#if env_firefox\nfirefox-only\n!#else\nnot-firefox\n!#endif\n!#if !env_mobile\ndesktop\n!#if ext_ubol\nubol\n!#endif\n!#endif\n!#if cap_user_stylesheet || env_safari\ncss\n!#endif\nz\n";
        assert_eq!(preprocess(text), "a\nnot-firefox\ndesktop\ncss\nz\n");
    }

    #[test]
    fn ublock_includes_stay_beside_the_list() {
        let text = "!#include filters-2024.txt\n!#include https://evil.example/x.txt\n!#include ../up.txt\n";
        assert_eq!(includes(text), vec!["filters-2024.txt"]);
    }

    #[test]
    fn ublock_options() {
        assert!(one("||ads.com^$from=a.com|b.com").unwrap().contains(r#""if-domain":["*a.com","*b.com"]"#));
        assert!(one("||ads.com^$all").is_some());
        assert!(one("||ads.com^$strict3p").unwrap().contains("third-party"));
        assert!(!plain_css("^script:has-text(ad)"), "HTML filtering");
    }

    #[test]
    fn json_escapes() {
        let mut s = String::new();
        push_str(&mut s, "a\"b\\c\n");
        assert_eq!(s, r#""a\"b\\c\u000a""#);
    }
}
