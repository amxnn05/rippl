//! Videos through yt-dlp on every system (Ctrl+Shift+Y): the same
//! arguments and progress lines as the GTK build (`rippl_core::ytdlp`).
//!
//! yt-dlp is a program of its own, run beside rippl and ended with it. If
//! it isn't on the system, rippl fetches yt-dlp's official standalone build
//! for this system from its GitHub releases the first time it's needed, into
//! rippl's data folder (`tools/`), and lets it update itself after that
//! (`yt-dlp -U`, at most once a week). With ffmpeg on the system, videos
//! come as the best video and sound merged into an .mp4; without it, as the
//! best single file.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

use rippl_core::ytdlp::{self, Format, Line};

use crate::browser::{Browser, Ev, Proxy, post};

#[cfg(target_os = "macos")]
const ASSET: &str = "yt-dlp_macos";
#[cfg(windows)]
const ASSET: &str = "yt-dlp.exe";
#[cfg(all(unix, not(target_os = "macos")))]
const ASSET: &str = "yt-dlp_linux";

const EXE: &str = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };

fn tools() -> PathBuf {
    rippl_core::dirs::data().join("tools")
}

/// A program on PATH.
fn on_path(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) && !name.ends_with(".exe") { format!("{name}.exe") } else { name.to_string() };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(&exe)).find(|p| p.is_file())
}

/// yt-dlp: rippl's own copy, else the system's.
pub fn ytdlp() -> Option<PathBuf> {
    let mine = tools().join(EXE);
    if mine.is_file() {
        return Some(mine);
    }
    on_path("yt-dlp")
}

pub fn ffmpeg() -> bool {
    if cfg!(not(all(unix, not(target_os = "macos")))) && !helpers().is_empty() && !tools().join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }).is_file() && on_path("ffmpeg").is_none() {
        return false;
    }
    tools().join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }).is_file() || on_path("ffmpeg").is_some()
}

/// Fetches yt-dlp's standalone build (blocking; run on a thread).
fn fetch_ytdlp() -> Result<PathBuf, String> {
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{ASSET}");
    let dir = tools();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{EXE}.part"));
    let mut response = ureq::get(&url).header("User-Agent", "rippl").call().map_err(|e| format!("couldn't download yt-dlp: {e}"))?;
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    std::io::copy(&mut response.body_mut().as_reader(), &mut file).map_err(|e| format!("couldn't download yt-dlp: {e}"))?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755));
    }
    let done = dir.join(EXE);
    std::fs::rename(&part, &done).map_err(|e| e.to_string())?;
    Ok(done)
}

/// Where this system's ffmpeg and Deno come from, zipped: (what, address,
/// the program's name inside).
fn helpers() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        out.push(("ffmpeg", "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip", "ffmpeg.exe"));
        out.push(("Deno", "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip", "deno.exe"));
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        out.push(("ffmpeg", "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip", "ffmpeg"));
        out.push(("Deno", "https://github.com/denoland/deno/releases/latest/download/deno-aarch64-apple-darwin.zip", "deno"));
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        out.push(("ffmpeg", "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffmpeg.zip", "ffmpeg"));
        out.push(("Deno", "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-apple-darwin.zip", "deno"));
    }
    out
}

/// A helper (ffmpeg, Deno) from its zip into tools/, unless the system
/// has it.
fn ensure_helper(what: &str, url: &str, inside: &str) -> Result<(), String> {
    let name = inside.trim_end_matches(".exe");
    if tools().join(inside).is_file() || on_path(name).is_some() {
        return Ok(());
    }
    let dir = tools();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip_path = dir.join(format!("{name}.zip.part"));
    let mut response = ureq::get(url).header("User-Agent", "rippl").call().map_err(|e| format!("couldn't download {what}: {e}"))?;
    let mut file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
    std::io::copy(&mut response.body_mut().with_config().limit(400 * 1024 * 1024).reader(), &mut file)
        .map_err(|e| format!("couldn't download {what}: {e}"))?;
    drop(file);
    let result = (|| {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let base = entry.name().rsplit('/').next().unwrap_or("").to_string();
            if base == inside && entry.is_file() {
                let part = dir.join(format!("{inside}.part"));
                let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
                drop(out);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755));
                }
                std::fs::rename(&part, dir.join(inside)).map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        Err(format!("{what} wasn't in its download"))
    })();
    let _ = std::fs::remove_file(&zip_path);
    result
}

/// rippl's own copy, kept current: `yt-dlp -U` when a week old.
fn maybe_update(path: &PathBuf) {
    if !path.starts_with(tools()) {
        return;
    }
    let old = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age > Duration::from_secs(7 * 24 * 3600));
    if old {
        let mut c = Command::new(path);
        c.arg("-U").stdout(Stdio::null()).stderr(Stdio::null());
        hide_window(&mut c);
        if c.status().is_ok() {
            // Touched even if it was current, so this waits another week.
            let _ = std::fs::File::options().append(true).open(path).and_then(|f| f.set_modified(SystemTime::now()));
        }
    }
}

fn hide_window(c: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let _ = c;
}

/// What a running yt-dlp reports back.
pub enum GrabEv {
    Ready(Result<PathBuf, String>),
    Line(Line),
    Exit(bool),
}

