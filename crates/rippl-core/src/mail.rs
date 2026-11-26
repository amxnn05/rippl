//! Email links in a webmail: a `mailto:` link
//! opens the chosen webmail's compose page in a tab, the draft filled in,
//! instead of the desktop's mail app. `webmail = none` (the default) keeps
//! the mail app.
//!
//! A mailto is read as RFC 6068 has it: addresses before the `?`, comma
//! separated, then `to`, `cc`, `bcc`, `subject` and `body`, every part
//! percent-encoded; `+` is a plus (user+tag@example.com), never a space.
//! Every value going into the compose address is percent-encoded down to
//! the unreserved characters, so nothing in one field can start another.

/// The webmails offered, by the name the config takes.
pub const WEBMAILS: [&str; 5] = ["none", "gmail", "outlook", "fastmail", "proton"];

/// Most kept of each: past these a link is stuffed, not written by hand.
const MOST_RECIPIENTS: usize = 50;
const MOST_SUBJECT: usize = 500;
const MOST_LENGTH: usize = 8000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
}

/// %XX decoded as UTF-8; anything not valid kept as it came.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// All but the unreserved characters (and `keep`) percent-encoded.
fn encode(text: &str, keep: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "-._~".contains(c) || (c.is_ascii() && keep.contains(c)) {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn addresses(text: &str) -> Vec<String> {
    decode(text)
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty() && !a.chars().any(char::is_control))
        .map(String::from)
        .collect()
}

impl Draft {
    /// A `mailto:` link read; `None` for anything else.
    pub fn read(link: &str) -> Option<Draft> {
        if link.len() < 7 || !link[..7].eq_ignore_ascii_case("mailto:") {
            return None;
        }
        let rest = &link[7..];
        let rest = rest.split('#').next().unwrap_or("");
        let (head, fields) = rest.split_once('?').unwrap_or((rest, ""));
        let mut d = Draft { to: addresses(head), ..Draft::default() };
        let (mut subject, mut body) = (None, None);
        for field in fields.split('&').filter(|f| !f.is_empty()) {
            let (name, value) = field.split_once('=').unwrap_or((field, ""));
            match decode(name).to_lowercase().as_str() {
                "to" => d.to.extend(addresses(value)),
                "cc" => d.cc.extend(addresses(value)),
                "bcc" => d.bcc.extend(addresses(value)),
                "subject" if subject.is_none() => subject = Some(decode(value)),
                "body" if body.is_none() => body = Some(decode(value)),
                _ => {}
            }
        }
        d.to.truncate(MOST_RECIPIENTS);
        d.cc.truncate(MOST_RECIPIENTS);
        d.bcc.truncate(MOST_RECIPIENTS);
        let subject = subject.unwrap_or_default().replace("\r\n", "\n");
        d.subject = subject.lines().collect::<Vec<_>>().join(" ").chars().take(MOST_SUBJECT).collect();
        d.body = body.unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n");
        Some(d)
    }

    /// The draft as a clean mailto again (Proton takes it whole).
    pub fn mailto(&self) -> String {
        let mut fields = Vec::new();
        if !self.cc.is_empty() {
            fields.push(format!("cc={}", encode(&self.cc.join(","), "@,+")));
        }
        if !self.bcc.is_empty() {
            fields.push(format!("bcc={}", encode(&self.bcc.join(","), "@,+")));
        }
        if !self.subject.is_empty() {
            fields.push(format!("subject={}", encode(&self.subject, "")));
        }
        if !self.body.is_empty() {
            fields.push(format!("body={}", encode(&self.body.replace('\n', "\r\n"), "")));
        }
        let mut out = format!("mailto:{}", encode(&self.to.join(","), "@,+"));
        if !fields.is_empty() {
            out.push('?');
            out.push_str(&fields.join("&"));
        }
        out
    }

    fn compose_exactly(&self, webmail: &str) -> Option<String> {
        let (to, cc, bcc) = (self.to.join(","), self.cc.join(","), self.bcc.join(","));
        let query = |pairs: &[(&str, &str)]| -> String {
            pairs.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}={}", encode(v, ""))).collect::<Vec<_>>().join("&")
        };
        let join = |base: &str, sep: &str, rest: String| if rest.is_empty() { base.to_string() } else { format!("{base}{sep}{rest}") };
        Some(match webmail {
            "gmail" => join(
                "https://mail.google.com/mail/?view=cm&fs=1",
                "&",
                query(&[("to", &to), ("cc", &cc), ("bcc", &bcc), ("su", &self.subject), ("body", &self.body)]),
            ),
            "outlook" => join(
                "https://outlook.live.com/mail/0/deeplink/compose",
                "?",
                query(&[("to", &to), ("cc", &cc), ("bcc", &bcc), ("subject", &self.subject), ("body", &self.body)]),
            ),
            "fastmail" => join(
                "https://app.fastmail.com/mail/compose",
                "?",
                query(&[("to", &to), ("cc", &cc), ("bcc", &bcc), ("subject", &self.subject), ("body", &self.body)]),
            ),
            "proton" => format!("https://mail.proton.me/inbox#mailto={}", encode(&self.mailto(), "")),
            _ => return None,
        })
    }

    /// `webmail`'s compose page with the draft in it, its body shortened
    /// if it must be to keep the address under 8,000 bytes; `None` for
    /// `none`.
    pub fn compose(&self, webmail: &str) -> Option<String> {
        let full = self.compose_exactly(webmail)?;
        if full.len() <= MOST_LENGTH {
            return Some(full);
        }
        let chars: Vec<char> = self.body.chars().collect();
        let (mut low, mut high) = (0usize, chars.len());
        let mut best = Draft { body: String::new(), ..self.clone() };
        while low <= high {
            let mid = (low + high) / 2;
            let trial = Draft { body: chars[..mid].iter().collect(), ..self.clone() };
            if trial.compose_exactly(webmail).is_some_and(|u| u.len() <= MOST_LENGTH) {
                best = trial;
                low = mid + 1;
            } else if mid == 0 {
                break;
            } else {
                high = mid - 1;
            }
        }
        best.compose_exactly(webmail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_mailto() {
        let d = Draft::read("MAILTO:a@x.com,b%2Btag@y.org?cc=c@z.net&subject=Hi%20there%0D%0Aagain&body=Line1%0D%0ALine2&subject=no&x=1#frag").unwrap();
        assert_eq!(d.to, vec!["a@x.com", "b+tag@y.org"]);
        assert_eq!(d.cc, vec!["c@z.net"]);
        assert_eq!(d.subject, "Hi there again");
        assert_eq!(d.body, "Line1\nLine2");
        assert!(Draft::read("https://x.com").is_none());
    }

    #[test]
    fn composes_without_letting_fields_escape() {
        let d = Draft { to: vec!["a@x.com".into()], subject: "1&2=3 #4".into(), body: "hi".into(), ..Draft::default() };
        assert_eq!(
            d.compose("gmail").unwrap(),
            "https://mail.google.com/mail/?view=cm&fs=1&to=a%40x.com&su=1%262%3D3%20%234&body=hi"
        );
        assert!(d.compose("outlook").unwrap().starts_with("https://outlook.live.com/mail/0/deeplink/compose?to="));
        assert!(d.compose("proton").unwrap().starts_with("https://mail.proton.me/inbox#mailto=mailto%3Aa%40x.com"));
        assert_eq!(d.compose("none"), None);
        let long = Draft { body: "é".repeat(5000), ..d };
        let url = long.compose("fastmail").unwrap();
        assert!(url.len() <= 8000 && url.contains("body="), "a long body is shortened, not dropped");
    }
}
