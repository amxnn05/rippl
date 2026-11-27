//! Downloading videos with yt-dlp (Ctrl+Shift+Y, `grab.rs`): which pasted
//! text counts as a link, the arguments rippl runs yt-dlp with, and reading
//! the lines it prints back.
//!
//! rippl never runs a shell: the arguments are a list, the link comes after
//! `--` (so a pasted "link" can't be an option), yt-dlp's own config files
//! are ignored, and it never updates itself. It gets no cookies of rippl's.

use std::path::{Path, PathBuf};

use crate::address::Proxy;

/// What to keep of a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// The best video with its sound, as an `.mp4`.
    Video,
    /// Only the sound, as an `.mp3`.
    Audio,
}

impl Format {
    pub fn key(self) -> &'static str {
        match self {
            Format::Video => "video",
            Format::Audio => "audio",
        }
    }

    pub fn from_key(key: &str) -> Option<Format> {
        match key {
            "video" => Some(Format::Video),
            "audio" => Some(Format::Audio),
            _ => None,
        }
    }

    /// The other one: what Shift+Return asks for.
    pub fn other(self) -> Format {
        match self {
            Format::Video => Format::Audio,
            Format::Audio => Format::Video,
        }
    }
}

/// Pasted text as a link yt-dlp may be given: `https://…` or `http://…`,
/// or an address without its scheme (`youtu.be/abc`), which gets https.
/// Anything else (no host, spaces, control characters, `file:`, a lone
/// word) is refused.
pub fn link(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.len() > 4096 || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let full = if lower.starts_with("https://") || lower.starts_with("http://") {
        text.to_string()
    } else if lower.contains("://") || lower.starts_with('-') {
        return None;
    } else {
        format!("https://{text}")
    };
    let after = &full[full.find("://")? + 3..];
    let host = after.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(""); // no user:pass@ in the host
    let name = host.trim_start_matches('[').split(']').next().unwrap_or("");
    let name = if host.starts_with('[') { name } else { host.split(':').next().unwrap_or("") };
    let ok_host = !name.is_empty()
        && (host.starts_with('[') || name.contains('.') || name == "localhost")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':') || !c.is_ascii());
    ok_host.then_some(full)
}

/// How video formats are ranked: resolution, then frame rate, then HDR.
pub const BEST: &str = "res,fps,hdr:12";

/// How rippl names the file: the title (cut to 150 bytes) and the video's
/// id, so two videos with one title don't land on the same name.
pub const NAME: &str = "%(title).150B [%(id)s].%(ext)s";

/// The arguments for one download into `dir`. `ffmpeg` says whether it's
/// installed: without it, nothing can be merged or converted, so the best
/// single file is taken instead.
pub fn args(url: &str, format: Format, dir: &Path, proxy: &Proxy, ffmpeg: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ignore-config",
        "--no-update",
        "--no-playlist",
        "--color",
        "never",
        "--quiet",
        "--no-warnings",
        "--progress",
        "--newline",
        "--progress-template",
        PROGRESS,
        "--print",
        "before_dl:rippl-title %(title)s",
        "--print",
        "post_process:rippl-finishing",
        "--print",
        "after_move:rippl-file %(filepath)s",
        "--no-mtime",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push("--paths".into());
    a.push(dir.to_string_lossy().into_owned());
    a.push("--output".into());
    a.push(NAME.into());
    // The highest resolution first, then the highest frame rate, then HDR;
    // the rest is yt-dlp's own order (the better codec, then bitrate). The
    // container never decides: a 1080p60 VP9 or AV1 stream beats a 1080p30
    // H.264 one, and ffmpeg puts any of them in the .mp4.
    let fmt: &[&str] = match (format, ffmpeg) {
        (Format::Video, true) => &["--format", "bv*+ba/b", "--format-sort", BEST, "--merge-output-format", "mp4"],
        (Format::Video, false) => &["--format", "b", "--format-sort", BEST],
        (Format::Audio, true) => &["--format", "ba/b", "--extract-audio", "--audio-format", "mp3", "--audio-quality", "0"],
        (Format::Audio, false) => &["--format", "ba[ext=m4a]/ba/b"],
    };
    a.extend(fmt.iter().map(|s| s.to_string()));
    match proxy {
        Proxy::System => {}
        Proxy::Direct => a.extend(["--proxy".to_string(), String::new()]),
        Proxy::Through(uri) => a.extend(["--proxy".to_string(), uri.clone()]),
    }
    a.push("--".into());
    a.push(url.into());
    a
}

