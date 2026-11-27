//! Downloads: saved to your Downloads folder under the site's name for the
//! file (cleaned, and never overwriting: `report (1).pdf`), and listed at
//! the foot of the column while rippl runs. Click a finished one to open
//! it; × cancels a running one or takes a finished one off the list.
//!
//! Pause and resume: WebKitGTK can't pause a
//! download, so pausing keeps what has come so far (a hard link to WebKit's
//! `.wkdownload` file, taken before the download is stopped, so nothing is
//! copied) as `<name>.part`; resuming asks the server for the rest with an
//! HTTP Range request, with the tab's own cookies and user agent, and adds
//! it on. A server that won't answer a range sends the whole file again,
//! which then starts over. The proxy setting applies.
//!
//! Nothing about downloads is kept after quitting: the files are in your
//! Downloads folder, and that is the record (a `.part` left there can be
//! deleted).
//!
//! Videos (Ctrl+Shift+Y, grab.rs) come down through yt-dlp, run as a
//! process of its own, and are listed here the same way: its progress lines
//! fill the row, pause stops it and keeps its `.part` files, and resuming
//! runs it again, which carries on from them. It ends when rippl does.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use rippl_core::ytdlp::{self, Format, Line};
use webkit6::prelude::*;
use webkit6::{gio, glib, gtk, Download};

struct Item {
    download: Download,
    row: gtk::Box,
    name: String,
    state: State,
    /// Where it comes from, the store its cookies are in, and the user
    /// agent it was asked with: what resuming needs.
    url: String,
    session: Option<webkit6::NetworkSession>,
    agent: String,
    bar: gtk::ProgressBar,
    status: gtk::Label,
    pause: gtk::Button,
    /// The file it ends up as, and the bytes it will have.
    dest: PathBuf,
    total: u64,
}

#[derive(Clone, Debug)]
enum State {
    Running,
    /// Kept so far in this `.part` file.
    Paused(PathBuf),
    /// Fetching the rest of the `.part` file; cancel to pause again.
    Resuming(PathBuf, gio::Cancellable),
    Done(PathBuf),
    Failed(String),
}

impl State {
    fn running(&self) -> bool {
        matches!(self, State::Running)
    }
}

/// A video coming down through yt-dlp.
struct Grab {
    id: u32,
    url: String,
    format: Format,
    name: String,
    state: GrabState,
    process: Option<gio::Subprocess>,
    /// Every `.part` file it has written, for removing on cancel.
    parts: Vec<PathBuf>,
    /// Where it landed, once yt-dlp says.
    file: Option<PathBuf>,
    /// yt-dlp's last error line.
    error: String,
    fraction: f64,
    widgets: Row,
}

#[derive(Clone, Debug, PartialEq)]
enum GrabState {
    Running,
    Paused,
    Done(PathBuf),
    Failed(String),
}

/// One row's widgets: its name, pause and ×, its bar, its status line.
#[derive(Clone)]
struct Row {
    row: gtk::Box,
    label: gtk::Label,
    pause: gtk::Button,
    close: gtk::Button,
    bar: gtk::ProgressBar,
    status: gtk::Label,
}

pub struct Downloads {
    /// The column's foot: a heading and the rows. Hidden while empty.
    pub area: gtk::Box,
    /// The store whose cookies a download with no tab resumes with.
    pub session: RefCell<Option<webkit6::NetworkSession>>,
    rows: gtk::Box,
    items: RefCell<Vec<Item>>,
    grabs: RefCell<Vec<Grab>>,
    next_grab: Cell<u32>,
    me: RefCell<Weak<Downloads>>,
}

/// `~/Downloads`, or where the user's XDG settings put it.
fn folder() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Downloads).unwrap_or_else(|| glib::home_dir().join("Downloads"))
}

