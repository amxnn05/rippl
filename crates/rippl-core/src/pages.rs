//! rippl's own pages, `rippl://history`, `rippl://bookmarks` and
//! `rippl://settings`, as HTML strings. The app serves them (see
//! internal_pages.rs) and acts on their buttons.
//!
//! Safety:
//! - Everything from the outside world (titles, URLs, hosts, selectors) is
//!   HTML-escaped. Nothing is put into script except the token, which is
//!   hex.
//! - A button is a navigation to `rippl://<page>?do=<act>&arg=…&token=…`.
//!   The token is random for each launch and only appears inside rippl's
//!   own pages. A web page can't open `rippl://` at all (the scheme is
//!   registered local and display-isolated), and a local HTML file that
//!   tries can't know the token, so it can't clear your history or change
//!   a permission.
//! - Each page carries a Content-Security-Policy that allows only its own
//!   inline script and style.

use crate::places::Place;

/// `&<>"'` escaped for HTML text and attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `%XX` and `+` decoded.
pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of `key` in a URL's query string.
pub fn param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == key).then(|| decode(v))
    })
}

/// `rippl://history?q=x` → `history`.
pub fn page_name(url: &str) -> String {
    let rest = url.strip_prefix("rippl://").unwrap_or("");
    rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase()
}

