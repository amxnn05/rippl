//! The ring a tab shows in its icon's place while its page loads (in the
//! mark's place, as on a pin): a thin arc going
//! round a faint track, in the tab's own ink. Every ring is timed off the
//! frame clock, so all loading tabs turn together, and a redrawn column
//! doesn't make one jump. It costs frames only while something loads: the
//! ring goes with the tab's row when loading stops.

use std::f64::consts::PI;

use webkit6::gtk;
use webkit6::prelude::*;

/// One turn, in microseconds of the frame clock.
const TURN: i64 = 800_000;

/// A loading ring `size` pixels across.
pub(crate) fn ring(size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_width(size).content_height(size).build();
    area.add_css_class("loader");
    area.set_valign(gtk::Align::Center);
    area.set_halign(gtk::Align::Center);
    area.set_draw_func(|area, cr, w, h| {
        let ink = area.color();
        let (w, h) = (f64::from(w), f64::from(h));
        let line = (w.min(h) * 0.13).max(1.4);
        let r = w.min(h) / 2.0 - line / 2.0 - 0.5;
        let (cx, cy) = (w / 2.0, h / 2.0);
        // The track.
        cr.set_line_width(line);
        cr.set_source_rgba(ink.red().into(), ink.green().into(), ink.blue().into(), 0.16);
        cr.arc(cx, cy, r, 0.0, 2.0 * PI);
        let _ = cr.stroke();
        // The arc, a third of the way round, turning.
        let t = area.frame_clock().map_or(0, |c| c.frame_time());
        let start = (t % TURN) as f64 / TURN as f64 * 2.0 * PI - PI / 2.0;
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.set_source_rgba(ink.red().into(), ink.green().into(), ink.blue().into(), 0.85);
        cr.arc(cx, cy, r, start, start + 2.0 * PI * 0.3);
        let _ = cr.stroke();
    });
    area.add_tick_callback(|area, _| {
        area.queue_draw();
        webkit6::glib::ControlFlow::Continue
    });
    area
}
