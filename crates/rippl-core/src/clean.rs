//! Tracking IDs taken off addresses.
//!
//! - **Click IDs** (`fbclid`, `gclid`, `msclkid`, …) are put on links by ad
//!   and social networks so the site you land on can tie the visit to the
//!   click, and to you. They do nothing for the page. rippl's page-process
//!   extension strips them from every request before it leaves (the same
//!   list as Brave's, which it gets from WebKit's on the Mac).
//! - **Campaign tags** (`utm_source`, …) only say where a link was shared.
//!   They're left on requests, since some sites need them, but taken off
//!   when you copy an address (Ctrl+Shift+C) Copy Clean Link.

/// Identify one click and, through it, one person.
const CLICK_IDS: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "gbraid", "wbraid", "dclid", "msclkid", "mc_eid", "igshid", "yclid",
    "twclid", "ttclid", "li_fat_id", "_hsenc", "_hsmi", "mkt_tok", "oly_enc_id", "oly_anon_id",
    "vero_id", "rb_clickid", "s_cid", "_openstat", "srsltid",
];

fn campaign(key: &str) -> bool {
    key.starts_with("utm_") || matches!(key, "ref_src" | "ref_url")
}

/// `url` without the query parameters `drop` picks. `None` when nothing was
/// dropped (so callers leave the request alone).
fn without(url: &str, drop: impl Fn(&str) -> bool) -> Option<String> {
    let (base, rest) = url.split_once('?')?;
    let (query, fragment) = match rest.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (rest, None),
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("");
            !pair.is_empty() && !drop(&key.to_ascii_lowercase())
        })
        .collect();
    if kept.len() == query.split('&').filter(|p| !p.is_empty()).count() {
        return None;
    }
    let mut out = base.to_string();
    if !kept.is_empty() {
        out.push('?');
        out.push_str(&kept.join("&"));
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    Some(out)
}

/// The address without click IDs, if it had any.
pub fn strip_click_ids(url: &str) -> Option<String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    without(url, |k| CLICK_IDS.contains(&k))
}

/// The address as worth sharing: no click IDs, no campaign tags.
pub fn clean_link(url: &str) -> String {
    without(url, |k| CLICK_IDS.contains(&k) || campaign(k)).unwrap_or_else(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_ids_go_the_rest_stays() {
        assert_eq!(
            strip_click_ids("https://shop.com/item?id=7&fbclid=abc&color=red#top").as_deref(),
            Some("https://shop.com/item?id=7&color=red#top")
        );
        assert_eq!(strip_click_ids("https://a.com/?gclid=1").as_deref(), Some("https://a.com/"));
        assert_eq!(strip_click_ids("https://a.com/?q=fbclid"), None, "a value, not a key");
        assert_eq!(strip_click_ids("https://a.com/?page=2"), None);
        assert_eq!(strip_click_ids("https://a.com/path"), None);
        assert_eq!(strip_click_ids("data:text/html,?fbclid=1"), None);
    }

    #[test]
    fn clean_links_also_drop_campaigns() {
        assert_eq!(clean_link("https://a.com/post?utm_source=x&utm_medium=y&id=3"), "https://a.com/post?id=3");
        assert_eq!(clean_link("https://a.com/post"), "https://a.com/post");
    }
}
