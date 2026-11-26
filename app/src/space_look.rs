//! Spaces' colours and the swipe between spaces (the 
//! colours,  and ).
//!
//! - **Colour**: a space's menu (right-click its button at the column's
//!   foot) offers the five colours, and a second one to make a
//!   gradient. The chrome behind the tabs wears it faintly: the column,
//!   the strip above the page, the bookmarks bar and the ground around the
//!   page's card, never the page itself. 7% of the colour on the light
//!   ground, 11% on the dark, which swallows more; a gradient runs across
//!   the whole window at 135°. A space without one looks as before.
//! - **Swipe**: two fingers sideways on the column go to the next or
//!   previous space, one space a stroke. A mouse's sideways wheel does the
//!   same.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rippl_core::tabs::SpaceId;
use webkit6::prelude::*;
use webkit6::{glib, gtk};

use crate::browser::Browser;

/// the palette: blue, green, orange, red, violet.
pub(crate) const COLOURS: [(&str, &str); 5] =
    [("Blue", "#4285f5"), ("Green", "#33a873"), ("Orange", "#f59e33"), ("Red", "#e65466"), ("Violet", "#9e66e6")];
/// How much of the colour the chrome wears.
const WASH_LIGHT: f64 = 0.07;
const WASH_DARK: f64 = 0.11;
/// The grounds it is mixed into (look.rs's --frame).
const FRAME_LIGHT: &str = "#f4f4f4";
const FRAME_DARK: &str = "#131313";
/// How far two fingers go sideways before a space turns.
const SWIPE: f64 = 90.0;

impl Browser {
    /// Washes the window in the space on screen's colour, or takes it off.
    pub(crate) fn apply_space_look(&self) {
        let colours = {
            let tabs = self.tabs.borrow();
            tabs.spaces.iter().find(|s| s.id == tabs.space).map(|s| s.colours.clone()).unwrap_or_default()
        };
        let css = if colours.is_empty() || self.config.borrow().glass {
            String::new()
        } else {
            let mix = |base: &str, wash: f64| -> String {
                let ends: Vec<String> = colours.iter().map(|c| format!("mix({base}, {c}, {wash})")).collect();
                if ends.len() == 1 {
                    ends[0].clone()
                } else {
                    format!("linear-gradient(135deg, {})", ends.join(", "))
                }
            };
            format!(
                "window.rippl {{ background: {}; }}\nwindow.rippl.dark {{ background: {}; }}\n",
                mix(FRAME_LIGHT, WASH_LIGHT),
                mix(FRAME_DARK, WASH_DARK)
            )
        };
        self.space_css.load_from_string(&css);
    }

    /// A space's colour, chosen from its menu.
    pub(crate) fn colour_space(&self, space: SpaceId, colours: &[String]) {
        self.tabs.borrow_mut().colour_space(space, colours);
        self.dirty.set(true);
        self.apply_space_look();
        self.render_spaces();
    }

    /// The colour rows for a space's menu: its colour, and the one a
    /// gradient runs into.
    pub(crate) fn colour_rows(&self, space: SpaceId, menu: &gtk::Box) {
        let now = self.tabs.borrow().spaces.iter().find(|s| s.id == space).map(|s| s.colours.clone()).unwrap_or_default();
        for second in [false, true] {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let title = gtk::Label::new(Some(if second { "Into" } else { "Colour" }));
            title.add_css_class("swatch-title");
            title.set_width_chars(6);
            title.set_xalign(0.0);
            row.append(&title);
            let mut choices: Vec<(&str, Option<&str>)> = vec![("None", None)];
            choices.extend(COLOURS.iter().map(|(n, c)| (*n, Some(*c))));
            for (name, colour) in choices {
                let swatch = gtk::Button::new();
                swatch.add_css_class("swatch");
                swatch.set_tooltip_text(Some(name));
                swatch.set_focusable(false);
                let chosen = match (second, colour) {
                    (false, Some(c)) => now.first().map(String::as_str) == Some(c),
                    (false, None) => now.is_empty(),
                    (true, Some(c)) => now.get(1).map(String::as_str) == Some(c),
                    (true, None) => now.len() < 2,
                };
                if chosen {
                    swatch.add_css_class("chosen");
                }
                match colour {
                    Some(c) => swatch.add_css_class(&format!("swatch-{}", &c[1..])),
                    None => swatch.set_icon_name("window-close-symbolic"),
                }
                let weak = self.weak();
                let now = now.clone();
                let colour = colour.map(String::from);
                swatch.connect_clicked(move |button| {
                    let Some(b) = weak.upgrade() else { return };
                    let mut next = now.clone();
                    match (second, colour.clone()) {
                        (false, Some(c)) => {
                            next.truncate(2);
                            if next.is_empty() { next.push(c) } else { next[0] = c }
                        }
                        (false, None) => next.clear(),
                        (true, Some(c)) if !next.is_empty() => {
                            next.truncate(1);
                            next.push(c);
                        }
                        (true, Some(c)) => next = vec![c.clone(), c],
                        (true, None) => next.truncate(1),
                    }
                    if let Some(p) = button.ancestor(gtk::Popover::static_type()).and_downcast::<gtk::Popover>() {
                        p.popdown();
                    }
                    b.colour_space(space, &next);
                });
                row.append(&swatch);
            }
            menu.append(&row);
        }
    }

    /// Two fingers sideways on the column: the next or previous space.
    pub(crate) fn wire_space_swipe(&self) {
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::HORIZONTAL);
        let gathered = Rc::new(Cell::new(0.0f64));
        let taken = Rc::new(Cell::new(false));
        let last = Rc::new(Cell::new(Instant::now()));
        {
            let (gathered, taken) = (gathered.clone(), taken.clone());
            scroll.connect_scroll_begin(move |_| {
                gathered.set(0.0);
                taken.set(false);
            });
        }
        let weak = self.weak();
        scroll.connect_scroll(move |c, dx, _| {
            let Some(b) = weak.upgrade() else { return glib::Propagation::Proceed };
            // A wheel's notches (no begin and end): one stroke is a run of
            // them close together.
            if c.unit() == gtk::gdk::ScrollUnit::Wheel {
                if last.get().elapsed() > Duration::from_millis(400) {
                    gathered.set(0.0);
                    taken.set(false);
                }
                last.set(Instant::now());
                gathered.set(gathered.get() + dx * SWIPE);
            } else {
                gathered.set(gathered.get() + dx);
            }
            if !taken.get() && gathered.get().abs() >= SWIPE {
                taken.set(true);
                let step = if gathered.get() > 0.0 { 1 } else { -1 };
                let next = b.tabs.borrow().neighbour_space(step);
                if let Some(space) = next {
                    glib::idle_add_local_once(move || b.switch_space(space));
                }
            }
            glib::Propagation::Stop
        });
        self.column.add_controller(scroll);
    }
}
