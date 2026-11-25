//! Downloads: saved to your Downloads folder under the site's name for the
//! file (cleaned, and never overwriting: `report (1).pdf`), and listed at
//! the foot of the column while rippl runs. Click a finished one to open
//! it; × cancels a running one or takes a finished one off the list.
//!
//! Nothing about downloads is kept after quitting: the files are in your
//! Downloads folder, and that is the record.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use webkit6::prelude::*;
use webkit6::{gio, glib, gtk, Download};

struct Item {
    download: Download,
    row: gtk::Box,
    name: String,
    state: State,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Running,
    Done(PathBuf),
    Failed(String),
}

pub struct Downloads {
    /// The column's foot: a heading and the rows. Hidden while empty.
    pub area: gtk::Box,
    rows: gtk::Box,
    items: RefCell<Vec<Item>>,
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
            Rc::new(Downloads { area, rows, items: RefCell::new(Vec::new()), me: RefCell::new(Weak::new()) });
        *downloads.me.borrow_mut() = Rc::downgrade(&downloads);
        downloads
    }

    /// A download began, from any tab or session.
    pub fn started(&self, download: &Download) {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("download");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let label = gtk::Label::builder()
            .label("Starting…")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(1)
            .build();
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.add_css_class("close");
        close.set_tooltip_text(Some("Cancel or remove"));
        line.append(&label);
        line.append(&close);
        let bar = gtk::ProgressBar::new();
        let status = gtk::Label::builder().xalign(0.0).build();
        status.add_css_class("dim");
        row.append(&line);
        row.append(&bar);
        row.append(&status);
        self.rows.prepend(&row);
        self.area.set_visible(true);
        self.items.borrow_mut().push(Item {
            download: download.clone(),
            row: row.clone(),
            name: String::new(),
            state: State::Running,
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
            if item.state != State::Running {
                return; // failed first; "finished" follows every failure
            }
            let path = d.destination().map(|p| PathBuf::from(p.as_str())).unwrap_or_default();
            done_bar.set_visible(false);
            done_status.set_label("Done. Click to open");
            item.state = State::Done(path);
        });

        let weak = self.me.borrow().clone();
        download.connect_failed(move |d, error| {
            let Some(this) = weak.upgrade() else { return };
            let mut items = this.items.borrow_mut();
            let Some(item) = items.iter_mut().find(|i| &i.download == d) else { return };
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
            let running = this.items.borrow().iter().any(|i| i.download == d && i.state == State::Running);
            if running {
                d.cancel();
            } else {
                this.remove(&d);
            }
        });
    }

    fn remove(&self, download: &Download) {
        let mut items = self.items.borrow_mut();
        if let Some(i) = items.iter().position(|i| &i.download == download) {
            let item = items.remove(i);
            self.rows.remove(&item.row);
        }
        self.area.set_visible(!items.is_empty());
    }

    /// One line per download, for the control socket.
    pub fn status(&self) -> String {
        let items = self.items.borrow();
        if items.is_empty() {
            return "none".into();
        }
        items
            .iter()
            .map(|i| match &i.state {
                State::Running => format!("running\t{}\t{:.0}%", i.name, i.download.estimated_progress() * 100.0),
                State::Done(p) => format!("done\t{}\t{}", i.name, p.display()),
                State::Failed(why) => format!("failed\t{}\t{why}", i.name),
            })
            .collect::<Vec<_>>()
            .join("\n")
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

#[cfg(test)]
mod tests {
    #[test]
    fn sizes() {
        assert_eq!(super::size(512), "512 B");
        assert_eq!(super::size(1536), "1.5 KB");
        assert_eq!(super::size(5 * 1024 * 1024), "5.0 MB");
    }
}
