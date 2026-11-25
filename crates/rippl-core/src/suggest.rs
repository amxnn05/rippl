//! What the address field offers as you type: places ranked by how well
//! they match and how much you use them, and the inline completion of an
//! address you have been to ("you" → "youtube.com" with "tube.com" selected,
//! so Backspace removes it and Return accepts it).
//!
//! Nothing here touches the network: suggestions come only from your own
//! history, bookmarks and open tabs. Nothing typed is sent anywhere until
//! Return.

use crate::places::Place;

/// The URL without its scheme and a leading `www.`: what people type.
pub fn bare(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.strip_prefix("www.").unwrap_or(rest)
}

/// How much a place deserves to be shown for `typed`. Zero: not at all.
pub fn score(typed: &str, place: &Place, now: i64) -> f64 {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return 0.0;
    }
    let url = bare(&place.url).to_lowercase();
    let title = place.title.to_lowercase();
    let words: Vec<&str> = typed.split_whitespace().collect();
    if !words.iter().all(|w| url.contains(w) || title.contains(w)) {
        return 0.0;
    }
    // Frecency: use, fading with time (half weight after a week, a third
    // after two).
    let days = ((now - place.last).max(0) as f64) / 86_400.0;
    let mut s = (place.visits.max(1) as f64) / (1.0 + days / 7.0);
    let host = url.split('/').next().unwrap_or("");
    if host.starts_with(&typed) {
        s *= 10.0; // typing the start of a site
    } else if url.starts_with(&typed) {
        s *= 5.0;
    } else if host.split(['.', '-']).any(|part| part.starts_with(&typed)) {
        s *= 3.0; // "tube" for youtube.com is weaker than "you"
    } else if title.split(|c: char| !c.is_alphanumeric()).any(|w| words.iter().any(|t| w.starts_with(t))) {
        s *= 2.0;
    }
    if place.bookmarked {
        s *= 2.0;
    }
    // The site's front page before its deep pages.
    s / (1.0 + url.len() as f64 / 200.0)
}

/// `places` ranked for `typed`, best first, at most `limit`.
pub fn rank(typed: &str, places: Vec<Place>, now: i64, limit: usize) -> Vec<Place> {
    let mut scored: Vec<(f64, Place)> =
        places.into_iter().map(|p| (score(typed, &p, now), p)).filter(|(s, _)| *s > 0.0).collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().take(limit).map(|(_, p)| p).collect()
}

/// What `typed` completes to inline, from the best suggestions: a host
/// (or the URL, when it was typed with a path) that starts with exactly
/// what was typed. `None` when typing has a space, or nothing starts so.
pub fn complete(typed: &str, ranked: &[Place]) -> Option<String> {
    if typed.is_empty() || typed.contains(char::is_whitespace) {
        return None;
    }
    let lower = typed.to_lowercase();
    for place in ranked {
        let bare = bare(&place.url);
        let host = bare.split('/').next().unwrap_or(bare);
        // Up to the host only, unless a path is being typed: completing
        // "git" to a deep GitHub page would be a surprise.
        let candidate = if typed.contains('/') {
            bare.trim_end_matches('/')
        } else {
            host
        };
        if candidate.to_lowercase().starts_with(&lower) && candidate.len() > typed.len() {
            // Keep exactly what was typed (its case), add the rest.
            return Some(format!("{typed}{}", &candidate[typed.len()..]));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(url: &str, title: &str, visits: i64, days_ago: i64) -> Place {
        Place { url: url.into(), title: title.into(), visits, last: 1_000_000 - days_ago * 86_400, ..Default::default() }
    }

    const NOW: i64 = 1_000_000;

    #[test]
    fn site_start_beats_more_visits_elsewhere() {
        let list = vec![
            place("https://www.example.org/youtube-review", "A review", 30, 0),
            place("https://www.youtube.com/", "YouTube", 5, 0),
        ];
        let ranked = rank("you", list, NOW, 10);
        assert_eq!(ranked[0].url, "https://www.youtube.com/");
    }

    #[test]
    fn recent_beats_old() {
        let list = vec![place("https://a.com/x", "Rust thing", 10, 60), place("https://b.com/y", "Rust other", 10, 0)];
        assert_eq!(rank("rust", list, NOW, 10)[0].url, "https://b.com/y");
    }

    #[test]
    fn no_match_no_row() {
        let list = vec![place("https://a.com/", "Alpha", 1, 0)];
        assert!(rank("zzz", list, NOW, 10).is_empty());
    }

    #[test]
    fn inline_completion() {
        let list = vec![place("https://www.youtube.com/watch?v=1", "", 3, 0), place("https://github.com/rust-lang/rust", "", 1, 0)];
        assert_eq!(complete("you", &list).as_deref(), Some("youtube.com"));
        assert_eq!(complete("YouT", &list).as_deref(), Some("YouTube.com"));
        assert_eq!(complete("github.com/rust", &list).as_deref(), Some("github.com/rust-lang/rust"));
        assert_eq!(complete("youtube.com", &list), None, "nothing left to add");
        assert_eq!(complete("you tube", &list), None);
        assert_eq!(complete("zz", &list), None);
    }
}