impl Downloads {
    pub fn new() -> Rc<Downloads> {
        let area = gtk::Box::new(gtk::Orientation::Vertical, 2);
        area.add_css_class("downloads");
        let heading = gtk::Label::builder().label("Downloads").xalign(0.0).build();
        heading.add_css_class("heading");
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 2);
        area.append(&heading);
        area.append(&rows);
        area.set_visible(false);
        let downloads =
            Rc::new(Downloads {
            area,
            session: RefCell::new(None),
            rows,
            items: RefCell::new(Vec::new()),
            grabs: RefCell::new(Vec::new()),
            next_grab: Cell::new(1),
            me: RefCell::new(Weak::new()),
        });
        *downloads.me.borrow_mut() = Rc::downgrade(&downloads);
        downloads
    }

    /// A new row at the top of the list, saying `label`.
    fn new_row(&self, label: &str) -> Row {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("download");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let label = gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(1)
            .build();
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.add_css_class("close");
        close.set_tooltip_text(Some("Cancel or remove"));
        let pause = gtk::Button::from_icon_name("media-playback-pause-symbolic");
        pause.add_css_class("flat");
        pause.add_css_class("close");
        pause.set_tooltip_text(Some("Pause"));
        line.append(&label);
        line.append(&pause);
        line.append(&close);
        let bar = gtk::ProgressBar::new();
        let status = gtk::Label::builder().xalign(0.0).build();
        status.add_css_class("dim");
        row.append(&line);
        row.append(&bar);
        row.append(&status);
        self.rows.prepend(&row);
        self.area.set_visible(true);
        Row { row, label, pause, close, bar, status }
    }

    /// The list shows while anything is in it.
    fn refresh_visible(&self) {
        let any = !self.items.borrow().is_empty() || !self.grabs.borrow().is_empty();
        self.area.set_visible(any);
    }

    /// A download began, from any tab or session.
    pub fn started(&self, download: &Download) {
        let Row { row, label, pause, close, bar, status } = self.new_row("Starting…");
        let view = download.web_view();
        self.items.borrow_mut().push(Item {
            download: download.clone(),
            row: row.clone(),
            name: String::new(),
            state: State::Running,
            url: download.request().and_then(|r| r.uri()).map(|u| u.to_string()).unwrap_or_default(),
            session: view.as_ref().and_then(|v| v.network_session()).or_else(|| self.session.borrow().clone()),
            agent: view.and_then(|v| WebViewExt::settings(&v)).and_then(|s| s.user_agent()).map(|u| u.to_string()).unwrap_or_default(),
            bar: bar.clone(),
            status: status.clone(),
            pause: pause.clone(),
            dest: PathBuf::new(),
            total: 0,
        });
        let weak = self.me.borrow().clone();
        let d = download.clone();
        pause.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.pause_or_resume(&d);
            }
        });

        // Where it goes: decided by rippl, not left to the page.
        let weak = self.me.borrow().clone();
        let name_label = label.clone();
        download.connect_decide_destination(move |d, suggested| {
            let dir = folder();
            let _ = std::fs::create_dir_all(&dir);
            let path = rippl_core::downloads::destination(&dir, suggested, |p| p.exists());
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            name_label.set_label(&name);
            if let Some(this) = weak.upgrade()
                && let Some(item) = this.items.borrow_mut().iter_mut().find(|i| &i.download == d) {
                    item.name = name;
                    item.dest = path.clone();
                }
            d.set_destination(&path.to_string_lossy());
            true
        });

        let progress = bar.clone();
        let progress_status = status.clone();
        download.connect_received_data(move |d, _| {
            progress.set_fraction(d.estimated_progress());
            let got = d.received_data_length();
            let total = d.response().map_or(0, |r| r.content_length());
            progress_status.set_label(&if total > 0 {
                format!("{} of {}", size(got), size(total))
            } else {
                size(got)
            });
        });

        let weak = self.me.borrow().clone();
        let (done_bar, done_status) = (bar.clone(), status.clone());
        download.connect_finished(move |d| {
            let Some(this) = weak.upgrade() else { return };
            let mut items = this.items.borrow_mut();
            let Some(item) = items.iter_mut().find(|i| &i.download == d) else { return };
            if !item.state.running() {
                return; // failed or paused first; "finished" follows those
            }
            let path = d.destination().map(|p| PathBuf::from(p.as_str())).unwrap_or_default();
            done_bar.set_visible(false);
            done_status.set_label("Done. Click to open");
            item.pause.set_visible(false);
            item.state = State::Done(path);
        });

        let weak = self.me.borrow().clone();
        download.connect_failed(move |d, error| {
            let Some(this) = weak.upgrade() else { return };
            let mut items = this.items.borrow_mut();
            let Some(item) = items.iter_mut().find(|i| &i.download == d) else { return };
            // Stopped to pause: what came is kept, and that's no failure.
            if !item.state.running() {
                return;
            }
            item.pause.set_visible(false);
            let why = if error.matches(webkit6::DownloadError::CancelledByUser) {
                "Cancelled".to_string()
            } else {
                error.message().to_string()
            };
            bar.set_visible(false);
            status.set_label(&why);
            item.state = State::Failed(why);
        });

        // Click a finished download: open it with its default app.
        let click = gtk::GestureClick::new();
        let weak = self.me.borrow().clone();
        let d = download.clone();
        click.connect_released(move |_, _, _, _| {
            let Some(this) = weak.upgrade() else { return };
            let path = this.items.borrow().iter().find(|i| i.download == d).and_then(|i| match &i.state {
                State::Done(p) => Some(p.clone()),
                _ => None,
            });
            if let Some(path) = path {
                let uri = gio::File::for_path(&path).uri();
                if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
                    eprintln!("rippl: could not open {}: {e}", path.display());
                }
            }
        });
        row.add_controller(click);

        let weak = self.me.borrow().clone();
        let d = download.clone();
        close.connect_clicked(move |_| {
            let Some(this) = weak.upgrade() else { return };
            let state = this.items.borrow().iter().find(|i| i.download == d).map(|i| i.state.clone());
            match state {
                Some(State::Running) => d.cancel(),
                Some(State::Resuming(part, c)) => {
                    c.cancel();
                    let _ = std::fs::remove_file(part);
                    this.remove(&d);
                }
                Some(State::Paused(part)) => {
                    let _ = std::fs::remove_file(part);
                    this.remove(&d);
                }
                _ => this.remove(&d),
            }
        });
    }

    /// The row's pause button: pause a running download, resume a paused
    /// one, pause a resuming one again.
    pub fn pause_or_resume(&self, d: &Download) {
        let state = self.items.borrow().iter().find(|i| &i.download == d).map(|i| i.state.clone());
        match state {
            Some(State::Running) => self.pause(d),
            Some(State::Paused(part)) => self.resume(d, part),
            Some(State::Resuming(part, c)) => {
                c.cancel();
                self.set_paused(d, part);
            }
            _ => {}
        }
    }

    /// For the control socket: the `n`th download's pause button, counted
    /// as `status` lists them.
    pub fn press_pause(&self, n: usize) {
        let d = self.items.borrow().get(n).map(|i| i.download.clone());
        if let Some(d) = d {
            self.pause_or_resume(&d);
            return;
        }
        let webkit = self.items.borrow().len();
        let grab = n.checked_sub(webkit).and_then(|k| self.grabs.borrow().get(k).map(|g| g.id));
        if let Some(id) = grab {
            self.grab_pause_or_resume(id);
        }
    }

    /// For the control socket: the `n`th download's ×, counted as `status` lists them.
    pub fn press_close(&self, n: usize) {
        let webkit = self.items.borrow().len();
        // The id first, its borrow over, then the × (which borrows again).
        let id = n.checked_sub(webkit).and_then(|k| self.grabs.borrow().get(k).map(|g| g.id));
        if let Some(id) = id {
            self.grab_close(id);
        }
    }

    /// Keeps what came, then stops WebKit's download.
    fn pause(&self, d: &Download) {
        let Some(dest) = d.destination().map(|p| PathBuf::from(p.as_str())) else { return };
        let wk = PathBuf::from(format!("{}.wkdownload", dest.display()));
        let part = PathBuf::from(format!("{}.part", dest.display()));
        let _ = std::fs::remove_file(&part);
        let kept = std::fs::hard_link(&wk, &part).or_else(|_| std::fs::copy(&wk, &part).map(|_| ()));
        if kept.is_err() {
            if let Some(item) = self.items.borrow().iter().find(|i| &i.download == d) {
                item.status.set_label("This download can't be paused");
            }
            return;
        }
        if let Some(item) = self.items.borrow_mut().iter_mut().find(|i| &i.download == d) {
            item.dest = dest;
            item.total = d.response().map_or(0, |r| r.content_length());
            item.state = State::Paused(part.clone());
        }
        d.cancel();
        self.set_paused(d, part);
    }

    fn set_paused(&self, d: &Download, part: PathBuf) {
        let mut items = self.items.borrow_mut();
        let Some(item) = items.iter_mut().find(|i| &i.download == d) else { return };
        let got = std::fs::metadata(&part).map_or(0, |m| m.len());
        item.status.set_label(&if item.total > 0 {
            format!("Paused · {} of {}", size(got), size(item.total))
        } else {
            format!("Paused · {}", size(got))
        });
        item.pause.set_icon_name("media-playback-start-symbolic");
        item.pause.set_tooltip_text(Some("Resume"));
        item.state = State::Paused(part);
    }

    /// Fetches the rest of `part` and finishes it.
    fn resume(&self, d: &Download, part: PathBuf) {
        let (url, session, agent, dest) = {
            let items = self.items.borrow();
            let Some(item) = items.iter().find(|i| &i.download == d) else { return };
            (item.url.clone(), item.session.clone(), item.agent.clone(), item.dest.clone())
        };
        let cancel = gio::Cancellable::new();
        if let Some(item) = self.items.borrow_mut().iter_mut().find(|i| &i.download == d) {
            item.state = State::Resuming(part.clone(), cancel.clone());
            item.pause.set_icon_name("media-playback-pause-symbolic");
            item.pause.set_tooltip_text(Some("Pause"));
            item.bar.set_visible(true);
            item.status.set_label("Resuming…");
        }
        let weak = self.me.borrow().clone();
        let d = d.clone();
        glib::MainContext::default().spawn_local(async move {
            let result = fetch_rest(&url, session, &agent, &part, &cancel, |got, total| {
                if let Some(this) = weak.upgrade()
                    && let Some(item) = this.items.borrow_mut().iter_mut().find(|i| i.download == d)
                {
                    item.total = total;
                    item.bar.set_fraction(if total > 0 { got as f64 / total as f64 } else { 0.0 });
                    item.status.set_label(&if total > 0 { format!("{} of {}", size(got), size(total)) } else { size(got) });
                }
            })
            .await;
            let Some(this) = weak.upgrade() else { return };
            match result {
                Ok(()) => {
                    // Into its place (the name may have been taken since).
                    let final_path = if dest.exists() && std::fs::metadata(&dest).is_ok_and(|m| m.len() > 0) {
                        let dir = dest.parent().map(PathBuf::from).unwrap_or_else(folder);
                        let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        rippl_core::downloads::destination(&dir, &name, |p| p.exists())
                    } else {
                        dest.clone()
                    };
                    let moved = std::fs::rename(&part, &final_path);
                    let mut items = this.items.borrow_mut();
                    let Some(item) = items.iter_mut().find(|i| i.download == d) else { return };
                    item.bar.set_visible(false);
                    item.pause.set_visible(false);
                    match moved {
                        Ok(()) => {
                            item.status.set_label("Done. Click to open");
                            item.state = State::Done(final_path);
                        }
                        Err(e) => {
                            item.status.set_label(&e.to_string());
                            item.state = State::Failed(e.to_string());
                        }
                    }
                }
                Err(_) if cancel.is_cancelled() => {} // paused again, or taken away
                Err(e) => {
                    let mut items = this.items.borrow_mut();
                    if let Some(item) = items.iter_mut().find(|i| i.download == d) {
                        item.status.set_label(&format!("Couldn't resume: {e} · press ▶ to try again"));
                        item.pause.set_icon_name("media-playback-start-symbolic");
                        item.state = State::Paused(part);
                    }
                }
            }
        });
    }

    fn remove(&self, download: &Download) {
        let removed = {
            let mut items = self.items.borrow_mut();
            items.iter().position(|i| &i.download == download).map(|i| items.remove(i).row)
        };
        if let Some(row) = removed {
            self.rows.remove(&row);
        }
        self.refresh_visible();
    }

    /// One line per download, for the control socket: WebKit's first, then
    /// yt-dlp's (the order `press_pause` counts in).
    pub fn status(&self) -> String {
        let items = self.items.borrow();
        let grabs = self.grabs.borrow();
        if items.is_empty() && grabs.is_empty() {
            return "none".into();
        }
        let webkit = items.iter().map(|i| match &i.state {
            State::Running => format!("running\t{}\t{:.0}%", i.name, i.download.estimated_progress() * 100.0),
            State::Paused(p) => format!("paused\t{}\t{} bytes", i.name, std::fs::metadata(p).map_or(0, |m| m.len())),
            State::Resuming(p, _) => format!("resuming\t{}\t{} bytes", i.name, std::fs::metadata(p).map_or(0, |m| m.len())),
            State::Done(p) => format!("done\t{}\t{}", i.name, p.display()),
            State::Failed(why) => format!("failed\t{}\t{why}", i.name),
        });
        let videos = grabs.iter().map(|g| {
            let name = if g.name.is_empty() { &g.url } else { &g.name };
            match &g.state {
                GrabState::Running => format!("running\t{name}\t{:.0}%\tyt-dlp {}", g.fraction * 100.0, g.format.key()),
                GrabState::Paused => format!("paused\t{name}\t{:.0}%\tyt-dlp {}", g.fraction * 100.0, g.format.key()),
                GrabState::Done(p) => format!("done\t{name}\t{}", p.display()),
                GrabState::Failed(why) => format!("failed\t{name}\t{why}"),
            }
        });
        webkit.chain(videos).collect::<Vec<_>>().join("\n")
    }

    // ---- videos, through yt-dlp ----------------------------------------

    /// Starts downloading `url` with yt-dlp. The error says what is missing.
    pub fn grab(&self, url: &str, format: Format) -> Result<(), String> {
        if glib::find_program_in_path("yt-dlp").is_none() {
            return Err("yt-dlp isn't installed (on Arch: sudo pacman -S yt-dlp)".into());
        }
        let widgets = self.new_row(url);
        widgets.status.set_label("Starting…");
        let id = self.next_grab.get();
        self.next_grab.set(id + 1);
        self.grabs.borrow_mut().push(Grab {
            id,
            url: url.to_string(),
            format,
            name: String::new(),
            state: GrabState::Running,
            process: None,
            parts: Vec::new(),
            file: None,
            error: String::new(),
            fraction: 0.0,
            widgets: widgets.clone(),
        });

        let weak = self.me.borrow().clone();
        widgets.pause.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.grab_pause_or_resume(id);
            }
        });
        let weak = self.me.borrow().clone();
        widgets.close.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.grab_close(id);
            }
        });
        // Click a finished one: open it (or the folder, if yt-dlp didn't say where).
        let click = gtk::GestureClick::new();
        let weak = self.me.borrow().clone();
        click.connect_released(move |_, _, _, _| {
            let Some(this) = weak.upgrade() else { return };
            let done = this.grabs.borrow().iter().find(|g| g.id == id).and_then(|g| match &g.state {
                GrabState::Done(p) => Some(p.clone()),
                _ => None,
            });
            if let Some(path) = done {
                let uri = gio::File::for_path(&path).uri();
                if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
                    eprintln!("rippl: could not open {}: {e}", path.display());
                }
            }
        });
        widgets.row.add_controller(click);

        self.run_grab(id);
        Ok(())
    }

    /// Runs yt-dlp for grab `id` (again, when resuming: it carries on from
    /// its `.part` files) and follows what it prints.
    fn run_grab(&self, id: u32) {
        let Some((url, format, w)) =
            self.grabs.borrow().iter().find(|g| g.id == id).map(|g| (g.url.clone(), g.format, g.widgets.clone()))
        else {
            return;
        };
        let dir = folder();
        let _ = std::fs::create_dir_all(&dir);
        let ffmpeg = glib::find_program_in_path("ffmpeg").is_some();
        let mut argv: Vec<OsString> = vec!["yt-dlp".into()];
        argv.extend(ytdlp::args(&url, format, &dir, &crate::engine::proxy_setting(), ffmpeg).into_iter().map(OsString::from));
        let argv: Vec<&std::ffi::OsStr> = argv.iter().map(OsString::as_os_str).collect();

        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE);
        // If rippl ends, so does yt-dlp: nothing keeps downloading unseen.
        launcher.set_child_setup(|| unsafe {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
        });
        let process = match launcher.spawn(&argv) {
            Ok(p) => p,
            Err(e) => {
                self.grab_failed(id, &format!("Couldn't run yt-dlp: {e}"));
                return;
            }
        };
        if let Some(g) = self.grabs.borrow_mut().iter_mut().find(|g| g.id == id) {
            g.process = Some(process.clone());
            g.state = GrabState::Running;
            g.error.clear();
        }
        w.bar.set_visible(true);
        w.pause.set_visible(true);
        w.pause.set_icon_name("media-playback-pause-symbolic");
        w.pause.set_tooltip_text(Some("Pause"));
        w.status.set_label("Starting…");

        // Its progress, title and file, line by line.
        if let Some(out) = process.stdout_pipe() {
            let weak = self.me.borrow().clone();
            let lines = gio::DataInputStream::new(&out);
            glib::MainContext::default().spawn_local(async move {
                while let Ok(Some(line)) = lines.read_line_utf8_future(glib::Priority::LOW).await {
                    let Some(this) = weak.upgrade() else { return };
                    this.grab_line(id, ytdlp::parse(&line));
                }
            });
        }
        // Its errors: the last one is what the row says if it fails.
        if let Some(err) = process.stderr_pipe() {
            let weak = self.me.borrow().clone();
            let lines = gio::DataInputStream::new(&err);
            glib::MainContext::default().spawn_local(async move {
                while let Ok(Some(line)) = lines.read_line_utf8_future(glib::Priority::LOW).await {
                    let Some(this) = weak.upgrade() else { return };
                    if let Line::Error(e) = ytdlp::parse(&line)
                        && let Some(g) = this.grabs.borrow_mut().iter_mut().find(|g| g.id == id)
                    {
                        g.error = e;
                    }
                }
            });
        }
        let weak = self.me.borrow().clone();
        glib::MainContext::default().spawn_local(async move {
            let ended = process.wait_future().await;
            let Some(this) = weak.upgrade() else { return };
            // Only this run counts: paused, taken away, or run again since.
            let current = this.grabs.borrow().iter().find(|g| g.id == id).is_some_and(|g| {
                g.state == GrabState::Running && g.process.as_ref() == Some(&process)
            });
            if !current {
                return;
            }
            if ended.is_ok() && process.is_successful() {
                this.grab_done(id);
            } else {
                let error = this.grabs.borrow().iter().find(|g| g.id == id).map(|g| g.error.clone()).unwrap_or_default();
                let why = if error.is_empty() {
                    format!("yt-dlp stopped (exit {})", process.exit_status())
                } else {
                    ytdlp::short_error(&error)
                };
                this.grab_failed(id, &why);
            }
        });
    }

    fn grab_line(&self, id: u32, line: Line) {
        let w = {
            let mut grabs = self.grabs.borrow_mut();
            let Some(g) = grabs.iter_mut().find(|g| g.id == id) else { return };
            if g.state != GrabState::Running {
                return;
            }
            match &line {
                Line::Title(t) => g.name = t.clone(),
                Line::Progress(p) => {
                    if let Some(part) = &p.part
                        && !g.parts.contains(part)
                    {
                        g.parts.push(part.clone());
                    }
                    g.fraction = p.fraction().unwrap_or(g.fraction);
                }
                Line::File(f) => g.file = Some(f.clone()),
                _ => {}
            }
            g.widgets.clone()
        };
        match line {
            Line::Title(t) => w.label.set_label(&t),
            Line::Progress(p) => {
                match p.fraction() {
                    Some(f) => w.bar.set_fraction(f),
                    None => w.bar.pulse(),
                }
                let mut text = match p.total {
                    Some(t) => format!("{} of {}", size(p.got), size(t)),
                    None => size(p.got),
                };
                if let Some(s) = p.speed.filter(|s| *s > 0.0) {
                    text.push_str(&format!(" · {}/s", size(s as u64)));
                }
                if let Some(eta) = p.eta.filter(|e| *e > 0) {
                    text.push_str(&format!(" · {} left", duration(eta)));
                }
                w.status.set_label(&text);
            }
            Line::Finishing => {
                w.bar.pulse();
                w.status.set_label("Finishing…");
            }
            _ => {}
        }
    }

    fn grab_done(&self, id: u32) {
        let (w, path) = {
            let mut grabs = self.grabs.borrow_mut();
            let Some(g) = grabs.iter_mut().find(|g| g.id == id) else { return };
            let path = g.file.clone().filter(|f| f.exists()).unwrap_or_else(folder);
            if g.name.is_empty()
                && let Some(n) = g.file.as_ref().and_then(|f| f.file_name())
            {
                g.name = n.to_string_lossy().into_owned();
            }
            g.state = GrabState::Done(path.clone());
            g.process = None;
            g.fraction = 1.0;
            (g.widgets.clone(), path)
        };
        w.bar.set_visible(false);
        w.pause.set_visible(false);
        w.status.set_label(if path.is_dir() { "Done. Click to open the folder" } else { "Done. Click to open" });
    }

    fn grab_failed(&self, id: u32, why: &str) {
        let w = {
            let mut grabs = self.grabs.borrow_mut();
            let Some(g) = grabs.iter_mut().find(|g| g.id == id) else { return };
            g.state = GrabState::Failed(why.to_string());
            g.process = None;
            g.widgets.clone()
        };
        w.bar.set_visible(false);
        w.pause.set_visible(false);
        w.status.set_label(why);
        w.status.set_tooltip_text(Some(why));
    }

    fn grab_pause_or_resume(&self, id: u32) {
        let state = self.grabs.borrow().iter().find(|g| g.id == id).map(|g| g.state.clone());
        match state {
            Some(GrabState::Running) => {
                let (process, w, fraction) = {
                    let mut grabs = self.grabs.borrow_mut();
                    let Some(g) = grabs.iter_mut().find(|g| g.id == id) else { return };
                    g.state = GrabState::Paused;
                    (g.process.take(), g.widgets.clone(), g.fraction)
                };
                if let Some(p) = process {
                    p.force_exit();
                }
                w.pause.set_icon_name("media-playback-start-symbolic");
                w.pause.set_tooltip_text(Some("Resume"));
                w.status.set_label(&format!("Paused · {:.0}%", fraction * 100.0));
            }
            Some(GrabState::Paused) => self.run_grab(id),
            _ => {}
        }
    }

    /// ×: stops a running or paused one and removes what it left; takes a
    /// finished or failed one off the list (the file stays).
    fn grab_close(&self, id: u32) {
        let Some(g) = ({
            let mut grabs = self.grabs.borrow_mut();
            grabs.iter().position(|g| g.id == id).map(|i| grabs.remove(i))
        }) else {
            return;
        };
        if let Some(p) = &g.process {
            p.force_exit();
        }
        if matches!(g.state, GrabState::Running | GrabState::Paused) {
            for part in &g.parts {
                let _ = std::fs::remove_file(part);
                // The piece it finished before (video before sound), and yt-dlp's notes.
                if let Some(done) = part.to_str().and_then(|p| p.strip_suffix(".part")) {
                    let _ = std::fs::remove_file(done);
                    let _ = std::fs::remove_file(format!("{done}.ytdl"));
                }
            }
        }
        self.rows.remove(&g.widgets.row);
        self.refresh_visible();
    }
}