/// Starts yt-dlp (fetching it first, if need be) for `url`, on threads;
/// lines come back as `Ev::Grab(id, …)`.
pub fn start(id: u64, url: String, format: Format, proxy: Proxy, children: std::sync::Arc<std::sync::Mutex<Vec<(u64, Child)>>>) {
    std::thread::spawn(move || {
        let exe = match ytdlp() {
            Some(p) => {
                maybe_update(&p);
                p
            }
            None => {
                post(&proxy, Ev::Grab(id, GrabEv::Line(Line::Title("Getting yt-dlp (once)…".into()))));
                match fetch_ytdlp() {
                    Ok(p) => p,
                    Err(e) => {
                        post(&proxy, Ev::Grab(id, GrabEv::Ready(Err(e))));
                        return;
                    }
                }
            }
        };
        // ffmpeg (to join picture and sound) and Deno (yt-dlp needs a
        // JavaScript runtime for YouTube), once, where the system has none.
        for (what, url, inside) in helpers() {
            let name = inside.trim_end_matches(".exe");
            if tools().join(inside).is_file() || on_path(name).is_some() {
                continue;
            }
            post(&proxy, Ev::Grab(id, GrabEv::Line(Line::Title(format!("Getting {what} (once)…")))));
            if let Err(e) = ensure_helper(what, url, inside) {
                eprintln!("rippl: {e}");
            }
        }
        let dir = rippl_core::dirs::downloads();
        let _ = std::fs::create_dir_all(&dir);
        let args = ytdlp::args(&url, format, &dir, &rippl_core::address::Proxy::System, ffmpeg());
        let mut c = Command::new(&exe);
        c.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(t) = tools().to_str() {
            // rippl's own ffmpeg, if one is put there, is found first.
            let path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths = vec![PathBuf::from(t)];
            paths.extend(std::env::split_paths(&path));
            if let Ok(p) = std::env::join_paths(paths) {
                c.env("PATH", p);
            }
        }
        hide_window(&mut c);
        let mut child = match c.spawn() {
            Ok(ch) => ch,
            Err(e) => {
                post(&proxy, Ev::Grab(id, GrabEv::Ready(Err(format!("couldn't run yt-dlp: {e}")))));
                return;
            }
        };
        post(&proxy, Ev::Grab(id, GrabEv::Ready(Ok(exe))));
        let out = child.stdout.take();
        let err = child.stderr.take();
        if let Ok(mut list) = children.lock() {
            list.push((id, child));
        }
        let p2 = proxy.clone();
        let errs = std::thread::spawn(move || {
            if let Some(e) = err {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    if line.contains("ERROR") {
                        post(&p2, Ev::Grab(id, GrabEv::Line(Line::Error(ytdlp::short_error(&line)))));
                    }
                }
            }
        });
        if let Some(o) = out {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                let parsed = ytdlp::parse(&line);
                if parsed != Line::Other {
                    post(&proxy, Ev::Grab(id, GrabEv::Line(parsed)));
                }
            }
        }
        let _ = errs.join();
        let ok = match children.lock() {
            Ok(mut list) => {
                let at = list.iter().position(|(i, _)| *i == id);
                match at {
                    Some(i) => list.remove(i).1.wait().map(|s| s.success()).unwrap_or(false),
                    None => false,
                }
            }
            Err(_) => false,
        };
        post(&proxy, Ev::Grab(id, GrabEv::Exit(ok)));
    });
}

impl Browser {
    /// Ctrl+Shift+Y: the bar for a link, filled with this page's address
    /// when it is one.
    pub fn open_grab_bar(&self) {
        let url = self.front_url();
        let fill = if url.starts_with("http") { url } else { String::new() };
        self.eval_chrome_pub(&format!("rippl.grab({})", serde_json::to_string(&fill).unwrap_or_default()));
    }

    /// Return in the bar: the video, or (Shift) only its sound.
    pub fn grab(&self, text: &str, audio: bool) {
        let Some(url) = ytdlp::link(text) else {
            self.notice("That isn't a link yt-dlp can take");
            return;
        };
        let format = if audio { Format::Audio } else { Format::Video };
        let private = self.front().is_some_and(|id| self.is_private(id));
        let id = self.downloads.grab_started(&url, format, private);
        start(id, url, format, self.proxy.clone(), self.children.clone());
        self.notice(if ffmpeg() { "Downloading the best quality there is" } else { "Downloading (with ffmpeg installed, videos come at their very best)" });
        self.render_due.set(true);
    }

    pub fn grab_event(&self, id: u64, ev: GrabEv) {
        match ev {
            GrabEv::Ready(Ok(_)) => {}
            GrabEv::Ready(Err(why)) => {
                self.downloads.grab_ended(id, false, &why);
                self.notice(&why);
            }
            GrabEv::Line(line) => self.downloads.grab_line(id, line),
            GrabEv::Exit(ok) => {
                if let Some(name) = self.downloads.grab_ended(id, ok, "") {
                    self.notice(&if ok { format!("Downloaded {name}") } else { format!("{name} didn't finish") });
                }
            }
        }
        self.render_due.set(true);
    }

    /// yt-dlp stops when rippl does.
    pub fn stop_grabs(&self) {
        if let Ok(mut list) = self.children.lock() {
            for (_, c) in list.iter_mut() {
                let _ = c.kill();
            }
        }
    }

    pub fn cancel_grab(&self, id: u64) {
        if let Ok(mut list) = self.children.lock()
            && let Some((_, c)) = list.iter_mut().find(|(i, _)| *i == id)
        {
            let _ = c.kill();
        }
    }
}