fn frame(page: &str, title: &str, token: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; --dim: #8888; --line: #8883; --accent: #3b82f6; }}
body {{ font: 15px/1.45 system-ui, sans-serif; max-width: 860px; margin: 32px auto; padding: 0 20px; }}
nav a {{ margin-right: 16px; color: inherit; opacity: .6; text-decoration: none; }}
nav a.here {{ opacity: 1; font-weight: 600; }}
h1 {{ font-size: 22px; margin: 22px 0 14px; }} h2 {{ font-size: 16px; margin: 28px 0 8px; }}
.row {{ display: flex; align-items: center; gap: 12px; padding: 7px 4px; border-bottom: 1px solid var(--line); }}
.row .main {{ flex: 1; min-width: 0; }}
.row .main a {{ color: inherit; text-decoration: none; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.row .sub {{ color: var(--dim); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.when {{ color: var(--dim); font-size: 13px; width: 120px; flex: none; }}
button {{ font: inherit; font-size: 13px; padding: 3px 10px; border-radius: 6px; border: 1px solid var(--line); background: transparent; color: inherit; cursor: pointer; }}
button:hover {{ border-color: var(--accent); }}
button.danger:hover {{ border-color: #e44; color: #e44; }}
input[type=search] {{ font: inherit; width: 100%; box-sizing: border-box; padding: 8px 12px; border-radius: 8px; border: 1px solid var(--line); background: transparent; color: inherit; }}
.empty {{ color: var(--dim); padding: 12px 4px; }}
.tag {{ font-size: 12px; padding: 1px 8px; border-radius: 10px; border: 1px solid var(--line); }}
.allow {{ color: #2a2; }} .deny {{ color: #e44; }}
code {{ font-size: 13px; }}
</style></head><body>
<nav><a href="rippl://history"{h}>History</a><a href="rippl://bookmarks"{b}>Bookmarks</a><a href="rippl://passwords"{p}>Passwords</a><a href="rippl://settings"{s}>Settings</a></nav>
{body}
<script>
const TOKEN = "{token}", PAGE = "{page}";
document.addEventListener("click", e => {{
  const b = e.target.closest("[data-do]");
  if (!b) return;
  if (b.dataset.confirm && !confirm(b.dataset.confirm)) return;
  const q = new URLSearchParams(location.search).get("q") || "";
  location.replace("rippl://" + PAGE + "?do=" + encodeURIComponent(b.dataset.do) +
    "&arg=" + encodeURIComponent(b.dataset.arg || "") + "&token=" + TOKEN + (q ? "&q=" + encodeURIComponent(q) : ""));
}});
if (new URLSearchParams(location.search).has("do")) {{
  const q = new URLSearchParams(location.search).get("q");
  history.replaceState(null, "", "rippl://" + PAGE + (q ? "?q=" + encodeURIComponent(q) : ""));
}}
</script>
</body></html>"#,
        title = escape(title),
        h = if page == "history" { r#" class="here""# } else { "" },
        b = if page == "bookmarks" { r#" class="here""# } else { "" },
        s = if page == "settings" { r#" class="here""# } else { "" },
        p = if page == "passwords" { r#" class="here""# } else { "" },
        token = token.chars().filter(char::is_ascii_hexdigit).collect::<String>(),
        page = page,
    )
}

/// "3 min ago", "yesterday 14:05"-style, from unix seconds.
pub fn ago(then: i64, now: i64) -> String {
    let d = (now - then).max(0);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", d / 86_400),
    }
}

fn place_row(p: &Place, now: i64, button: &str) -> String {
    let title = if p.title.trim().is_empty() { &p.url } else { &p.title };
    format!(
        r#"<div class="row"><span class="when">{when}</span><div class="main"><a href="{url}">{title}</a><div class="sub">{url_text}</div></div>{button}</div>"#,
        when = ago(p.last, now),
        url = escape(&p.url),
        title = escape(title),
        url_text = escape(crate::suggest::bare(&p.url)),
    )
}

pub fn history(places: &[Place], q: &str, now: i64, token: &str) -> String {
    let mut body = format!(
        r#"<h1>History</h1><form action="rippl://history"><input type="search" name="q" value="{q}" placeholder="Search history" autofocus></form>"#,
        q = escape(q)
    );
    if places.is_empty() {
        body.push_str(r#"<p class="empty">Nothing here.</p>"#);
    }
    for p in places {
        let button = format!(r#"<button data-do="forget" data-arg="{}">Forget</button>"#, escape(&p.url));
        body.push_str(&place_row(p, now, &button));
    }
    body.push_str(r#"<h2>Clear</h2><p><button class="danger" data-do="clear" data-confirm="Forget all history? Bookmarks stay.">Clear all history</button></p>"#);
    frame("history", "History", token, &body)
}

/// `sources`: other browsers found, as labels ("Chrome — Default").
pub fn bookmarks(places: &[Place], sources: &[String], note: &str, now: i64, token: &str) -> String {
    let mut body = String::from("<h1>Bookmarks</h1>");
    if !note.is_empty() {
        body.push_str(&format!(r#"<p class="tag">{}</p>"#, escape(note)));
    }
    if places.is_empty() {
        body.push_str(r#"<p class="empty">No bookmarks yet. Ctrl+D keeps the page you're on.</p>"#);
    }
    for p in places {
        let button = format!(r#"<button data-do="remove" data-arg="{}">Remove</button>"#, escape(&p.url));
        body.push_str(&place_row(p, now, &button));
    }
    body.push_str("<h2>Bring bookmarks over</h2>");
    if sources.is_empty() {
        body.push_str(r#"<p class="empty">No other browser's bookmarks found on this machine.</p>"#);
    }
    for (i, label) in sources.iter().enumerate() {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{}</div><button data-do="import" data-arg="{i}">Import</button></div>"#,
            escape(label)
        ));
    }
    frame("bookmarks", "Bookmarks", token, &body)
}

/// `accounts`: (origin, username); `never`: origins never offered saving.
/// No password is ever on this page: only which accounts exist.
pub fn passwords(accounts: &[(String, String)], never: &[String], store: &str, token: &str) -> String {
    let mut body = format!(
        "<h1>Passwords</h1><p class=\"sub\">Kept in {}, never in rippl's files. To see a password, open your keyring app (Passwords and Keys).</p>",
        escape(store)
    );
    if accounts.is_empty() {
        body.push_str(r#"<p class="empty">None saved yet. rippl offers to save a sign-in once it has worked.</p>"#);
    }
    for (origin, username) in accounts {
        let host = crate::passwords::host_of_origin(origin);
        let who = if username.is_empty() { "(no username)" } else { username };
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{who}<div class="sub">{host}</div></div><button class="danger" data-do="delete" data-arg="{arg}" data-confirm="Delete the saved password for {who} on {host}?">Delete</button></div>"#,
            who = escape(who),
            host = escape(host),
            arg = escape(&format!("{origin}\n{username}")),
        ));
    }
    body.push_str("<h2>Never offered on</h2>");
    if never.is_empty() {
        body.push_str(r#"<p class="empty">No site.</p>"#);
    }
    for origin in never {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{h}</div><button data-do="allow-saving" data-arg="{o}">Offer again</button></div>"#,
            h = escape(crate::passwords::host_of_origin(origin)),
            o = escape(origin)
        ));
    }
    frame("passwords", "Passwords", token, &body)
}

/// What the settings page shows.
pub struct SettingsView<'a> {
    pub permissions: &'a [(String, String, bool)],
    pub shield_off: &'a [String],
    pub hidden: &'a [(String, Vec<String>)],
    /// (name, value) lines from the config.
    pub config: &'a [(String, String)],
    pub config_path: &'a str,
}

pub fn settings(v: &SettingsView, token: &str) -> String {
    let mut body = String::from("<h1>Settings</h1><h2>Site permissions</h2>");
    if v.permissions.is_empty() {
        body.push_str(r#"<p class="empty">No site has asked yet.</p>"#);
    }
    for (host, kind, allow) in v.permissions {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{host}</div><span>{kind}</span><span class="tag {cls}">{word}</span><button data-do="forget-permissions" data-arg="{host}">Forget</button></div>"#,
            host = escape(host),
            kind = escape(kind),
            cls = if *allow { "allow" } else { "deny" },
            word = if *allow { "allowed" } else { "denied" },
        ));
    }
    body.push_str("<h2>Shield is off for</h2>");
    if v.shield_off.is_empty() {
        body.push_str(r#"<p class="empty">Every site. Ctrl+Shift+B turns it off for the site you're on.</p>"#);
    }
    for host in v.shield_off {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{h}</div><button data-do="shield-on" data-arg="{h}">Turn Shield back on</button></div>"#,
            h = escape(host)
        ));
    }
    body.push_str("<h2>Hidden on sites</h2>");
    if v.hidden.is_empty() {
        body.push_str(r#"<p class="empty">Nothing. Ctrl+Shift+H, then click what you want gone.</p>"#);
    }
    for (host, selectors) in v.hidden {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{h}<div class="sub">{list}</div></div><button data-do="unhide" data-arg="{h}">Show again</button></div>"#,
            h = escape(host),
            list = escape(&selectors.join("  ·  ")),
        ));
    }
    body.push_str(&format!("<h2>Configuration</h2><p class=\"sub\">Edit <code>{}</code> and restart rippl.</p>", escape(v.config_path)));
    for (name, value) in v.config {
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{}</div><code>{}</code></div>"#,
            escape(name),
            escape(value)
        ));
    }
    frame("settings", "Settings", token, &body)
}

/// Reading mode: the article alone. `body` is the article as rebuilt by
/// the extractor (reader.rs), which keeps only plain tags and http(s)
/// links and images. The page's policy forbids every script, so even
/// something that slipped through could not run.
pub fn reader(title: &str, byline: &str, site: &str, url: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src https: http: data:; style-src 'unsafe-inline'">
<meta name="viewport" content="width=device-width">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ font: 19px/1.65 Charter, "Iowan Old Style", Georgia, serif; max-width: 680px; margin: 48px auto 96px; padding: 0 24px; }}
@media (prefers-color-scheme: light) {{ body {{ background: #fbfaf7; color: #1f1f1f; }} }}
@media (prefers-color-scheme: dark) {{ body {{ background: #1c1c1e; color: #e6e6e6; }} }}
.site {{ font: 14px system-ui, sans-serif; opacity: .6; text-transform: uppercase; letter-spacing: .05em; }}
h1.title {{ font-size: 34px; line-height: 1.2; margin: 8px 0 6px; }}
.byline {{ font: 15px system-ui, sans-serif; opacity: .7; margin-bottom: 32px; }}
img, figure {{ max-width: 100%; height: auto; }} figure {{ margin: 24px 0; }}
figcaption {{ font: 14px system-ui, sans-serif; opacity: .7; }}
pre {{ overflow-x: auto; font-size: 15px; padding: 12px; border-radius: 6px; background: #8881; }}
blockquote {{ border-left: 3px solid #8886; margin-left: 0; padding-left: 18px; opacity: .9; }}
a {{ color: inherit; }}
.source {{ font: 14px system-ui, sans-serif; opacity: .6; margin-top: 48px; word-break: break-all; }}
</style></head><body>
<div class="site">{site}</div><h1 class="title">{title}</h1><div class="byline">{byline}</div>
{body}
<p class="source">Reading mode (F9 to leave) · {url}</p>
</body></html>"#,
        title = escape(title),
        byline = escape(byline),
        site = escape(site),
        url = escape(url),
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(url: &str, title: &str) -> Place {
        Place { url: url.into(), title: title.into(), visits: 1, last: 0, bookmarked: false }
    }

    #[test]
    fn hostile_titles_are_escaped() {
        let evil = place("https://x.com/?a=\"><script>alert(1)</script>", "<img src=x onerror=alert(1)>");
        let html = history(&[evil], "\"><script>", 10, "abc123");
        assert!(!html.contains("<img src=x"));
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
        assert_eq!(html.matches("<script>").count(), 1, "only rippl's own script");
    }

    #[test]
    fn token_is_hex_only() {
        let html = history(&[], "", 0, "ab\";alert(1);//");
        // Only the hex characters survive: a, b, a, e, 1.
        assert!(html.contains(r#"const TOKEN = "abae1""#), "non-hex characters dropped");
        assert!(!html.contains("alert(1)"));
    }

    #[test]
    fn query_params() {
        let url = "rippl://history?do=forget&arg=https%3A%2F%2Fa.com%2F%3Fx%3D1&q=rust+lang&token=ff";
        assert_eq!(page_name(url), "history");
        assert_eq!(param(url, "do").as_deref(), Some("forget"));
        assert_eq!(param(url, "arg").as_deref(), Some("https://a.com/?x=1"));
        assert_eq!(param(url, "q").as_deref(), Some("rust lang"));
        assert_eq!(param(url, "none"), None);
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%E2%9C%93"), "✓");
    }

    #[test]
    fn reader_escapes_its_own_fields_and_forbids_script() {
        let html = reader("<b>t</b>", "by <i>x</i>", "site", "https://a/?\"><x", "<p>body</p>");
        assert!(html.contains("&lt;b&gt;t&lt;/b&gt;") && html.contains("<p>body</p>"));
        assert!(html.contains("default-src 'none'") && !html.contains("script-src"));
    }

    #[test]
    fn passwords_page_has_no_secrets_and_escapes() {
        let html = passwords(&[("https://x.com".into(), "<b>bob</b>".into())], &[], "test", "ff");
        assert!(html.contains("&lt;b&gt;bob&lt;/b&gt;") && html.contains("x.com"));
        assert!(html.contains("rippl://passwords"));
    }

    #[test]
    fn ages() {
        assert_eq!(ago(100, 130), "just now");
        assert_eq!(ago(0, 600), "10 min ago");
        assert_eq!(ago(0, 90_000), "yesterday");
        assert_eq!(ago(0, 864_000), "10 days ago");
    }
}
