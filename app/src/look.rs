//! How rippl looks: the design, carried over.
//!
//! - The page is a white **card** on a slightly off-white ground (8 px gap,
//!   10 px corners); the column of tabs sits on that ground.
//! - **Rows** are 28 px, titles 12.5-point, quiet grey until live; the live
//!   tab lies on a soft grey pill, the one under the pointer on a lighter
//!   one. Each row starts with the site's **letter** in a faint square.
//! - **Pinned** tabs are letter squares in a row at the top.
//! - A faint **+ New tab** row closes the list; the sliders at the foot
//!   open Settings.
//!
//! Every colour is one of the light/dark pairs, written once below
//! (GTK 4.16+ CSS variables, GTK 4.20+ `prefers-color-scheme`), and follows
//! the theme setting (system, light, dark). Nothing else in rippl knows
//! which it is.

use rippl_core::address::monogram;
use rippl_core::tabs::Life;
use webkit6::prelude::*;
use webkit6::{gdk, glib, gtk};

use crate::browser::Browser;

/// the palette: greys as
/// light / dark pairs. The column's greys are black or white at a strength
/// that lands on the own over the frame colour.
const CSS: &str = r#"
window.rippl {
  --ground: #ffffff; --frame: #f4f4f4; --ink: #171717; --muted: rgba(0,0,0,0.424);
  --faint: rgba(0,0,0,0.131); --hairline: #e8e8e8; --wash: rgba(0,0,0,0.063);
  --hover: rgba(0,0,0,0.042); --tile: rgba(0,0,0,0.031); --pin-live: rgba(0,0,0,0.10);
  --mark: rgba(23,23,23,0.06); --accent: #171717;
}
@media (prefers-color-scheme: dark) {
  window.rippl {
    --ground: #1c1c1c; --frame: #131313; --ink: #ededed; --muted: rgba(255,255,255,0.546);
    --faint: rgba(255,255,255,0.265); --hairline: #333333; --wash: rgba(255,255,255,0.087);
    --hover: rgba(255,255,255,0.06); --tile: rgba(255,255,255,0.043); --pin-live: rgba(255,255,255,0.124);
    --mark: rgba(237,237,237,0.06); --accent: #ededed;
  }
}

window.rippl { background: var(--frame); color: var(--ink);
  font-family: "Inter", "Adwaita Sans", "SF Pro Text", system-ui, sans-serif; }
window.rippl .column { padding: 10px 6px 8px 10px; background: transparent; }

/* The address field: a quiet row, not a box. */
window.rippl .field { min-height: 30px; border-radius: 9px; border: none; box-shadow: none; outline: none;
  background: var(--tile); color: var(--ink); font-size: 13px; padding: 0 8px; margin-bottom: 4px; }
window.rippl .field:focus-within { background: var(--ground); box-shadow: 0 0 0 1px var(--hairline); }
window.rippl .field image { color: var(--muted); -gtk-icon-size: 13px; }

/* Pinned tabs: squares holding one letter. */
window.rippl .pins { margin-bottom: 8px; }
window.rippl .pins flowboxchild { padding: 0; min-width: 0; min-height: 0; background: none; }
window.rippl .pin { min-width: 30px; min-height: 30px; padding: 0; border-radius: 9px; border: none;
  background: var(--tile); color: var(--muted); font-size: 12px; font-weight: 500; box-shadow: none; }
window.rippl .pin:hover { background: var(--wash); color: var(--ink); }
window.rippl .pin.front { background: var(--pin-live); color: var(--ink); }
window.rippl .pin.asleep { opacity: 0.6; }

/* Tab rows. */
window.rippl .tabs, window.rippl .tabs row { background: transparent; }
window.rippl .tabs row { min-height: 28px; border-radius: 9px; padding: 0 6px 0 10px; margin: 1px 0;
  color: var(--muted); font-size: 12.5px; }
window.rippl .tabs row:hover { background: var(--hover); }
window.rippl .tabs row.front { background: var(--wash); color: var(--ink); }
window.rippl .tabs row.asleep { opacity: 0.55; }
window.rippl .tabs row.asleep:hover { opacity: 1; }
window.rippl .tabs row.split { box-shadow: inset 2px 0 0 var(--accent); }
window.rippl .tabs row button.close { opacity: 0; min-height: 18px; min-width: 18px; padding: 0; color: var(--muted); }
window.rippl .tabs row:hover button.close { opacity: 0.8; }