/// One progress line: bytes so far, bytes in all (or the estimate), speed
/// in bytes a second, seconds left, then the file being written (last, as
/// it may have spaces). yt-dlp writes `NA` for what it doesn't know.
const PROGRESS: &str = "download:rippl-progress %(progress.downloaded_bytes)s %(progress.total_bytes)s \
%(progress.total_bytes_estimate)s %(progress.speed)s %(progress.eta)s %(progress.tmpfilename)s";

/// What a line from yt-dlp says.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// The video's title, before its download begins.
    Title(String),
    Progress(Progress),
    /// Downloaded; merging or converting now.
    Finishing,
    /// Done, at this path.
    File(PathBuf),
    /// An error yt-dlp gave up with (on its error stream).
    Error(String),
    Other,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    pub got: u64,
    pub total: Option<u64>,
    pub speed: Option<f64>,
    pub eta: Option<u64>,
    /// The `.part` file it is writing, for removing on cancel.
    pub part: Option<PathBuf>,
}

impl Progress {
    /// 0.0 to 1.0, when the size is known.
    pub fn fraction(&self) -> Option<f64> {
        self.total.filter(|t| *t > 0).map(|t| (self.got as f64 / t as f64).clamp(0.0, 1.0))
    }
}

pub fn parse(line: &str) -> Line {
    let line = line.trim_end_matches(['\r', '\n']);
    if let Some(rest) = line.strip_prefix("rippl-title ") {
        return Line::Title(rest.trim().to_string());
    }
    if line.trim() == "rippl-finishing" {
        return Line::Finishing;
    }
    if let Some(rest) = line.strip_prefix("rippl-file ") {
        return Line::File(PathBuf::from(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("rippl-progress ") {
        let mut parts = rest.splitn(6, ' ');
        let mut next = || parts.next().unwrap_or("NA");
        let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0);
        let got = num(next()).map_or(0, |v| v as u64);
        let total = num(next()).map(|v| v as u64);
        let estimate = num(next()).map(|v| v as u64);
        let speed = num(next());
        let eta = num(next()).map(|v| v as u64);
        let part = Some(next().trim()).filter(|p| !p.is_empty() && *p != "NA").map(PathBuf::from);
        return Line::Progress(Progress { got, total: total.or(estimate), speed, eta, part });
    }
    if let Some(rest) = line.strip_prefix("ERROR:") {
        return Line::Error(rest.trim().to_string());
    }
    Line::Other
}

/// yt-dlp's error, shortened to what a person needs: no "[youtube] abc:"
/// prefix, and no advice about updating it or reporting bugs.
pub fn short_error(error: &str) -> String {
    let mut e = error.trim();
    if e.starts_with('[')
        && let Some(i) = e.find("] ")
    {
        e = &e[i + 2..];
        if let Some(i) = e.find(": ")
            && !e[..i].contains(' ')
        {
            e = &e[i + 2..];
        }
    }
    let e = e.split(". Please report").next().unwrap_or(e);
    let e = e.split("; please report").next().unwrap_or(e);
    let e = e.split(". Confirm you are on the latest").next().unwrap_or(e);
    let mut out: String = e.trim().chars().take(160).collect();
    if out.is_empty() {
        out = "yt-dlp couldn't download it".into();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        assert_eq!(link(" https://www.youtube.com/watch?v=abc \n").as_deref(), Some("https://www.youtube.com/watch?v=abc"));
        assert_eq!(link("http://example.com/v.mp4").as_deref(), Some("http://example.com/v.mp4"));
        assert_eq!(link("youtu.be/abc").as_deref(), Some("https://youtu.be/abc"));
        assert_eq!(link("HTTPS://Vimeo.com/1").as_deref(), Some("HTTPS://Vimeo.com/1"));
        assert_eq!(link("http://localhost:8000/clip.webm").as_deref(), Some("http://localhost:8000/clip.webm"));
        assert_eq!(link("http://127.0.0.1:8000/a").as_deref(), Some("http://127.0.0.1:8000/a"));
        assert_eq!(link("http://[::1]:8000/a").as_deref(), Some("http://[::1]:8000/a"));
        for bad in [
            "",
            "hello",
            "two words.com",
            "--exec rm",
            "-o x",
            "file:///etc/passwd",
            "ftp://example.com/a",
            "javascript:alert(1)",
            "https://",
            "https:///path",
            "https://exa mple.com",
            "https://a.com/\u{0}",
        ] {
            assert_eq!(link(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn link_is_last_after_double_dash() {
        let a = args("https://a.com/v", Format::Video, Path::new("/home/me/Downloads"), &Proxy::System, true);
        let n = a.len();
        assert_eq!(&a[n - 2..], ["--", "https://a.com/v"]);
        assert!(a.contains(&"--ignore-config".to_string()));
        assert!(a.contains(&"--no-update".to_string()));
        assert!(!a.iter().any(|x| x == "--exec" || x.starts_with("--cookies")));
        let paths = a.iter().position(|x| x == "--paths").unwrap();
        assert_eq!(a[paths + 1], "/home/me/Downloads");
    }

    #[test]
    fn formats() {
        let dir = Path::new("/d");
        let has = |a: &[String], s: &str| a.iter().any(|x| x == s);
        let v = args("https://a.com", Format::Video, dir, &Proxy::System, true);
        assert!(has(&v, "--merge-output-format") && has(&v, "mp4") && has(&v, "bv*+ba/b"));
        // Frame rate right after resolution; the container never ranks.
        let sort = &v[v.iter().position(|x| x == "--format-sort").unwrap() + 1];
        assert!(sort.starts_with("res,fps") && !sort.contains("ext"));
        let v = args("https://a.com", Format::Video, dir, &Proxy::System, false);
        assert!(!has(&v, "--merge-output-format") && has(&v, "b"));
        let a = args("https://a.com", Format::Audio, dir, &Proxy::System, true);
        assert!(has(&a, "--extract-audio") && has(&a, "mp3"));
        let a = args("https://a.com", Format::Audio, dir, &Proxy::System, false);
        assert!(!has(&a, "--extract-audio"));
        assert_eq!(Format::Video.other(), Format::Audio);
        assert_eq!(Format::from_key("audio"), Some(Format::Audio));
    }

    #[test]
    fn proxies() {
        let dir = Path::new("/d");
        let at = |a: &[String]| a.iter().position(|x| x == "--proxy").map(|i| a[i + 1].clone());
        assert_eq!(at(&args("https://a.com", Format::Video, dir, &Proxy::System, true)), None);
        assert_eq!(at(&args("https://a.com", Format::Video, dir, &Proxy::Direct, true)), Some(String::new()));
        let through = Proxy::Through("socks5://127.0.0.1:9050".into());
        assert_eq!(at(&args("https://a.com", Format::Video, dir, &through, true)).as_deref(), Some("socks5://127.0.0.1:9050"));
    }

    #[test]
    fn lines() {
        assert_eq!(parse("rippl-title A video: part 1\n"), Line::Title("A video: part 1".into()));
        assert_eq!(parse("rippl-finishing"), Line::Finishing);
        assert_eq!(parse("rippl-file /home/me/Downloads/A [x].mp4"), Line::File("/home/me/Downloads/A [x].mp4".into()));
        let Line::Progress(p) = parse("rippl-progress 1048576 4194304 NA 524288.5 6 /d/A video [x].f137.mp4.part") else {
            panic!()
        };
        assert_eq!(p.got, 1_048_576);
        assert_eq!(p.total, Some(4_194_304));
        assert_eq!(p.speed, Some(524_288.5));
        assert_eq!(p.eta, Some(6));
        assert_eq!(p.part.as_deref(), Some(Path::new("/d/A video [x].f137.mp4.part")));
        assert_eq!(p.fraction(), Some(0.25));
        // Only an estimate, and nothing else known.
        let Line::Progress(p) = parse("rippl-progress 10 NA 40.0 NA NA NA") else { panic!() };
        assert_eq!((p.total, p.speed, p.eta, p.part), (Some(40), None, None, None));
        let Line::Progress(p) = parse("rippl-progress NA NA NA NA NA NA") else { panic!() };
        assert_eq!((p.got, p.fraction()), (0, None));
        assert_eq!(parse("ERROR: [youtube] abc: Video unavailable"), Line::Error("[youtube] abc: Video unavailable".into()));
        assert_eq!(parse("[download] something else"), Line::Other);
    }

    #[test]
    fn errors_short() {
        assert_eq!(short_error("[youtube] abc123: Video unavailable"), "Video unavailable");
        assert_eq!(
            short_error("[generic] Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404>)"),
            "Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404>)"
        );
        assert_eq!(
            short_error("Unsupported URL: https://a.com. Please report this issue on https://github.com/yt-dlp"),
            "Unsupported URL: https://a.com"
        );
        assert_eq!(short_error(""), "yt-dlp couldn't download it");
    }
}