/// GETs the rest of `url` into `part` (from its length on), with the
/// cookies of `session` for that address. A server that ignores the range
/// sends it all: then `part` starts over.
async fn fetch_rest(
    url: &str,
    session: Option<webkit6::NetworkSession>,
    agent: &str,
    part: &std::path::Path,
    cancel: &gio::Cancellable,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    use std::io::Write;
    use webkit6::soup;
    use webkit6::soup::prelude::*;
    let have = std::fs::metadata(part).map_or(0, |m| m.len());
    // The page's cookies for that address, as it would send them.
    let cookies = match session.and_then(|s| s.cookie_manager()) {
        Some(manager) => cookie_header(manager, url).await,
        None => String::new(),
    };
    let client = soup::Session::new();
    if !agent.is_empty() {
        client.set_user_agent(agent);
    }
    if let Some(r) = crate::engine::proxy_resolver() {
        client.set_proxy_resolver(Some(&r));
    }
    let message = soup::Message::new("GET", url).map_err(|e| e.to_string())?;
    if let Some(h) = message.request_headers() {
        h.append("Range", &format!("bytes={have}-"));
        if !cookies.is_empty() {
            h.append("Cookie", &cookies);
        }
    }
    let stream = client.send_future(&message, glib::Priority::LOW).await.map_err(|e| e.to_string())?;
    let status = message.status();
    let (mut got, append) = match status {
        soup::Status::PartialContent => (have, true),
        soup::Status::Ok => (0, false),
        // Asked from its very end: nothing more to come.
        soup::Status::RequestedRangeNotSatisfiable => return Ok(()),
        other => return Err(format!("HTTP {other:?}")),
    };
    let rest = message.response_headers().map_or(0, |h| h.content_length()) as u64;
    let total = got + rest;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(|e| e.to_string())?;
    loop {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        let bytes = stream.read_bytes_future(64 * 1024, glib::Priority::LOW).await.map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            break;
        }
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        got += bytes.len() as u64;
        progress(got, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    if total > 0 && got < total {
        return Err("the connection ended early".into());
    }
    Ok(())
}

/// The `Cookie:` header `manager` would send to `url`.
fn cookie_header(manager: webkit6::CookieManager, url: &str) -> impl std::future::Future<Output = String> {
    let (tx, rx) = oneshot();
    manager.cookies(url, None::<&gio::Cancellable>, move |r| {
        let header = r
            .map(|mut list| {
                list.iter_mut()
                    .filter_map(|c| Some(format!("{}={}", c.name()?, c.value()?)))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        tx.send(header);
    });
    rx
}

/// A one-shot channel from a callback to a future, on the main context.
fn oneshot() -> (Sender, impl std::future::Future<Output = String>) {
    let slot: Rc<RefCell<(Option<String>, Option<std::task::Waker>)>> = Rc::default();
    let tx = Sender(slot.clone());
    let rx = std::future::poll_fn(move |cx| {
        let mut s = slot.borrow_mut();
        match s.0.take() {
            Some(v) => std::task::Poll::Ready(v),
            None => {
                s.1 = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        }
    });
    (tx, rx)
}

struct Sender(Rc<RefCell<(Option<String>, Option<std::task::Waker>)>>);

impl Sender {
    fn send(self, v: String) {
        let mut s = self.0.borrow_mut();
        s.0 = Some(v);
        if let Some(w) = s.1.take() {
            w.wake();
        }
    }
}

/// 1536 → "1.5 KB".
fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

/// 75 → "1 min", 3700 → "1 h 1 min", 40 → "40 s".
fn duration(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        _ => format!("{} h {} min", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sizes() {
        assert_eq!(super::size(512), "512 B");
        assert_eq!(super::size(1536), "1.5 KB");
        assert_eq!(super::size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn durations() {
        assert_eq!(super::duration(40), "40 s");
        assert_eq!(super::duration(75), "1 min");
        assert_eq!(super::duration(3700), "1 h 1 min");
    }
}