/* The site's letter in a faint square. */
window.rippl .mark { min-width: 15px; min-height: 15px; border-radius: 3px; background: var(--mark);
  color: var(--muted); font-size: 8.5px; font-weight: 500; }
window.rippl .tabs row.front .mark { color: var(--ink); }

/* New tab, and the foot. */
window.rippl .new-tab { min-height: 28px; border-radius: 9px; padding: 0 10px; border: none; box-shadow: none;
  background: transparent; color: var(--faint); font-size: 12.5px; }
window.rippl .new-tab:hover { background: var(--hover); color: var(--muted); }
window.rippl .foot button { min-height: 26px; min-width: 26px; padding: 0; border: none; box-shadow: none;
  background: transparent; color: var(--faint); }
window.rippl .foot button:hover { color: var(--ink); background: var(--hover); }

/* The page: a white card. */
window.rippl .stage { background: var(--ground); border-radius: 10px; margin: 8px 8px 8px 0;
  box-shadow: 0 0 0 1px var(--hairline); }
window.rippl paned.split > separator { background: var(--hairline); min-width: 6px; }
window.rippl webview.pane-front { box-shadow: inset 0 2px 0 var(--accent); }

/* What the field offers while you type. */
window.rippl .offers, window.rippl .offers row { background: transparent; }
window.rippl .offers row { border-radius: 9px; padding: 5px 10px; margin: 1px 0; color: var(--ink); font-size: 12.5px; }
window.rippl .offers row:hover { background: var(--hover); }
window.rippl .offers row:selected { background: var(--wash); color: var(--ink); }
window.rippl .dim { color: var(--muted); font-size: 11.5px; }

/* Downloads at the foot of the column. */
window.rippl .downloads { padding: 6px 4px 0 6px; border-top: 1px solid var(--hairline); }
window.rippl .downloads .heading { color: var(--muted); font-size: 11.5px; font-weight: 600; }
window.rippl .download { padding: 4px 2px; font-size: 12.5px; }
window.rippl .download button.close { min-height: 18px; min-width: 18px; padding: 0; opacity: 0.7; }

/* Over the page: dark, opaque, white text, whatever the page or theme. */
window.rippl .find, window.rippl .ask, window.rippl .toast { background: rgba(28,28,30,0.96); color: white;
  border-radius: 12px; padding: 6px 10px; box-shadow: 0 4px 16px rgba(0,0,0,0.25); font-size: 13px; }
window.rippl .toast { border-radius: 18px; padding: 8px 16px; }
window.rippl .status { background: var(--ground); color: var(--muted); border-radius: 7px; padding: 3px 8px;
  font-size: 11.5px; box-shadow: 0 0 0 1px var(--hairline); }
window.rippl .ask label, window.rippl .find label { color: white; }
window.rippl .accounts row { padding: 6px 12px; border-radius: 6px; }

/* The site card and the tab menu. */
window.rippl popover.card > contents, window.rippl popover.tab-menu > contents { background: var(--ground);
  color: var(--ink); border-radius: 12px; box-shadow: 0 6px 24px rgba(0,0,0,0.18), 0 0 0 1px var(--hairline); }
