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
//! Every colour is one of the light/dark pairs, written once below as
//! CSS variables (GTK 4.16+), and the dark ones apply when the window has
//! the `dark` class. `apply_look` decides: `theme = light | dark`, or for
//! `system` the desktop's own choice, read from the XDG settings portal
//! (`org.freedesktop.appearance color-scheme`) and followed live. Pages are
//! told the same through GTK's interface colour scheme, which WebKit reads
//! for `prefers-color-scheme`. Nothing else in rippl knows which it is.

use rippl_core::address::monogram;
use rippl_core::tabs::{Life, TabId};
use webkit6::prelude::*;
use webkit6::{gdk, gio, glib, gtk};

use crate::config::Theme;

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
/* Dark: set by rippl (apply_look), not by a media query, which GTK
   doesn't evaluate from the desktop's preference without libadwaita. */
window.rippl.dark {
  --ground: #1c1c1c; --frame: #131313; --ink: #ededed; --muted: rgba(255,255,255,0.546);
  --faint: rgba(255,255,255,0.265); --hairline: #333333; --wash: rgba(255,255,255,0.087);
  --hover: rgba(255,255,255,0.06); --tile: rgba(255,255,255,0.043); --pin-live: rgba(255,255,255,0.124);
  --mark: rgba(237,237,237,0.06); --accent: #ededed;
}

/* Glass: the column's ground lets the desktop
   through, so a compositor that blurs (Hyprland) frosts it. A solid
   background makes GTK tell the compositor the window is opaque, and then
   nothing is blurred. The page's card stays solid. */
window.rippl.glass { --frame: rgba(244,244,244,0.72); backdrop-filter: blur(24px); }
window.rippl.dark.glass { --frame: rgba(19,19,19,0.70); }

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
window.rippl .pin { min-width: 30px; min-height: 30px; padding: 0; border-radius: 9px; border: none;
  background: var(--tile); color: var(--muted); font-size: 12px; font-weight: 500; box-shadow: none; }
window.rippl .pin:hover { background: var(--wash); color: var(--ink); }
window.rippl .pin.front { background: var(--pin-live); color: var(--ink); }
window.rippl .pin.asleep { opacity: 0.6; }
window.rippl .pin .unread { background: #ed3d36; color: white; font-size: 9px; font-weight: 700;
  min-width: 13px; min-height: 13px; padding: 0 3px; border-radius: 7px;
  box-shadow: 0 0 0 1.5px var(--frame); }

/* Tab rows. */
window.rippl .tabs, window.rippl .tabs row { background: transparent; }
window.rippl .tabs row { min-height: 28px; border-radius: 9px; padding: 0 6px 0 10px; margin: 1px 0;
  color: var(--muted); font-size: 12.5px; }
window.rippl .tabs row:hover { background: var(--hover); }
window.rippl .tabs row.front { background: var(--wash); color: var(--ink); }
window.rippl .tabs row.asleep { opacity: 0.55; }
window.rippl .tabs row.asleep:hover { opacity: 1; }
window.rippl .tabs row.split { box-shadow: inset 2px 0 0 var(--accent); }
window.rippl .tabs row.in-group { margin-left: 12px; }
window.rippl .tabs row.group-head { min-height: 24px; color: var(--muted); font-size: 11.5px; font-weight: 600;
  padding: 6px 6px 0 8px; }
window.rippl .tabs row.group-head:hover { background: transparent; color: var(--ink); }
window.rippl .foot .space { min-width: 22px; min-height: 22px; padding: 0; border-radius: 6px; border: none;
  box-shadow: none; background: transparent; color: var(--faint); font-size: 11px; font-weight: 600; }
window.rippl .foot .space:hover { color: var(--ink); background: var(--hover); }
window.rippl .foot .space.here { color: var(--ink); background: var(--wash); }
window.rippl .tabs row button.close { opacity: 0; min-height: 18px; min-width: 18px; padding: 0; color: var(--muted); }
window.rippl .tabs row:hover button.close { opacity: 0.8; }
/* Loading: the ring in the ×'s place, giving way to it under the pointer. */
window.rippl .tabs row .loader, window.rippl .top-tab .loader { transition: opacity 140ms ease-out; }
window.rippl .tabs row.loading:hover .loader, window.rippl .top-tab.loading:hover .loader { opacity: 0; }
window.rippl .top-tab.loading:not(:hover) .close { opacity: 0; }

