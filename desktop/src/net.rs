//! The few requests rippl makes itself (Shield's lists, search
//! suggestions when you allow them): no cookies, a generic user agent, and
//! nothing about you.

use std::io::Read;

const AGENT: &str = "Mozilla/5.0 rippl";

/// A text body, at most `limit` bytes; None on any failure.
pub fn get_text(url: &str, limit: u64) -> Option<String> {
    let mut response = ureq::get(url).header("User-Agent", AGENT).call().ok()?;
    let mut body = String::new();
    response.body_mut().as_reader().take(limit).read_to_string(&mut body).ok()?;
    Some(body)
}
