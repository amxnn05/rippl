//! How pressed the machine is for memory, and handing freed memory back.
//!
//! Linux PSI (`/proc/pressure/memory`, kernel 4.20+) says what share of the
//! last ten seconds tasks spent stalled waiting for memory. That is a much
//! better signal than "free RAM", which Linux keeps near zero on purpose
//! (the page cache uses it).

use rippl_core::tabs::Pressure;

/// The current pressure. `None` when PSI is not available.
pub fn read() -> Pressure {
    let Ok(text) = std::fs::read_to_string("/proc/pressure/memory") else {
        return Pressure::None;
    };
    parse(&text)
}

fn avg10(line: &str) -> f64 {
    line.split_whitespace()
        .find_map(|kv| kv.strip_prefix("avg10="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

fn parse(text: &str) -> Pressure {
    let some = text.lines().find(|l| l.starts_with("some")).map_or(0.0, avg10);
    let full = text.lines().find(|l| l.starts_with("full")).map_or(0.0, avg10);
    // "full": every task stalled at once. Even a few percent of that is a
    // machine that is swapping hard.
    if full >= 5.0 || some >= 40.0 {
        Pressure::Critical
    } else if some >= 10.0 {
        Pressure::Some
    } else {
        Pressure::None
    }
}

/// Gives the heap pages glibc is holding freed back to the kernel. Cheap
/// when there is little to return; worth it after the window loses focus.
pub fn trim() {
    // SAFETY: malloc_trim only walks glibc's own free lists.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        let calm = "some avg10=0.00 avg60=0.00 avg300=0.00 total=1\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n";
        let busy = "some avg10=12.50 avg60=3.00 avg300=1.00 total=1\nfull avg10=1.00 avg60=0.00 avg300=0.00 total=0\n";
        let bad = "some avg10=20.00 avg60=3.00 avg300=1.00 total=1\nfull avg10=7.00 avg60=0.00 avg300=0.00 total=0\n";
        assert_eq!(parse(calm), Pressure::None);
        assert_eq!(parse(busy), Pressure::Some);
        assert_eq!(parse(bad), Pressure::Critical);
    }
}