/* The site's letter in a faint square. */
window.rippl .mark { min-width: 15px; min-height: 15px; border-radius: 3px; background: var(--mark);
  color: var(--muted); font-size: 8.5px; font-weight: 500; }
window.rippl .tabs row.front .mark { color: var(--ink); }
window.rippl .tabs .icon { border-radius: 3px; }
window.rippl .css-edit { border-radius: 8px; box-shadow: 0 0 0 1px var(--hairline); }

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
/* The new tab: the field on its own in the middle of the page. */
window.rippl .field.home-field { min-height: 50px; padding: 0 22px; border-radius: 14px; font-size: 15.5px;
  background: var(--ground); color: var(--ink);
  box-shadow: 0 0 0 1px var(--hairline), 0 8px 24px rgba(0,0,0,0.06); }
window.rippl .field.home-field:focus-within { background: var(--ground);
  box-shadow: 0 0 0 1px var(--hairline), 0 8px 24px rgba(0,0,0,0.06); }
window.rippl .field.home-field > text > placeholder { color: var(--faint); }
window.rippl .field.home-field > image { color: var(--faint); }
window.rippl .home-offers { background: var(--ground); border-radius: 14px; padding: 6px;
  box-shadow: 0 0 0 1px var(--hairline), 0 6px 20px rgba(0,0,0,0.07); }
