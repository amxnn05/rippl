//! Keeping the HTTP cache under a size (`cache` in the config). WebKit's
//! own limit grows with the free disk space, up to gigabytes; rippl
//! checks at launch and every six hours, and when the cache is over the
//! cap, empties the sites taking the most room until it is under three
//! quarters of it. Many small sites stay cached; one video site that
//! filled it goes.

/// The choices offered, and what each holds in megabytes.
pub const CHOICES: [&str; 5] = ["128m", "256m", "512m", "1g", "2g"];

/// The default: half a gigabyte.
pub const DEFAULT_MB: u64 = 512;

/// A setting's value in megabytes: `128m`, `512m`, `1g`, `2g`, or a plain
/// number of megabytes (32 at least).
pub fn megabytes(value: &str) -> Option<u64> {
    let v = value.trim().to_ascii_lowercase();
    let mb = if let Some(g) = v.strip_suffix('g') {
        g.trim().parse::<u64>().ok()?.checked_mul(1024)?
    } else {
        v.strip_suffix('m').unwrap_or(&v).trim().parse::<u64>().ok()?
    };
    (32..=1 << 20).contains(&mb).then_some(mb)
}

/// The setting's name for `mb`: `1g` for 1024, `512m` for 512.
pub fn name(mb: u64) -> String {
    if mb.is_multiple_of(1024) { format!("{}g", mb / 1024) } else { format!("{mb}m") }
}

/// Which sites to empty, as indices into `sizes` (bytes per site): none
/// while the whole is within `cap` bytes; otherwise the biggest first,
/// until what is left is within three quarters of it.
pub fn to_empty(sizes: &[u64], cap: u64) -> Vec<usize> {
    let mut total: u64 = sizes.iter().sum();
    if total <= cap {
        return Vec::new();
    }
    let goal = cap / 4 * 3;
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sizes[i]));
    let mut out = Vec::new();
    for i in order {
        if total <= goal {
            break;
        }
        total -= sizes[i];
        out.push(i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_read() {
        assert_eq!(megabytes("512m"), Some(512));
        assert_eq!(megabytes("1g"), Some(1024));
        assert_eq!(megabytes("300"), Some(300));
        assert_eq!(megabytes("2G"), Some(2048));
        assert_eq!(megabytes("1m"), None, "too small to be useful");
        assert_eq!(megabytes("lots"), None);
        assert_eq!(name(1024), "1g");
        assert_eq!(name(512), "512m");
        assert!(CHOICES.iter().all(|c| megabytes(c).is_some_and(|mb| name(mb) == *c)));
    }

    #[test]
    fn the_biggest_go_first_down_to_three_quarters() {
        const MB: u64 = 1 << 20;
        // Under the cap: nothing.
        assert!(to_empty(&[100 * MB, 200 * MB], 512 * MB).is_empty());
        // 700 MB, cap 512: the 400 MB site goes (300 left ≤ 384).
        assert_eq!(to_empty(&[100 * MB, 400 * MB, 200 * MB], 512 * MB), vec![1]);
        // Evenly spread: the biggest few until under 384 MB.
        let even = vec![60 * MB; 10];
        assert_eq!(to_empty(&even, 512 * MB).len(), 4);
        assert!(to_empty(&[], 512 * MB).is_empty());
    }
}