window.rippl .card-title { font-weight: 600; font-size: 14px; }
window.rippl .safe { color: #15803d; font-size: 12px; }
window.rippl .unsafe { color: #b45309; font-size: 12px; }
@media (prefers-color-scheme: dark) {
  window.rippl .safe { color: #4ade80; }
  window.rippl .unsafe { color: #fbbf24; }
}
window.rippl popover.card button { font-size: 12.5px; padding: 3px 10px; }
window.rippl .menu-item { font-size: 13px; padding: 5px 14px; min-width: 180px; }
"#;

impl Browser {
    pub(crate) fn style(&self) {
        self.window.add_css_class("rippl");
        let css = gtk::CssProvider::new();
        css.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(&self.window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    /// The quiet row at the end of the list.
    pub(crate) fn new_tab_row() -> gtk::Button {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let plus = gtk::Label::builder().label("+").width_chars(2).build();
        let label = gtk::Label::builder().label("New tab").xalign(0.0).build();
        line.append(&plus);
        line.append(&label);
        // The column's buttons never take the keyboard: a key
        // pressed while one had the focus used to press it.
        let button = gtk::Button::builder().child(&line).focusable(false).build();
        button.add_css_class("new-tab");
        button
    }

    /// The column's foot: the sliders that open Settings.
    pub(crate) fn column_foot() -> (gtk::Box, gtk::Button) {
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        foot.add_css_class("foot");
        let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings (Ctrl+,)"));
        settings.set_focusable(false);
        foot.append(&settings);
        (foot, settings)
    }

    pub(crate) fn wire_look(&self) {
        let weak = self.weak();
        self.new_tab.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.open("", false, true);
            }
        });
        let weak = self.weak();
        self.settings_button.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.open_internal("settings");
            }
        });
    }

    /// Draws the column again: pins, then rows. Cheap enough at browser
    /// scale to do whole.
    pub(crate) fn render(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        while let Some(child) = self.pins.first_child() {
            self.pins.remove(&child);
        }
        let tabs = self.tabs.borrow();
        let partner = self.partner.borrow();
        let mut any_pin = false;
        for tab in &tabs.list {
            let id = tab.id;
            let live = Some(id) == tabs.front;
            if tab.pinned {
                any_pin = true;
                let pin = gtk::Button::with_label(&monogram(&tab.url, &tab.title));
                pin.add_css_class("pin");
                pin.set_focusable(false);
                pin.set_tooltip_text(Some(&tab.title));
                if live {
                    pin.add_css_class("front");
                }
                if tab.life != Life::Awake {
                    pin.add_css_class("asleep");
                }
                self.on_click(&pin, id);
                self.pins.append(&pin);
                continue;
            }
            let row = gtk::ListBoxRow::new();
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if !tab.url.is_empty() {
                let mark = gtk::Label::new(Some(&if tab.private { "◐".to_string() } else { monogram(&tab.url, &tab.title) }));
                mark.add_css_class("mark");
                mark.set_valign(gtk::Align::Center);
                line.append(&mark);
            }
            let title = if tab.playing_audio { format!("{} ♪", tab.title) } else { tab.title.clone() };
            let label = gtk::Label::builder()
                .label(title)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .build();
            let close = gtk::Button::from_icon_name("window-close-symbolic");
            close.add_css_class("flat");
            close.add_css_class("close");
            close.set_tooltip_text(Some("Close tab"));
            close.set_focusable(false);
            line.append(&label);
            line.append(&close);
            row.set_child(Some(&line));
            row.set_tooltip_text(Some(&tab.url));
            if live {
                row.add_css_class("front");
            }
            if tab.life != Life::Awake && !tab.url.is_empty() {
                row.add_css_class("asleep");
            }
            if partner.contains_key(&id) {
                row.add_css_class("split");
                row.set_tooltip_text(Some(&format!("{} · side by side (Ctrl+Alt+S separates)", tab.url)));
            }
            let weak = self.weak();
            close.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.close(id);
                }
            });
            self.on_click(&row, id);
            self.list.append(&row);
        }
        self.pins.set_visible(any_pin);
    }

    /// Click: select. Middle-click: close. Alt-click: beside the front tab.
    fn on_click(&self, widget: &impl IsA<gtk::Widget>, id: rippl_core::tabs::TabId) {
        let click = gtk::GestureClick::builder().button(0).build();
        let weak = self.weak();
        click.connect_released(move |gesture, _, _, _| {
            let Some(b) = weak.upgrade() else { return };
            let button = gesture.current_button();
            let alt = gesture.current_event_state().contains(gdk::ModifierType::ALT_MASK);
            // Deferred: the widget this gesture belongs to is about to be
            // replaced by the redraw that selecting does.
            let front = b.front() == Some(id);
            let anchor = gesture.widget();
            glib::idle_add_local_once(move || match (button, alt) {
                (2, _) => b.close(id),
                (3, _) => {
                    if let Some(w) = anchor {
                        b.tab_menu(id, &w);
                    }
                }
                (1, true) => b.split_with(id),
                // A click on the tab you're on: its site card.
                (1, false) if front => b.open_site_card(),
                _ => b.select(id),
            });
        });
        widget.add_controller(click);
    }
}