window.rippl .home-offers row { padding: 9px 12px; font-size: 13px; }
window.rippl .home-grid { margin-top: 2px; }
/* Ctrl+Shift+Y (grab.rs): the new tab's field over a light dim, a card under it. */
window.rippl .grab-dim { background: rgba(0,0,0,0.12); }
window.rippl .grab-note { padding: 10px 14px; }
window.rippl .grab-note label { color: var(--muted); font-size: 12.5px; }
window.rippl .grab-note label.grab-error { color: #b45309; }
window.rippl .home-grid > flowboxchild { padding: 0; }
window.rippl .home-tile { min-height: 50px; padding: 0 10px; border-radius: 14px; background: transparent;
  box-shadow: inset 0 0 0 1px var(--hairline); border: none; color: var(--muted); font-size: 12px; }
window.rippl .home-tile:hover { background: var(--hover); color: var(--ink); }
/* Peek: the page dimmed, a panel over it. */
window.rippl .bookmarks-bar { margin: 6px 8px 0 0; }
window.rippl .stage.under-bar { margin-top: 4px; }
window.rippl .bar-item { min-height: 24px; padding: 0 8px; border-radius: 6px; font-size: 12px; color: var(--ink); background: none; box-shadow: none; }
window.rippl .bar-item:hover { background: var(--wash); }
window.rippl .bar-hint { font-size: 12px; color: var(--muted); padding: 0 8px; }
window.rippl .bar-menu .bar-item { padding: 4px 8px; }
window.rippl .bar-folder-head { font-size: 11px; color: var(--muted); padding: 6px 8px 2px; }
/* The column as icons only (chrome.rs). */
window.rippl .column.icons { padding: 10px 6px 8px 6px; }
window.rippl .column.icons .tabs row.tile { min-height: 32px; padding: 0; margin: 1px 4px; border-radius: 8px; }
window.rippl .column.icons .new-tab { padding: 0; }
window.rippl.no-column .stage { margin-left: 8px; }
window.rippl.no-column .bookmarks-bar { margin-left: 8px; }
/* The strip above the page: the field, and the tabs across the top. */
window.rippl .top-strip { margin: 8px 8px 0 8px; min-height: 32px; }
window.rippl .top-strip .field { margin-bottom: 0; }
window.rippl .top-tabs { padding: 0 2px; }
window.rippl .top-tab { min-height: 30px; padding: 0 4px 0 10px; border-radius: 9px; color: var(--muted); font-size: 12.5px; }
window.rippl .top-tab.pinned { padding: 0 10px; }
window.rippl .top-tab:hover { background: var(--hover); color: var(--ink); }
window.rippl .top-tab.front { background: var(--wash); color: var(--ink); }
window.rippl .top-tab.asleep { opacity: 0.75; }
window.rippl .top-tab .close { min-width: 20px; min-height: 20px; padding: 0; opacity: 0; }
window.rippl .top-tab:hover .close, window.rippl .top-tab.front .close { opacity: 0.7; }
window.rippl .top-knob { min-width: 30px; min-height: 30px; padding: 0; border-radius: 9px; border: none;
  background: none; box-shadow: none; color: var(--muted); }
window.rippl .top-knob:hover { background: var(--hover); color: var(--ink); }
window.rippl .top-offers { margin: 4px 0 0 4px; }
/* A space's colour choices (space_look.rs). */
window.rippl .space.tint-4285f5 { color: #4285f5; }
window.rippl .space.tint-33a873 { color: #33a873; }
window.rippl .space.tint-f59e33 { color: #f59e33; }
window.rippl .space.tint-e65466 { color: #e65466; }
window.rippl .space.tint-9e66e6 { color: #9e66e6; }
window.rippl .swatch { min-width: 20px; min-height: 20px; padding: 0; border-radius: 10px; border: none;
  box-shadow: inset 0 0 0 1px var(--hairline); background: var(--tile); color: var(--muted); -gtk-icon-size: 10px; }
window.rippl .swatch.chosen { box-shadow: 0 0 0 2px var(--ink); }
window.rippl .swatch-title { font-size: 11px; color: var(--muted); }
window.rippl .swatch.swatch-4285f5 { background: #4285f5; }
window.rippl .swatch.swatch-33a873 { background: #33a873; }
window.rippl .swatch.swatch-f59e33 { background: #f59e33; }
window.rippl .swatch.swatch-e65466 { background: #e65466; }
window.rippl .swatch.swatch-9e66e6 { background: #9e66e6; }
window.rippl .pin.pin-folder { color: var(--muted); -gtk-icon-size: 14px; }
/* The AI add-on's panel (ai.rs). */
window.rippl .ai-panel { margin: 8px 8px 8px 0; padding: 12px; border-radius: 10px; background: var(--ground);
  box-shadow: 0 0 0 1px var(--hairline); }
window.rippl .ai-title { font-weight: 600; font-size: 13px; }
window.rippl .ai-answer { background: transparent; color: var(--ink); font-size: 13px; }
window.rippl .ai-answer text { background: transparent; }
window.rippl .ai-ask { min-height: 32px; border-radius: 9px; }
/* Settings' sections in the column (settings_rail.rs) rail. */
window.rippl .settings-rail { padding: 0 0 8px; }
window.rippl .rail-back { min-height: 28px; padding: 0 10px; border-radius: 8px; border: none; box-shadow: none;
  background: transparent; color: var(--muted); font-size: 12.5px; }
window.rippl .rail-back:hover { background: var(--hover); color: var(--ink); }
window.rippl .rail-title { font-size: 13px; font-weight: 600; color: var(--ink); padding: 8px 10px 8px; }
window.rippl .rail-find { min-height: 28px; border-radius: 8px; margin: 0 2px 10px; font-size: 12.5px;
  background: var(--ground); box-shadow: 0 0 0 1px var(--hairline); color: var(--ink); }
window.rippl .rail-rule { margin: 7px 10px; background: var(--hairline); min-height: 1px; }
window.rippl .rail-row { min-height: 30px; padding: 0 10px; border-radius: 8px; border: none; box-shadow: none;
  background: transparent; color: var(--muted); font-size: 13px; }
window.rippl .rail-row:hover { background: var(--hover); color: var(--ink); }
window.rippl .rail-row.here { background: var(--ground); color: var(--ink); font-weight: 500;
  box-shadow: 0 1px 3px rgba(0,0,0,0.06), 0 0 0 1px var(--hairline); }
window.rippl .rail-glyph { font-size: 12px; }
window.rippl .switcher-dim { background: rgba(0,0,0,0.22); }
window.rippl .switcher { background: var(--ground); border-radius: 14px; padding: 18px;
  box-shadow: 0 0 0 1px var(--hairline), 0 10px 30px rgba(0,0,0,0.25); }
window.rippl .switch-card { padding: 6px; border-radius: 10px; font-size: 12px; color: var(--ink); }
window.rippl .switch-card.chosen { background: var(--wash); }
window.rippl .switch-face { border-radius: 7px; background: var(--frame); box-shadow: inset 0 0 0 1px var(--hairline); }
window.rippl .switch-mark { font-size: 28px; font-weight: 600; opacity: 0.6; }
window.rippl .peek-dim { background: rgba(0,0,0,0.22); }
window.rippl .peek-panel { background: var(--ground); border-radius: 12px;
  box-shadow: 0 0 0 1px var(--hairline), 0 10px 30px rgba(0,0,0,0.25); }
window.rippl .peek-knob { min-width: 28px; min-height: 28px; padding: 0; border-radius: 14px; border: none;
  background: var(--ground); color: var(--ink); box-shadow: inset 0 0 0 1px var(--hairline); }
window.rippl .peek-knob:hover { background: var(--hover); }

/* The player at the foot: a hairline, not a tab's ground. */
window.rippl .player { min-height: 28px; padding: 0 7px 0 10px; border-radius: 9px; margin: 0 0 4px 0;
  box-shadow: inset 0 0 0 1px var(--hairline); color: var(--muted); font-size: 12.5px; }
window.rippl .player:hover { background: var(--hover); color: var(--ink); }
window.rippl .player-button { min-width: 15px; min-height: 15px; padding: 0; border-radius: 8px; border: none;
  background: transparent; color: var(--muted); box-shadow: none; -gtk-icon-size: 9px; }
window.rippl .player-button:hover { background: var(--wash); }
window.rippl paned.split > separator { background: var(--hairline); min-width: 6px; min-height: 6px; }
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
window.rippl.dark .safe { color: #4ade80; }
window.rippl.dark .unsafe { color: #fbbf24; }
window.rippl popover.card button { font-size: 12.5px; padding: 3px 10px; }
window.rippl .menu-item { font-size: 13px; padding: 5px 14px; min-width: 180px; }
"#;

/// What the desktop prefers, from the settings portal: Some(true) dark,
/// Some(false) light, None no preference (or no portal).
fn desktop_prefers_dark() -> Option<bool> {
    let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>).ok()?;
    let reply = bus
        .call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "ReadOne",
            Some(&("org.freedesktop.appearance", "color-scheme").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            500,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    // (v) holding a u32: 1 prefer dark, 2 prefer light, 0 no preference.
    let value = reply.child_value(0).as_variant()?.get::<u32>()?;
    match value {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

impl Browser {
    /// Light or dark, everywhere: rippl's palette, GTK's widgets, and pages.
    pub(crate) fn apply_look(&self) {
        let theme = self.config.borrow().theme;
        let dark = match theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => desktop_prefers_dark().unwrap_or(false),
        };
        if dark {
            self.window.add_css_class("dark");
        } else {
            self.window.remove_css_class("dark");
        }
        self.apply_space_look();
        self.apply_home_theme();
        if self.config.borrow().glass {
            self.window.add_css_class("glass");
        } else {
            self.window.remove_css_class("glass");
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_interface_color_scheme(if dark {
                gtk::InterfaceColorScheme::Dark
            } else {
                gtk::InterfaceColorScheme::Light
            });
            // WebKit also reads darkness from the GTK theme's name: a theme
            // called "…-dark" (Omarchy's Adwaita-dark) makes every page
            // dark whatever the colour scheme says. In rippl only, the light
            // look takes the theme's light variant, and the dark look gets
            // the desktop's own theme back.
            let desktop = self.desktop_theme.get_or_init(|| settings.gtk_theme_name().map(|n| n.to_string()));
            let wanted = match (dark, desktop.as_deref()) {
                (false, Some(name)) if name.ends_with("-dark") => Some(name.trim_end_matches("-dark").to_string()),
                (_, Some(name)) => Some(name.to_string()),
                (_, None) => None,
            };
            if settings.gtk_theme_name().map(|n| n.to_string()) != wanted {
                settings.set_gtk_theme_name(wanted.as_deref());
            }
            #[allow(deprecated)]
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
    }

    /// With `theme = system`, a change of the desktop's colour scheme is
    /// followed at once.
    pub(crate) fn follow_desktop_look(&self) {
        let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else { return };
        let weak = self.weak();
        #[allow(deprecated)]
        bus.signal_subscribe(
            Some("org.freedesktop.portal.Desktop"),
            Some("org.freedesktop.portal.Settings"),
            Some("SettingChanged"),
            Some("/org/freedesktop/portal/desktop"),
            None,
            gio::DBusSignalFlags::NONE,
            move |_, _, _, _, _, params| {
                let namespace = params.child_value(0).get::<String>().unwrap_or_default();
                let key = params.child_value(1).get::<String>().unwrap_or_default();
                if namespace == "org.freedesktop.appearance" && key == "color-scheme"
                    && let Some(b) = weak.upgrade() {
                        b.apply_look();
                    }
            },
        );
    }

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

    /// The column's foot: Settings, then one button per space and a +.
    pub(crate) fn column_foot() -> (gtk::Box, gtk::Button, gtk::Box) {
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        foot.add_css_class("foot");
        let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings (Ctrl+,)"));
        settings.set_focusable(false);
        // Never expanding: an expanding child makes the whole column take
        // half the window.
        let spaces = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        foot.append(&settings);
        foot.append(&spaces);
        (foot, settings, spaces)
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
        if self.config.borrow().top_tabs {
            self.render_top();
            self.render_player();
            return;
        }
        let icons = self.config.borrow().icon_column;
        self.row_of.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        while let Some(child) = self.pins.first_child() {
            self.pins.remove(&child);
        }
        let tabs = self.tabs.borrow();
        let mut any_pin = false;
        let mut pin_at = 0;
        let mut in_group: Option<rippl_core::tabs::GroupId> = None;
        let mut folders_drawn: Vec<String> = Vec::new();
        // Only the space on screen; a group's tabs under its heading.
        for tab in tabs.here() {
            let id = tab.id;
            if !tab.pinned && tab.group != in_group {
                in_group = tab.group;
                if let Some(group) = tab.group.and_then(|g| tabs.groups.iter().find(|x| x.id == g)) {
                    let count = tabs.here().filter(|t| t.group == Some(group.id)).count();
                    self.list.append(&self.group_heading(group, count));
                }
            }
            let folded = tab.group.and_then(|g| tabs.groups.iter().find(|x| x.id == g)).is_some_and(|g| g.collapsed);
            if folded && !tab.pinned {
                continue;
            }
            let live = Some(id) == tabs.front;
            // A pin in a folder: the folder's square, once, where its
            // first pin would be.
            if tab.pinned && let Some(folder) = &tab.pin_folder {
                any_pin = true;
                if !folders_drawn.contains(folder) {
                    folders_drawn.push(folder.clone());
                    let members: Vec<TabId> =
                        tabs.here().filter(|t| t.pinned && t.pin_folder.as_ref() == Some(folder)).map(|t| t.id).collect();
                    let holds_front = tabs.front.is_some_and(|f| members.contains(&f));
                    let square = self.pin_folder_square(folder, &members, holds_front);
                    let across = if icons { 1 } else { 6 };
                    self.pins.attach(&square, pin_at % across, pin_at / across, 1, 1);
                    pin_at += 1;
                }
                continue;
            }
            if tab.pinned {
                any_pin = true;
                let pin = gtk::Button::new();
                // What the page says is waiting, hung from the pin's corner
                //: a pin shows no title to say it.
                let mark: gtk::Widget = if tab.loading && tab.life == Life::Awake {
                    crate::loader::ring(14).upcast()
                } else {
                    gtk::Label::new(Some(&monogram(&tab.url, &tab.title))).upcast()
                };
                let face = gtk::Overlay::builder().child(&mark).build();
                if let Some(count) = rippl_core::tabs::unread(&tab.title).filter(|_| tab.life == Life::Awake) {
                    // Centred on the letter's top right corner, a touch
                    // outside it, as an app's count hangs from its icon in
                    // on a wide pin the square's corner is far off.
                    let badge = gtk::Label::builder()
                        .label(&count)
                        .halign(gtk::Align::Center)
                        .valign(gtk::Align::Center)
                        .margin_start(20)
                        .margin_bottom(13)
                        .build();
                    badge.add_css_class("unread");
                    badge.set_can_target(false);
                    face.add_overlay(&badge);
                }
                pin.set_child(Some(&face));
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
                self.mark_fresh(id, pin.upcast_ref());
                let across = if icons { 1 } else { 6 };
                self.pins.attach(&pin, pin_at % across, pin_at / across, 1, 1);
                pin_at += 1;
                continue;
            }
            let row = gtk::ListBoxRow::new();
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let loading = tab.loading && tab.life == Life::Awake && !tab.url.is_empty();
            if loading && icons {
                // Icons only: no ×, so the ring takes the icon's place.
                line.append(&crate::loader::ring(15));
            } else if !tab.url.is_empty() {
                // The site's icon when known, its letter until then.
                match self.icon_for(id, &tab.url).filter(|_| !tab.private) {
                    Some(icon) => {
                        let image = gtk::Image::from_paintable(Some(&icon));
                        image.set_pixel_size(15);
                        image.add_css_class("icon");
                        image.set_valign(gtk::Align::Center);
                        line.append(&image);
                    }
                    None => {
                        let mark =
                            gtk::Label::new(Some(&if tab.private { "◐".to_string() } else { monogram(&tab.url, &tab.title) }));
                        mark.add_css_class("mark");
                        mark.set_valign(gtk::Align::Center);
                        line.append(&mark);
                    }
                }
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
            if icons {
                // A tile: the mark alone, its title a tooltip.
                line.set_halign(gtk::Align::Center);
                if tab.url.is_empty() {
                    line.append(&gtk::Image::from_icon_name("list-add-symbolic"));
                }
                row.add_css_class("tile");
            } else {
                line.append(&label);
                // Loading: the ring where the × comes; under the pointer,
                // the × again.
                let slot = gtk::Overlay::builder().child(&close).build();
                if loading {
                    let ring = crate::loader::ring(13);
                    ring.set_can_target(false);
                    slot.add_overlay(&ring);
                    row.add_css_class("loading");
                }
                line.append(&slot);
            }
            row.set_child(Some(&line));
            row.set_tooltip_text(Some(&if icons { format!("{}\n{}", tab.title, tab.url) } else { tab.url.clone() }));
            if live {
                row.add_css_class("front");
            }
            if tab.life != Life::Awake && !tab.url.is_empty() {
                row.add_css_class("asleep");
            }
            if tab.group.is_some() {
                row.add_css_class("in-group");
            }
            if self.agent_tabs.borrow().contains(&id) {
                row.add_css_class("agent");
                row.set_tooltip_text(Some(&format!("{} · an agent's tab", tab.url)));
                label.set_label(&format!("✦ {}", label.label()));
            }
            if self.in_split(id) {
                row.add_css_class("split");
                row.set_tooltip_text(Some(&format!("{} · side by side (Ctrl+Alt+S separates)", tab.url)));
            }
            let weak = self.weak();
            close.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.close_animated(id);
                }
            });
            self.on_click(&row, id);
            self.mark_fresh(id, row.upcast_ref());
            if self.closing.borrow().contains(&id) {
                row.add_css_class("leaving");
            }
            self.list.append(&row);
        }
        self.pins.set_visible(any_pin);
        self.settle_fresh();
        self.place_settings_rail();
        self.render_player();
        drop(tabs);
        self.render_spaces();
    }

    /// A group's heading: its name, a fold mark, and how many tabs; a
    /// click folds or unfolds it, a right-click renames it.
    fn group_heading(&self, group: &rippl_core::tabs::Group, count: usize) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.add_css_class("group-head");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let mark = gtk::Label::new(Some(if group.collapsed { "▸" } else { "▾" }));
        let name = gtk::Label::builder()
            .label(&group.name)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(1)
            .build();
        let n = gtk::Label::new(Some(&count.to_string()));
        n.add_css_class("dim");
        line.append(&mark);
        line.append(&name);
        line.append(&n);
        row.set_child(Some(&line));
        let gid = group.id;
        let click = gtk::GestureClick::builder().button(0).build();
        let weak = self.weak();
        click.connect_released(move |gesture, _, _, _| {
            let Some(b) = weak.upgrade() else { return };
            let button = gesture.current_button();
            let anchor = gesture.widget();
            glib::idle_add_local_once(move || {
                if button == 3 {
                    if let Some(w) = anchor {
                        b.group_menu(gid, &w);
                    }
                } else {
                    b.tabs.borrow_mut().toggle_group(gid);
                    b.dirty.set(true);
                    b.render();
                }
            });
        });
        row.add_controller(click);
        row
    }

    /// Right-click on a group's heading: rename it, or ungroup its tabs.
    fn group_menu(&self, group: rippl_core::tabs::GroupId, anchor: &gtk::Widget) {
        let name = self.tabs.borrow().groups.iter().find(|g| g.id == group).map(|g| g.name.clone()).unwrap_or_default();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 6);
        menu.set_margin_top(8);
        menu.set_margin_bottom(8);
        menu.set_margin_start(8);
        menu.set_margin_end(8);
        let entry = gtk::Entry::builder().text(&name).placeholder_text("Group name").build();
        let ungroup = gtk::Button::with_label("Ungroup tabs");
        ungroup.add_css_class("flat");
        ungroup.add_css_class("menu-item");
        menu.append(&entry);
        menu.append(&ungroup);
        let popover = gtk::Popover::builder().child(&menu).has_arrow(false).build();
        popover.add_css_class("tab-menu");
        popover.set_parent(anchor);
        popover.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        let weak = self.weak();
        let pop = popover.clone();
        entry.connect_activate(move |e| {
            let Some(b) = weak.upgrade() else { return };
            b.tabs.borrow_mut().rename_group(group, &e.text());
            pop.popdown();
            b.dirty.set(true);
            glib::idle_add_local_once(move || b.render());
        });
        let weak = self.weak();
        let pop = popover.clone();
        ungroup.connect_clicked(move |_| {
            let Some(b) = weak.upgrade() else { return };
            pop.popdown();
            let members: Vec<rippl_core::tabs::TabId> =
                b.tabs.borrow().list.iter().filter(|t| t.group == Some(group)).map(|t| t.id).collect();
            for id in members {
                b.tabs.borrow_mut().set_group(id, None);
            }
            b.dirty.set(true);
            glib::idle_add_local_once(move || b.render());
        });
        popover.popup();
        entry.grab_focus();
    }

    /// Click: select. Middle-click: close. Alt-click: beside the front tab.
    /// A tab seen in the column for the first time slides in (motion.rs);
    /// at start-up nothing does.
    pub(crate) fn mark_fresh(&self, id: rippl_core::tabs::TabId, w: &gtk::Widget) {
        self.row_of.borrow_mut().insert(id, w.clone());
        if self.rendered_once.get() && !self.seen_rows.borrow().contains(&id) {
            w.add_css_class("fresh");
        }
    }

    /// After a drawing: every tab drawn now has been seen.
    pub(crate) fn settle_fresh(&self) {
        let ids: std::collections::HashSet<_> = self.row_of.borrow().keys().copied().collect();
        *self.seen_rows.borrow_mut() = ids;
        self.rendered_once.set(true);
    }

    pub(crate) fn on_click(&self, widget: &impl IsA<gtk::Widget>, id: rippl_core::tabs::TabId) {
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
                (2, _) => b.close_animated(id),
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
