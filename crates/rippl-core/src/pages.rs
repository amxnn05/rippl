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
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:; connect-src rippl:">
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
select, input.value {{ font: inherit; font-size: 13px; padding: 3px 8px; border-radius: 6px; border: 1px solid var(--line); background: transparent; color: inherit; }}
input.value {{ width: 260px; }}
.setting input.value {{ width: 210px; flex: none; }}
.note {{ color: var(--dim); font-size: 12px; }}
input[type=search] {{ font: inherit; width: 100%; box-sizing: border-box; padding: 8px 12px; border-radius: 8px; border: 1px solid var(--line); background: transparent; color: inherit; }}
.empty {{ color: var(--dim); padding: 12px 4px; }}
.tag {{ font-size: 12px; padding: 1px 8px; border-radius: 10px; border: 1px solid var(--line); }}
.allow {{ color: #2a2; }} .deny {{ color: #e44; }}
code {{ font-size: 13px; }}
body.page-settings {{ max-width: none; margin: 0; padding: 0;
  font: 13px/1.4 "Inter", "Adwaita Sans", system-ui, sans-serif;
  --ground: #ffffff; --ink: #171717; --muted: rgba(0,0,0,.46); --hairline: #e8e8e8; --wash: rgba(0,0,0,.063); --hover: rgba(0,0,0,.04);
  background: var(--ground); color: var(--ink); }}
@media (prefers-color-scheme: dark) {{ body.page-settings {{
  --ground: #1c1c1c; --ink: #ededed; --muted: rgba(255,255,255,.55); --hairline: #333333; --wash: rgba(255,255,255,.087); --hover: rgba(255,255,255,.06); }} }}
body.page-settings > nav, body.page-welcome > nav {{ display: none; }}
body.page-welcome {{ max-width: 600px; margin: 0 auto; padding: 56px 24px; font: 13px/1.45 "Inter", "Adwaita Sans", system-ui, sans-serif;
  --ground: #ffffff; --ink: #171717; --muted: rgba(0,0,0,.46); --hairline: #e8e8e8; --wash: rgba(0,0,0,.063); --hover: rgba(0,0,0,.04);
  background: var(--ground); color: var(--ink); }}
@media (prefers-color-scheme: dark) {{ body.page-welcome {{
  --ground: #1c1c1c; --ink: #ededed; --muted: rgba(255,255,255,.55); --hairline: #333333; --wash: rgba(255,255,255,.087); --hover: rgba(255,255,255,.06); }} }}
.welcome .logo {{ width: 96px; height: 96px; display: block; margin: 0 auto 18px; }}
.welcome h1 {{ text-align: center; font-size: 24px; margin: 0 0 8px; }}
.welcome .lede {{ text-align: center; color: var(--muted); margin: 0 auto 28px; max-width: 460px; font-size: 14px; }}
.welcome .card {{ border: 1px solid var(--hairline); border-radius: 12px; overflow: hidden; }}
.welcome .keys {{ display: grid; grid-template-columns: 1fr 1fr; gap: 8px 20px; margin: 26px 4px; }}
.welcome .key {{ display: flex; gap: 10px; align-items: center; color: var(--muted); font-size: 12.5px; }}
.welcome .go {{ text-align: center; }}
.welcome .button, .welcome button {{ font: 12.5px "Inter", system-ui, sans-serif; color: var(--ink); background: var(--ground); border: 1px solid var(--hairline);
  border-radius: 7px; padding: 4px 10px; text-decoration: none; cursor: pointer; }}
.welcome .button.primary {{ background: var(--ink); color: var(--ground); border-color: var(--ink); padding: 8px 18px; font-size: 13.5px; }}
.done {{ color: var(--muted); font-size: 12px; }}
/* Motion, as rippl's own (motion.rs): sections slide in, rows answer the
   pointer, buttons give; the welcome's parts rise one after another. */
@keyframes in-up {{ from {{ opacity: 0; transform: translateY(8px); }} to {{ opacity: 1; transform: none; }} }}
@keyframes in-pop {{ from {{ opacity: 0; transform: scale(.92); }} to {{ opacity: 1; transform: none; }} }}
.settings .cat:not([hidden]) {{ animation: in-up .28s cubic-bezier(.25,1.15,.4,1); }}
.setting {{ transition: background-color .14s ease-out; }}
.settings .card .setting:hover {{ background: var(--hover); }}
button, a.button, .row {{ transition: background-color .14s ease-out, border-color .14s ease-out, transform .16s cubic-bezier(.2,1.3,.4,1); }}
button:active, a.button:active {{ transform: scale(.97); }}
.settings aside a {{ transition: background-color .2s ease-out, color .14s ease-out, box-shadow .24s ease-out; }}
.toast {{ animation: in-up .3s cubic-bezier(.25,1.2,.4,1); }}
.welcome .logo {{ animation: in-pop .5s cubic-bezier(.2,1.35,.4,1) both; }}
.welcome h1 {{ animation: in-up .45s .08s cubic-bezier(.25,1.15,.4,1) both; }}
.welcome .lede {{ animation: in-up .45s .14s cubic-bezier(.25,1.15,.4,1) both; }}
.welcome .card {{ animation: in-up .5s .2s cubic-bezier(.25,1.15,.4,1) both; }}
.welcome .keys {{ animation: in-up .5s .28s cubic-bezier(.25,1.15,.4,1) both; }}
.welcome .go {{ animation: in-up .5s .34s cubic-bezier(.25,1.15,.4,1) both; }}
.row:hover {{ background: #8881; }}
@media (prefers-reduced-motion: reduce) {{ *, *::before, *::after {{ animation: none !important; transition: none !important; }} }}
.settings {{ display: flex; min-height: 100vh; }}
body.rail-in-column .settings aside {{ display: none; }}
.settings aside {{ flex: none; width: 196px; box-sizing: border-box; padding: 8px; position: sticky; top: 0; height: 100vh; overflow-y: auto;
  background: color-mix(in srgb, var(--wash) 45%, transparent); border-right: 1px solid var(--hairline); display: flex; flex-direction: column; gap: 2px; }}
.rail-title {{ font-size: 13px; font-weight: 600; padding: 14px 10px 10px; }}
.find {{ display: flex; align-items: center; gap: 7px; height: 28px; padding: 0 9px; margin-bottom: 10px; border-radius: 8px;
  background: var(--ground); border: 1px solid var(--hairline); }}
.find .lens {{ color: var(--muted); font-size: 12px; }}
.find input {{ flex: 1; min-width: 0; border: none; outline: none; background: transparent; color: var(--ink); font: 12.5px "Inter", system-ui, sans-serif; padding: 0; }}
.find input::placeholder {{ color: var(--muted); }}
.find input::-webkit-search-cancel-button {{ display: none; }}
.find kbd.hint {{ border: none; padding: 0; font-size: 10.5px; color: var(--muted); }}
.find:focus-within kbd.hint {{ display: none; }}
.settings aside .rule {{ height: 1px; background: var(--hairline); margin: 7px 10px; }}
.settings aside a {{ display: flex; align-items: center; gap: 9px; height: 30px; padding: 0 10px; border-radius: 8px; color: var(--muted);
  text-decoration: none; font-size: 13px; transition: background .15s, color .15s; }}
.settings aside a:hover {{ background: var(--hover); color: var(--ink); }}
.settings aside a.here {{ background: var(--ground); color: var(--ink); font-weight: 500; box-shadow: 0 1px 3px rgba(0,0,0,.06), 0 0 0 1px var(--hairline); }}
.settings aside a.none {{ opacity: .4; }}
.settings aside .name {{ flex: 1; }}
.settings aside .count:not(:empty) {{ font-size: 10.5px; font-weight: 500; padding: 0 6px; height: 17px; line-height: 17px; border-radius: 9px; background: var(--ground); color: var(--ink); }}
.settings .glyph {{ width: 16px; text-align: center; font-size: 12px; }}
.settings main {{ flex: 1; min-width: 0; padding: 26px 24px 24px; }}
.settings .column {{ max-width: 600px; margin: 0 auto; }}
.settings header {{ display: flex; align-items: center; gap: 10px; font-size: 17px; font-weight: 600; margin-bottom: 18px; }}
.settings header .glyph {{ font-size: 15px; width: auto; }}
.settings .group {{ margin-bottom: 22px; }}
.settings .group h2 {{ display: flex; align-items: center; gap: 6px; margin: 0 0 8px 2px; font-size: 10.5px; font-weight: 600; letter-spacing: .5px;
  text-transform: uppercase; color: color-mix(in srgb, var(--ink) 62%, transparent); }}
.settings .group h2 .glyph {{ width: auto; font-size: 10.5px; }}
.settings .card {{ border: 1px solid var(--hairline); border-radius: 10px; background: var(--ground); overflow: hidden; }}
.setting {{ display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-bottom: 1px solid var(--hairline); }}
.setting:last-child {{ border-bottom: none; }}
.setting .tile {{ flex: none; width: 26px; height: 26px; border-radius: 7px; background: var(--wash); display: flex; align-items: center; justify-content: center;
  font-size: 12px; color: color-mix(in srgb, var(--ink) 78%, transparent); }}
.setting .main {{ flex: 1; min-width: 0; }}
.setting .label {{ font-size: 13px; }}
.setting .detail, .setting .where {{ color: var(--muted); font-size: 11.5px; margin-top: 2px; }}
.setting .where {{ margin: 0 0 2px; }}
.setting code.value {{ color: var(--muted); max-width: 50%; overflow-wrap: anywhere; font-size: 12px; }}
.setting .keys {{ display: flex; gap: 4px; flex-wrap: wrap; justify-content: flex-end; }}
.settings .empty {{ color: var(--muted); padding: 12px 14px; margin: 0; }}
.settings select, .settings input.value, .settings button, .settings a.button {{ font: 12.5px "Inter", system-ui, sans-serif; color: var(--ink);
  background: var(--ground); border: 1px solid var(--hairline); border-radius: 7px; padding: 4px 9px; }}
.settings button:hover, .settings a.button:hover {{ background: var(--hover); }}
.settings input.value {{ width: 200px; flex: none; }}
kbd {{ font: 11.5px "Inter", system-ui, sans-serif; padding: 2px 7px; border-radius: 5px; border: 1px solid var(--hairline); border-bottom-width: 2px; background: var(--ground); }}
a.button {{ text-decoration: none; }}
.switch {{ position: relative; width: 34px; height: 20px; flex: none; }}
.switch input {{ opacity: 0; width: 0; height: 0; }}
.switch span {{ position: absolute; inset: 0; border-radius: 10px; background: var(--wash); box-shadow: inset 0 0 0 1px var(--hairline); transition: background .15s; cursor: pointer; }}
.switch span::after {{ content: ""; position: absolute; left: 2px; top: 2px; width: 16px; height: 16px; border-radius: 50%; background: #fff; box-shadow: 0 1px 2px rgba(0,0,0,.25); transition: transform .15s; }}
.switch input:checked + span {{ background: var(--ink); }}
.switch input:checked + span::after {{ transform: translateX(14px); background: var(--ground); }}
.toast {{ padding: 8px 12px; border-radius: 8px; background: var(--wash); font-size: 12.5px; margin: 0 0 16px; }}
}}
</style></head><body class="page-{page}">
<nav><a href="rippl://history"{h}>History</a><a href="rippl://bookmarks"{b}>Bookmarks</a><a href="rippl://archive"{a}>Archive</a><a href="rippl://passwords"{p}>Passwords</a><a href="rippl://settings"{s}>Settings</a></nav>
{body}
<script>
const TOKEN = "{token}", PAGE = "{page}";
const go = (act, arg) => {{
  const q = new URLSearchParams(location.search).get("q") || "";
  location.replace("rippl://" + PAGE + "?do=" + encodeURIComponent(act) +
    "&arg=" + encodeURIComponent(arg) + "&token=" + TOKEN + (q ? "&q=" + encodeURIComponent(q) : "") + location.hash);
}};
document.addEventListener("click", e => {{
  const b = e.target.closest("[data-do]");
  if (!b) return;
  if (b.dataset.confirm && !confirm(b.dataset.confirm)) return;
  let arg = b.dataset.arg || "";
  if (b.dataset.prompt !== undefined) {{
    const v = prompt(b.dataset.prompt, b.dataset.current || "");
    if (v === null) return;
    arg += "\n" + v;
  }}
  go(b.dataset.do, arg);
}});
document.addEventListener("change", e => {{
  const s = e.target.closest("[data-set]");
  if (s) go("set", s.dataset.set + "=" + s.value);
}});
if (new URLSearchParams(location.search).has("do")) {{
  const q = new URLSearchParams(location.search).get("q");
  history.replaceState(null, "", "rippl://" + PAGE + (q ? "?q=" + encodeURIComponent(q) : "") + location.hash);
}}
</script>
</body></html>"#,
        title = escape(title),
        h = if page == "history" { r#" class="here""# } else { "" },
        b = if page == "bookmarks" { r#" class="here""# } else { "" },
        a = if page == "archive" { r#" class="here""# } else { "" },
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

/// The tabs rippl closed by itself, newest first.
pub fn archive(places: &[Place], q: &str, closing: bool, now: i64, token: &str) -> String {
    let mut body = format!(
        r#"<h1>Archive</h1><p class="note">Tabs rippl closed because they were left alone, kept for a month. {when}</p><form action="rippl://archive"><input type="search" name="q" value="{q}" placeholder="Search the archive" autofocus></form>"#,
        q = escape(q),
        when = if closing {
            "Settings › Tabs left alone close themselves sets how long."
        } else {
            "Tabs don't close themselves now: Settings › Tabs left alone close themselves."
        },
    );
    if places.is_empty() {
        body.push_str(r#"<p class="empty">Nothing here.</p>"#);
    }
    for p in places {
        let button = format!(r#"<button data-do="forget" data-arg="{}">Forget</button>"#, escape(&p.url));
        body.push_str(&place_row(p, now, &button));
    }
    if !places.is_empty() {
        body.push_str(r#"<h2>Clear</h2><p><button class="danger" data-do="clear" data-confirm="Empty the archive? History keeps the pages.">Empty the archive</button></p>"#);
    }
    frame("archive", "Archive", token, &body)
}

/// `sources`: other browsers found, as labels ("Chrome — Default").
/// `sources`: other browsers found, as (label, whether it is of the
/// Chromium family, whose passwords can come too).
pub fn bookmarks(places: &[Place], sources: &[(String, bool)], note: &str, now: i64, token: &str) -> String {
    let mut body = String::from("<h1>Bookmarks</h1>");
    if !note.is_empty() {
        body.push_str(&format!(r#"<p class="tag">{}</p>"#, escape(note)));
    }
    if places.is_empty() {
        body.push_str(r#"<p class="empty">No bookmarks yet. Ctrl+D keeps the page you're on.</p>"#);
    }
    let mut folder: Option<&str> = None;
    for p in places {
        if folder != Some(p.folder.as_str()) {
            folder = Some(&p.folder);
            let name = if p.folder.is_empty() { "Not in a folder" } else { &p.folder };
            body.push_str(&format!("<h2>{}</h2>", escape(name)));
        }
        let buttons = format!(
            r#"<button data-do="move" data-arg="{url}" data-prompt="Folder (use / for a folder inside a folder; empty for none)" data-current="{folder}">Move…</button> <button data-do="remove" data-arg="{url}">Remove</button>"#,
            url = escape(&p.url),
            folder = escape(&p.folder),
        );
        body.push_str(&place_row(p, now, &buttons));
    }
    body.push_str(r#"<h2>Move in from another browser</h2><p class="note">Read from a copy: nothing in the other browser changes. Open tabs come in a space of their own; passwords go to your system keyring.</p>"#);
    if sources.is_empty() {
        body.push_str(r#"<p class="empty">No other browser found on this machine.</p>"#);
    }
    for (i, (label, chromium)) in sources.iter().enumerate() {
        let passwords = if *chromium {
            format!(r#"<button data-do="import-passwords" data-arg="{i}" data-confirm="Bring this browser's saved passwords into your keyring? Your keyring may ask to unlock.">Passwords</button>"#)
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<div class="row"><div class="main">{}</div><button data-do="import" data-arg="{i}">Bookmarks and history</button><button data-do="import-tabs" data-arg="{i}">Open tabs</button>{passwords}</div>"#,
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

/// One line of configuration on the settings page.
pub struct Setting {
    pub key: String,
    pub label: String,
    pub value: String,
    /// The values to pick from; empty for free text.
    pub choices: Vec<String>,
    /// Shown under it ("applies after a restart").
    pub note: String,
}

/// What the settings page shows.
pub struct SettingsView<'a> {
    pub permissions: &'a [(String, String, bool)],
    pub shield_off: &'a [String],
    pub hidden: &'a [(String, Vec<String>)],
    pub config: &'a [Setting],
    /// Each command's keys (`shortcut.<name>`), changed as the rest are.
    pub shortcuts: &'a [Setting],
    pub config_path: &'a str,
    /// The result of the last change, if any.
    pub note: &'a str,
    /// About rippl: (what, value) lines.
    pub about: &'a [(String, String)],
    /// The sections are down rippl's column, not in the page.
    pub rail_in_column: bool,
    /// Whether rippl is the default browser: "yes", "no", or "not
    /// installed".
    pub default_browser: &'a str,
}

/// rippl's logo (data/logo), for its own pages.
pub const LOGO_SVG: &str = include_str!("../../../data/logo/rippl.svg");

/// The logo as an image address its pages may show (their policy lets in
/// data: images only).
pub fn logo_uri() -> String {
    let mut out = String::from("data:image/svg+xml,");
    for b in LOGO_SVG.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b' ' | b'=' | b'/' | b':' | b',' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out.replace(' ', "%20")
}

/// The first page rippl shows: what it is, and the few
/// things worth doing first.
pub fn welcome(default_browser: &str, token: &str) -> String {
    let default = match default_browser {
        "yes" => r#"<span class="done">rippl is your default browser</span>"#.to_string(),
        "no" => r#"<button data-do="make-default">Make rippl the default</button>"#.to_string(),
        _ => r#"<span class="detail">After installing (scripts/install.sh)</span>"#.to_string(),
    };
    let step = |glyph: &str, title: &str, detail: &str, control: &str| {
        format!(
            r#"<div class="setting"><span class="tile">{glyph}</span><div class="main"><div class="label">{title}</div><div class="detail">{detail}</div></div>{control}</div>"#
        )
    };
    let mut steps = String::new();
    steps.push_str(&step("◎", "Your default browser", "Links from other apps open here", &default));
    steps.push_str(&step("⇣", "Bring things over", "Bookmarks, history, open tabs and passwords from Chrome, Firefox, Brave…", r#"<a class="button" href="rippl://bookmarks">Open…</a>"#));
    steps.push_str(&step("⛨", "Ads and trackers are blocked", "Before a page loads, with EasyList and uBlock Origin's lists. Ctrl+Shift+B turns it off for a site", r#"<span class="done">On</span>"#));
    steps.push_str(&step("☽", "Tabs sleep when you leave them", "They give their memory back and wake as they were", r#"<span class="done">On</span>"#));
    steps.push_str(&step("⚙", "Make it yours", "Tabs on top or down the side, themes, your own shortcuts", r#"<a class="button" href="rippl://settings">Settings…</a>"#));
    let keys = [
        ("Ctrl+L", "the field: an address or a search"),
        ("Ctrl+T", "a new tab"),
        ("Ctrl+Tab", "your tabs as pictures"),
        ("Ctrl+Alt+S", "two pages side by side"),
        ("Shift+click", "peek at a link"),
        ("F9", "reading mode"),
    ];
    let mut key_rows = String::new();
    for (k, what) in keys {
        key_rows.push_str(&format!(r#"<div class="key"><kbd>{k}</kbd><span>{what}</span></div>"#));
    }
    let body = format!(
        r#"<div class="welcome"><img class="logo" src="{logo}" alt=""><h1>Welcome to rippl</h1><p class="lede">A small, fast, quiet browser. Your tabs go down the side; what you aren't looking at sleeps; nothing you do leaves this computer unless you send it.</p>
<div class="card">{steps}</div><div class="keys">{key_rows}</div><p class="go"><a class="button primary" href="rippl://welcome?do=start&token={token}">Start browsing</a></p></div>"#,
        logo = logo_uri(),
        token = token.chars().filter(char::is_ascii_hexdigit).collect::<String>(),
    );
    frame("welcome", "Welcome to rippl", token, &body)
}

/// The settings' categories Settings window has them: id,
/// title, and a glyph for the sidebar.
pub const CATEGORIES: [(&str, &str, &str); 11] = [
    ("general", "General", "⚙"),
    ("appearance", "Appearance", "◐"),
    ("themes", "Themes", "▨"),
    ("tabs", "Tabs", "❐"),
    ("browsing", "Browsing", "◎"),
    ("spaces", "Spaces & Pins", "▤"),
    ("passwords", "Passwords", "⚿"),
    ("privacy", "Privacy", "⛨"),
    ("ai", "AI", "✦"),
    ("keyboard", "Keyboard & Mouse", "⌨"),
    ("about", "About", "ⓘ"),
];

/// Where a setting lives: its category and the section in it.
pub fn place(key: &str) -> (&'static str, &'static str) {
    match key {
        "webmail" => ("general", "Your browser"),
        "sync_folder" => ("general", "Sync and other devices"),
        "theme" | "dusk" => ("appearance", "Light and dark"),
        "glass" | "frame_rate" => ("appearance", "Window"),
        "motion" | "elastic" => ("appearance", "Motion"),
        "tabs" | "column" | "icons" | "bookmarks_bar" | "player" => ("appearance", "Tabs"),
        "home_theme" | "home_colour" | "home_image" | "home_grid" => ("themes", "New tab"),
        "tab_switcher" => ("tabs", "Switching"),
        "close_after" => ("tabs", "Closing"),
        "sleep" => ("tabs", "Memory"),
        "search" | "suggestions" => ("browsing", "Searching"),
        "peek" => ("browsing", "Links"),
        "autoplay" | "video_keys" | "grab" | "grab_format" => ("browsing", "Video"),
        "shield" | "ublock" => ("privacy", "Blocking"),
        "https_only" | "gpc" | "proxy" => ("privacy", "Connection"),
        "history" | "cache" => ("privacy", "History"),
        "autoscroll" => ("keyboard", "Mouse"),
        "agents" | "agents_see_tabs" => ("about", "Developer"),
        k if k.starts_with("ai_") || k.starts_with("translate") => ("ai", "On this computer"),
        _ => ("general", "Other"),
    }
}

/// `<Ctrl><Shift>t` → "Ctrl+Shift+T", as a person writes it.
pub fn key_label(accel: &str) -> String {
    let mut out = Vec::new();
    let mut rest = accel.trim();
    while let Some(r) = rest.strip_prefix('<') {
        let Some((m, after)) = r.split_once('>') else { break };
        out.push(match m.to_lowercase().as_str() {
            "control" | "ctrl" | "primary" => "Ctrl".to_string(),
            "shift" => "Shift".into(),
            "alt" | "mod1" => "Alt".into(),
            "super" | "meta" => "Super".into(),
            other => other.to_string(),
        });
        rest = after;
    }
    // Ctrl, Alt, Shift, Super: the order people say them in.
    let rank = |m: &String| ["Ctrl", "Alt", "Shift", "Super"].iter().position(|x| x == m).unwrap_or(9);
    out.sort_by_key(rank);
    let key = match rest {
        "comma" => ",".to_string(),
        "period" => ".".into(),
        "plus" => "+".into(),
        "minus" => "−".into(),
        "equal" => "=".into(),
        "bracketleft" => "[".into(),
        "bracketright" => "]".into(),
        "slash" => "/".into(),
        "Page_Up" => "Page Up".into(),
        "Page_Down" => "Page Down".into(),
        "Left" => "←".into(),
        "Right" => "→".into(),
        "Up" => "↑".into(),
        "Down" => "↓".into(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => k.to_string(),
    };
    out.push(key);
    out.join("+")
}

/// A setting's icon, in its soft tile.
fn glyph(key: &str) -> &'static str {
    match key {
        "search" | "suggestions" => "⌕",
        "theme" => "◐",
        "dusk" => "☾",
        "glass" => "◇",
        "frame_rate" => "≋",
        "motion" => "∿",
        "elastic" => "⇕",
        "tabs" | "column" => "▥",
        "icons" => "◉",
        "bookmarks_bar" => "★",
        "player" | "autoplay" => "▶",
        "video_keys" => "⏯",
        "grab" | "grab_format" => "⤓",
        "home_theme" | "home_colour" | "home_image" => "▨",
        "home_grid" => "▦",
        "tab_switcher" => "⇆",
        "close_after" => "⌫",
        "sleep" => "☽",
        "peek" => "◫",
        "shield" | "ublock" => "⛨",
        "https_only" => "🔒︎",
        "gpc" => "✋︎",
        "proxy" => "⇄",
        "history" => "↺",
        "cache" => "◔",
        "autoscroll" => "↕",
        "webmail" => "✉",
        "sync_folder" => "⟳",
        k if k.starts_with("ai") => "✦",
        "translate_to" => "文",
        k if k.starts_with("shortcut.") => "⌨",
        _ => "•",
    }
}

/// One setting's line: its icon, its words, its note, its control.
fn setting_row(s: &Setting) -> String {
    let control = if s.choices == ["on", "off"] || s.choices == ["off", "on"] {
        format!(
            r#"<label class="switch"><input type="checkbox" data-switch="{k}"{c}><span></span></label>"#,
            k = escape(&s.key),
            c = if s.value == "on" { " checked" } else { "" }
        )
    } else if s.choices.is_empty() {
        format!(r#"<input class="value" data-set="{k}" value="{v}" spellcheck="false">"#, k = escape(&s.key), v = escape(&s.value))
    } else {
        let mut options = String::new();
        let mut listed = false;
        for c in &s.choices {
            let selected = *c == s.value;
            listed |= selected;
            options.push_str(&format!(
                r#"<option value="{c}"{sel}>{c}</option>"#,
                c = escape(c),
                sel = if selected { " selected" } else { "" }
            ));
        }
        if !listed {
            options.push_str(&format!(r#"<option value="{v}" selected>{v}</option>"#, v = escape(&s.value)));
        }
        format!(r#"<select data-set="{}">{options}</select>"#, escape(&s.key))
    };
    let note = if s.note.is_empty() { String::new() } else { format!(r#"<div class="detail">{}</div>"#, escape(&s.note)) };
    format!(
        r#"<div class="setting" data-words="{words}"><span class="tile">{g}</span><div class="main"><div class="label">{label}</div>{note}</div>{control}</div>"#,
        words = escape(&format!("{} {} {}", s.label, s.key, s.note).to_lowercase()),
        label = escape(&s.label),
        g = glyph(&s.key),
    )
}

/// A shortcut's line: the command, its keys as chips, and the recorder.
fn shortcut_row(s: &Setting) -> String {
    let name = s.key.trim_start_matches("shortcut.");
    let chips: String = if s.value == "none" {
        r#"<span class="detail">No keys</span>"#.to_string()
    } else {
        s.value.split(',').map(|k| format!(r#"<kbd>{}</kbd>"#, escape(&key_label(k)))).collect()
    };
    let yours = if s.note.is_empty() { String::new() } else { format!(r#"<div class="detail">{}</div>"#, escape(&s.note)) };
    format!(
        r#"<div class="setting shortcut" data-words="{words}"><span class="tile">⌨</span><div class="main"><div class="label">{label}</div>{yours}</div><div class="keys">{chips}</div><button data-record="{name}" data-current="{cur}">Change</button><button data-record-add="{name}" data-current="{cur}" title="Another key for it">+</button>{reset}</div>"#,
        words = escape(&format!("{} {} {}", s.label, name, s.value).to_lowercase()),
        label = escape(&s.label),
        name = escape(name),
        cur = escape(if s.value == "none" { "" } else { &s.value }),
        reset = if s.note.is_empty() {
            String::new()
        } else {
            format!(r#"<button data-do="set" data-arg="shortcut.{}=" title="Its own keys back">Reset</button>"#, escape(name))
        },
    )
}

pub fn settings(v: &SettingsView, token: &str) -> String {
    // Each category's sections, in the order they first appear.
    let mut pages: Vec<(&str, Vec<(&str, String)>)> = CATEGORIES.iter().map(|(id, _, _)| (*id, Vec::new())).collect();
    let mut add = |page: &str, section: &'static str, html: String| {
        let Some((_, sections)) = pages.iter_mut().find(|(p, _)| *p == page) else { return };
        match sections.iter_mut().find(|(t, _)| *t == section) {
            Some((_, body)) => body.push_str(&html),
            None => sections.push((section, html)),
        }
    };
    for s in v.config {
        let (page, section) = place(&s.key);
        add(page, section, setting_row(s));
    }
    // General: the default browser, and what isn't a setting of its own.
    let default_control = match v.default_browser {
        "yes" => r#"<span class="detail">rippl is your default</span>"#.to_string(),
        "no" => r#"<button data-do="make-default">Make default</button>"#.to_string(),
        _ => r#"<span class="detail">Install rippl first: scripts/install.sh</span>"#.to_string(),
    };
    add(
        "general",
        "Your browser",
        format!(
            r#"<div class="setting" data-words="default browser links other apps"><span class="tile">◎</span><div class="main"><div class="label">Default browser</div><div class="detail">Links from other apps open in rippl</div></div>{default_control}</div>"#
        ),
    );
    add(
        "general",
        "Your browser",
        r#"<div class="setting" data-words="import move in bring over other browser chrome firefox brave"><span class="tile">⇣</span><div class="main"><div class="label">Bring things over</div><div class="detail">Bookmarks, history, open tabs and passwords from another browser on this computer</div></div><a class="button" href="rippl://bookmarks">Open…</a></div>"#.into(),
    );
    // Spaces & Pins: how they work (they're made in the column).
    let help = |label: &str, detail: &str| {
        format!(
            r#"<div class="setting" data-words="{w}"><span class="tile">ⓘ</span><div class="main"><div class="label">{l}</div><div class="detail">{d}</div></div></div>"#,
            w = escape(&format!("{label} {detail}").to_lowercase()),
            l = escape(label),
            d = escape(detail)
        )
    };
    add("spaces", "Spaces", help("A new space", "Ctrl+Alt+T, or + at the column's foot. Each space has its own tabs and its own sign-ins"));
    add("spaces", "Spaces", help("Go to a space", "Alt+1 to Alt+9, its button at the foot, or two fingers sideways on the column"));
    add("spaces", "Spaces", help("Name, colour, delete", "Right-click its button at the column's foot"));
    add("spaces", "Pins", help("Pin a tab", "Ctrl+Shift+P, or right-click it. Pins stay awake and never close themselves"));
    add("spaces", "Pins", help("Pin folders", "Right-click a pin: put it in a new folder, or one there is"));
    add("spaces", "Profiles", help("Another profile", "rippl --profile work: a whole separate browser, with its own everything"));
    // Passwords.
    add(
        "passwords",
        "Your passwords",
        r#"<div class="setting" data-words="passwords accounts keyring saved"><span class="tile">⚿</span><div class="main"><div class="label">Saved passwords</div><div class="detail">Kept in your system keyring (GNOME Keyring, KeePassXC), never in rippl's files</div></div><a class="button" href="rippl://passwords">Open…</a></div>"#.into(),
    );
    add("passwords", "Your passwords", help("Filling in", "Click into a sign-in box: your accounts for that site are offered under it"));
    add("passwords", "Client certificates", help("Certificates for sites that ask", "Put each one (with its key) in ~/.local/share/rippl/certificates; a site that asks lets you choose"));
    // Privacy: the lists.
    let mut lists = String::new();
    if v.shield_off.is_empty() {
        lists.push_str(r#"<p class="empty">Shield blocks on every site. Ctrl+Shift+B turns it off for the site you're on.</p>"#);
    }
    for host in v.shield_off {
        lists.push_str(&format!(
            r#"<div class="setting" data-words="shield off {h}"><span class="tile">⛨</span><div class="main"><div class="label">{h}</div><div class="detail">Shield is off here</div></div><button data-do="shield-on" data-arg="{h}">Turn back on</button></div>"#,
            h = escape(host)
        ));
    }
    add("privacy", "Sites without blocking", lists);
    let mut hidden = String::new();
    if v.hidden.is_empty() {
        hidden.push_str(r#"<p class="empty">Nothing hidden. Ctrl+Shift+H, then click what you want gone.</p>"#);
    }
    for (host, selectors) in v.hidden {
        hidden.push_str(&format!(
            r#"<div class="setting" data-words="hidden {h}"><span class="tile">◌</span><div class="main"><div class="label">{h}</div><div class="detail">{list}</div></div><button data-do="unhide" data-arg="{h}">Show again</button></div>"#,
            h = escape(host),
            list = escape(&selectors.join("  ·  ")),
        ));
    }
    add("privacy", "Hidden on sites", hidden);
    let mut perms = String::new();
    if v.permissions.is_empty() {
        perms.push_str(r#"<p class="empty">No site has asked yet.</p>"#);
    }
    for (host, kind, allow) in v.permissions {
        perms.push_str(&format!(
            r#"<div class="setting" data-words="permission {host} {kind}"><span class="tile">✓</span><div class="main"><div class="label">{host}</div><div class="detail">{kind}</div></div><span class="tag {cls}">{word}</span><button data-do="forget-permissions" data-arg="{host}">Forget</button></div>"#,
            host = escape(host),
            kind = escape(kind),
            cls = if *allow { "allow" } else { "deny" },
            word = if *allow { "allowed" } else { "denied" },
        ));
    }
    add("privacy", "Site permissions", perms);
    add(
        "privacy",
        "History",
        r#"<div class="setting" data-words="clear history forget"><span class="tile">⌫</span><div class="main"><div class="label">Clear history</div><div class="detail">Every page visited and the archive; bookmarks stay</div></div><button class="danger" data-do="clear-history" data-confirm="Forget all history? Bookmarks stay.">Clear…</button></div>"#.into(),
    );
    // Keyboard: every command.
    let mut keys = String::new();
    for s in v.shortcuts {
        keys.push_str(&shortcut_row(s));
    }
    add("keyboard", "Shortcuts", keys);
    add("keyboard", "Mouse", help("Middle click on a link", "Opens it in a new tab behind this one; on a tab, closes it"));
    add("keyboard", "Mouse", help("Shift+click on a link", "Peeks at it in a panel (Browsing › Links)"));
    // About.
    let mut about = String::new();
    for (what, value) in v.about {
        about.push_str(&format!(
            r#"<div class="setting" data-words="{w}"><span class="tile">ⓘ</span><div class="main"><div class="label">{what}</div></div><code class="value">{value}</code></div>"#,
            w = escape(&format!("{what} {value}").to_lowercase()),
            what = escape(what),
            value = escape(value)
        ));
    }
    about.push_str(&format!(
        r#"<div class="setting" data-words="config file"><span class="tile">⚙</span><div class="main"><div class="label">Settings file</div><div class="detail">Everything here is saved there as you change it; your comments stay</div></div><code class="value">{}</code></div>"#,
        escape(v.config_path)
    ));
    add("about", "rippl", about);

    // Sections in the order within each category.
    const ORDER: [&str; 28] = [
        "Your browser", "Sync and other devices", "Light and dark", "Window", "Motion", "Tabs", "New tab", "Switching", "Closing",
        "Memory", "Searching", "Links", "Video", "Spaces", "Pins", "Profiles", "Your passwords", "Client certificates",
        "Blocking", "Connection", "Sites without blocking", "Hidden on sites", "Site permissions", "History", "Shortcuts",
        "Mouse", "rippl", "Developer",
    ];
    for (_, sections) in pages.iter_mut() {
        sections.sort_by_key(|(t, _)| ORDER.iter().position(|o| o == t).unwrap_or(ORDER.len()));
    }
    // The page: the rail (Settings, Find a setting, the sections in
    // their three blocks) and the section on the right.
    let mut side = String::new();
    let mut main = String::new();
    for (n, ((id, title, glyph), (_, sections))) in CATEGORIES.iter().zip(&pages).enumerate() {
        if n == 6 || n == 10 {
            side.push_str(r#"<div class="rule"></div>"#);
        }
        side.push_str(&format!(
            r##"<a href="#{id}" data-cat="{id}"><span class="glyph">{glyph}</span><span class="name">{title}</span><span class="count"></span></a>"##
        ));
        main.push_str(&format!(r#"<section class="cat" id="cat-{id}"><header><span class="glyph">{glyph}</span>{title}</header>"#));
        if sections.is_empty() {
            main.push_str(r#"<p class="empty">Nothing to set here yet.</p>"#);
        }
        for (title, body) in sections {
            main.push_str(&format!(
                r#"<div class="group"><h2><span class="glyph">{glyph}</span>{}</h2><div class="card">{body}</div></div>"#,
                escape(title)
            ));
        }
        main.push_str("</section>");
    }
    let note = if v.note.is_empty() { String::new() } else { format!(r#"<p class="toast">{}</p>"#, escape(v.note)) };
    let early = if v.rail_in_column { "<script>document.body.classList.add('rail-in-column')</script>" } else { "" };
    let body = format!(
        r#"{early}<div class="settings"><aside><div class="rail-title">Settings</div><label class="find"><span class="lens">⌕</span><input type="search" id="find" placeholder="Find a setting" autocomplete="off"><kbd class="hint">Ctrl+F</kbd></label>{side}</aside><main><div class="column">{note}<section class="cat" id="cat-found"><header id="found-title">Found</header><div class="card" id="found"></div></section>{main}</div></main></div>
<script>{SETTINGS_SCRIPT}</script>"#
    );
    frame("settings", "Settings", token, &body)
}

/// The settings page's own behaviour: categories, search, switches, and
/// the shortcut recorder.
const SETTINGS_SCRIPT: &str = r##"
const cats = [...document.querySelectorAll("section.cat")];
const links = [...document.querySelectorAll("aside a")];
const find = document.getElementById("find");
const show = (id) => {
  cats.forEach(c => c.hidden = c.id !== "cat-" + id);
  links.forEach(a => a.classList.toggle("here", a.dataset.cat === id));
};
const route = () => show((location.hash || "#general").slice(1));
addEventListener("hashchange", route);
route();
// Escape empties the find field; Ctrl+F (rippl sends it here) comes to it.
find.addEventListener("keydown", e => { if (e.key === "Escape") { find.value = ""; find.dispatchEvent(new Event("input")); } });
window.__ripplFindSetting = () => { find.focus(); find.select(); };
find.addEventListener("input", () => {
  const q = find.value.trim().toLowerCase();
  const found = document.getElementById("found");
  found.innerHTML = "";
  links.forEach(a => { a.querySelector(".count").textContent = ""; a.classList.remove("none"); });
  if (!q) { route(); return; }
  document.getElementById("found-title").textContent = "Settings matching \u201c" + find.value.trim() + "\u201d";
  for (const c of cats) {
    if (c.id === "cat-found") continue;
    let count = 0;
    const link = links.find(a => "cat-" + a.dataset.cat === c.id);
    for (const row of c.querySelectorAll(".setting")) {
      if (!row.dataset.words.includes(q)) continue;
      count++;
      const copy = row.cloneNode(true);
      const where = document.createElement("div");
      where.className = "where";
      where.textContent = c.querySelector("header").lastChild.textContent + " › " + row.closest(".group").querySelector("h2").lastChild.textContent;
      copy.querySelector(".main").prepend(where);
      found.append(copy);
    }
    if (link) {
      link.querySelector(".count").textContent = count ? count : "";
      link.classList.toggle("none", !count);
    }
  }
  if (!found.children.length) found.innerHTML = '<p class="empty">No setting matches.</p>';
  cats.forEach(c => c.hidden = c.id !== "cat-found");
  links.forEach(a => a.classList.remove("here"));
});
document.addEventListener("change", e => {
  const s = e.target.closest("[data-switch]");
  if (s) go("set", s.dataset.switch + "=" + (s.checked ? "on" : "off"));
});
// The recorder: rippl's own shortcuts step aside while it listens.
const tell = (act) => fetch("rippl://settings?do=" + act + "&arg=&token=" + TOKEN).catch(() => {});
const keyName = (e) => {
  const named = { " ": "space", ",": "comma", ".": "period", "/": "slash", "[": "bracketleft", "]": "bracketright",
    "-": "minus", "=": "equal", "+": "plus", ";": "semicolon", "'": "apostrophe", "`": "grave", "\\": "backslash",
    ArrowLeft: "Left", ArrowRight: "Right", ArrowUp: "Up", ArrowDown: "Down", Enter: "Return", Backspace: "BackSpace",
    PageUp: "Page_Up", PageDown: "Page_Down", Delete: "Delete", Home: "Home", End: "End", Insert: "Insert", Tab: "Tab" };
  if (named[e.key]) return named[e.key];
  if (/^F\d{1,2}$/.test(e.key)) return e.key;
  if (e.code && e.code.startsWith("Key")) return e.code.slice(3).toLowerCase();
  if (e.code && e.code.startsWith("Digit")) return e.code.slice(5);
  return e.key.length === 1 ? e.key.toLowerCase() : null;
};
let recording = null;
document.addEventListener("click", e => {
  const b = e.target.closest("[data-record], [data-record-add]");
  if (!b) return;
  if (recording) recording.button.textContent = recording.text;
  recording = { name: b.dataset.record || b.dataset.recordAdd, add: !!b.dataset.recordAdd, current: b.dataset.current, button: b, text: b.textContent };
  b.textContent = "Press the keys… (Esc stops)";
  tell("record-start");
}, true);
document.addEventListener("keydown", e => {
  if (!recording) return;
  e.preventDefault(); e.stopPropagation();
  if (["Control", "Shift", "Alt", "Meta", "Super", "OS"].includes(e.key)) return;
  const r = recording;
  if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey) {
    recording = null; r.button.textContent = r.text; tell("record-stop"); return;
  }
  const key = keyName(e);
  if (!key) return;
  const accel = (e.ctrlKey ? "<Ctrl>" : "") + (e.shiftKey ? "<Shift>" : "") + (e.altKey ? "<Alt>" : "") + (e.metaKey ? "<Super>" : "") + key;
  recording = null;
  const value = r.add && r.current ? r.current + ", " + accel : accel;
  go("set", "shortcut." + r.name + "=" + value);
}, true);
"##;

/// HTTPS-only: the secure address didn't work. Offers the insecure one,
/// plainly, as a choice. No script.
pub fn https_failed(host: &str, http_url: &str, reason: &str) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<title>No secure connection</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ font: 16px/1.5 system-ui, sans-serif; max-width: 560px; margin: 12vh auto; padding: 0 24px; }}
h1 {{ font-size: 22px; }} .why {{ opacity: .6; font-size: 14px; }}
a.go {{ display: inline-block; margin-top: 16px; padding: 8px 14px; border-radius: 8px; border: 1px solid #8886; color: inherit; text-decoration: none; }}
</style></head><body>
<h1>{host} doesn't offer a secure connection</h1>
<p>rippl tried <b>https://{host}</b> and it didn't work. The site may only work over plain http, where anyone on your network can see and change what you send and receive.</p>
<p class="why">{reason}</p>
<a class="go" href="{url}">Continue to http://{host} anyway</a>
<p class="why">Allowed for {host} until you quit rippl. HTTPS-only can be turned off in rippl://settings.</p>
</body></html>"#,
        host = escape(host),
        url = escape(http_url),
        reason = escape(reason),
    )
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

    #[test]
    fn keys_as_people_write_them() {
        assert_eq!(key_label("<Shift><Control>o"), "Ctrl+Shift+O");
        assert_eq!(key_label("<Alt>Left"), "Alt+←");
        assert_eq!(key_label("<Ctrl>comma"), "Ctrl+,");
        assert_eq!(key_label("F5"), "F5");
    }

    #[test]
    fn every_setting_has_a_place() {
        for key in ["theme", "proxy", "sleep", "home_theme", "ai_model", "autoscroll"] {
            assert_ne!(super::place(key).1, "Other", "{key}");
        }
    }

    fn place(url: &str, title: &str) -> Place {
        Place { url: url.into(), title: title.into(), visits: 1, ..Default::default() }
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
