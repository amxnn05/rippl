//! Downloading a video (Ctrl+Shift+Y): a bar over the top of the page with
//! one field. It opens holding the link on the clipboard if there is one,
//! else the front page's address; paste or type another. Return downloads
//! it as `grab_format` says (the best video, as .mp4, unless set to audio);
//! Shift+Return as the other one. Esc closes the bar.
//!
//! The download itself is yt-dlp's, listed with the others at the foot of
//! the column (downloads.rs); which links it accepts and how it is run are
//! in `rippl_core::ytdlp`. Nothing leaves until Return.

use rippl_core::ytdlp::{self, Format};
use webkit6::prelude::*;
use webkit6::{gdk, glib, gtk};

use crate::browser::Browser;

impl Browser {
    /// The layer over the page: a light dim, and in the middle the same
    /// field Ctrl+T shows on a new tab (560 wide at most, 50 tall, lifted
    /// above centre, HomeLayout), with a card under it for the hint or what
    /// went wrong. A click on the dim closes it.
    pub(crate) fn grab_bar() -> (gtk::Box, gtk::Entry, gtk::Label) {
        let layer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        layer.add_css_class("grab-dim");
        layer.set_hexpand(true);
        layer.set_vexpand(true);
        let layout: crate::home::HomeLayout = glib::Object::new();
        layer.set_layout_manager(Some(layout));

        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.add_css_class("grab-box");
        column.set_valign(gtk::Align::Start);
        let entry = gtk::Entry::builder()
            .placeholder_text("Paste a video link")
            .input_purpose(gtk::InputPurpose::Url)
            .hexpand(true)
            .build();
        entry.add_css_class("field");
        entry.add_css_class("home-field");
        entry.set_primary_icon_name(Some("folder-download-symbolic"));
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("home-offers");
        card.add_css_class("grab-note");
        let note = gtk::Label::builder().xalign(0.0).wrap(true).build();
        card.append(&note);
        column.append(&entry);
        column.append(&card);
        column.set_parent(&layer);

        layer.set_visible(false);
        (layer, entry, note)
    }

    pub(crate) fn wire_grab(&self) {
        let keys = gtk::EventControllerKey::new();
        let weak = self.weak();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    let format = b.config.borrow().grab_format;
                    let format = if modifiers.contains(gdk::ModifierType::SHIFT_MASK) { format.other() } else { format };
                    let text = b.grab_entry.text().to_string();
                    match b.grab_link(&text, format) {
                        Ok(_) => b.close_grab(),
                        Err(why) => {
                            b.grab_note.add_css_class("grab-error");
                            b.grab_note.set_label(&why);
                        }
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    b.close_grab();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.grab_entry.add_controller(keys);
        // A click on the dim (not on the field or its card) closes the bar.
        let click = gtk::GestureClick::new();
        let weak = self.weak();
        click.connect_released(move |_, _, x, y| {
            let Some(b) = weak.upgrade() else { return };
            let dim: &gtk::Widget = b.grab_bar.upcast_ref();
            if dim.pick(x, y, gtk::PickFlags::DEFAULT).as_ref() == Some(dim) {
                b.close_grab();
            }
        });
        self.grab_bar.add_controller(click);
        // What Return and Shift+Return do, said again once the text changes.
        let weak = self.weak();
        self.grab_entry.connect_changed(move |_| {
            if let Some(b) = weak.upgrade() {
                b.grab_hint();
            }
        });
    }

    /// The note under the field when nothing is wrong.
    fn grab_hint(&self) {
        let format = self.config.borrow().grab_format;
        let name = |f: Format| match f {
            Format::Video => "video (.mp4)",
            Format::Audio => "audio only (.mp3)",
        };
        self.grab_note.remove_css_class("grab-error");
        self.grab_note.set_label(&format!("Return: {} · Shift+Return: {}", name(format), name(format.other())));
    }

    pub(crate) fn open_grab(&self) {
        if !self.config.borrow().grab {
            self.notice("Downloading videos is off in Settings");
            return;
        }
        self.grab_entry.set_text("");
        self.grab_hint();
        self.grab_bar.set_visible(true);
        self.grab_entry.grab_focus();
        // The front page's address, if it is one; the clipboard's link wins.
        let front = self.front_view().and_then(|v| v.uri()).map(|u| u.to_string()).and_then(|u| ytdlp::link(&u));
        if let Some(url) = &front {
            self.grab_entry.set_text(url);
            self.grab_entry.select_region(0, -1);
        }
        let clipboard = gdk::prelude::DisplayExt::clipboard(&WidgetExt::display(&self.window));
        let weak = self.weak();
        clipboard.read_text_async(None::<&webkit6::gio::Cancellable>, move |text| {
            let (Some(b), Ok(Some(text))) = (weak.upgrade(), text) else { return };
            // Only if the bar is still open and nobody has typed since.
            let untouched = b.grab_entry.text().is_empty() || b.grab_entry.text().as_str() == front.as_deref().unwrap_or("");
            if let Some(url) = ytdlp::link(&text)
                && b.grab_bar.is_visible()
                && untouched
            {
                b.grab_entry.set_text(&url);
                b.grab_entry.select_region(0, -1);
            }
        });
    }

    pub(crate) fn close_grab(&self) {
        self.grab_bar.set_visible(false);
        if let Some(view) = self.front_view() {
            view.grab_focus();
        }
    }

    /// Downloads what `text` links to. Ok holds the link; Err says why not.
    pub(crate) fn grab_link(&self, text: &str, format: Format) -> Result<String, String> {
        if !self.config.borrow().grab {
            return Err("Downloading videos is off in Settings".into());
        }
        let url = ytdlp::link(text).ok_or_else(|| {
            if text.trim().is_empty() { "Paste a link first".to_string() } else { "That isn't a link".to_string() }
        })?;
        self.downloads.grab(&url, format)?;
        Ok(url)
    }
}
