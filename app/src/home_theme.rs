//! What is behind the new tab's field: nothing (`home_theme = off`), a painted
//! wall, a solid colour, a gradient, or your own image.
//!
//! - **Walls** are painted in code, with cairo: Open Road,
//!   Mountain Lake, Northern Lights. Seeded, so a wall is the same wall
//!   every time; the dark look takes the same pigments down (a night scene
//!   less). Painted once at 1920×1200 into rippl's cache as a PNG and shown
//!   as the page's background, covering it: nothing redraws or animates.
//! - **Solid** and **gradient** use `home_colour` (`#rrggbb`), faint on the
//!   page's ground so the field still reads.
//! - **Image**: `home_image = /path/to/picture`, covering the page.

use std::f64::consts::PI;
use std::path::PathBuf;

use webkit6::gtk::{cairo, gdk, glib};
use webkit6::prelude::*;

use crate::browser::Browser;

const W: f64 = 1920.0;
const H: f64 = 1200.0;

type Rgb = (f64, f64, f64);

/// A pen: a pigment for the light or dark look.
struct Pen {
    dark: bool,
    night: bool,
}

impl Pen {
    fn ink(&self, c: Rgb) -> Rgb {
        let mut c = c;
        if self.night && !self.dark {
            c = (c.0 * 0.9 + 0.08, c.1 * 0.9 + 0.08, c.2 * 0.9 + 0.08);
        }
        let k = if self.dark { if self.night { 0.8 } else { 0.42 } } else { 1.0 };
        (c.0 * k, c.1 * k, c.2 * k)
    }
}

/// One canvas: its pen and its seeded generator.
struct Scene<'a> {
    g: &'a cairo::Context,
    pen: Pen,
    state: u64,
}

impl Scene<'_> {
    fn rand(&mut self, lo: f64, hi: f64) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        lo + (self.state % 1_000_000) as f64 / 1_000_000.0 * (hi - lo)
    }

    fn colour(&self, c: Rgb, a: f64) {
        let c = self.pen.ink(c);
        self.g.set_source_rgba(c.0, c.1, c.2, a);
    }

    fn fill(&self, c: Rgb, a: f64) {
        self.colour(c, a);
        let _ = self.g.fill();
    }

    fn poly(&self, pts: &[(f64, f64)]) {
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 { self.g.move_to(x * W, y * H) } else { self.g.line_to(x * W, y * H) }
        }
        self.g.close_path();
    }

    fn rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.g.rectangle(x * W, y * H, w * W, h * H);
    }

    fn oval(&self, x: f64, y: f64, rx: f64, ry: f64) {
        self.g.save().ok();
        self.g.translate(x * W, y * H);
        self.g.scale(rx * W, ry * H);
        self.g.arc(0.0, 0.0, 1.0, 0.0, 2.0 * PI);
        self.g.restore().ok();
    }

    fn sky(&self, cols: &[Rgb], to: f64) {
        let grad = cairo::LinearGradient::new(0.0, 0.0, 0.0, H * to);
        for (i, c) in cols.iter().enumerate() {
            let c = self.pen.ink(*c);
            grad.add_color_stop_rgb(i as f64 / (cols.len() - 1).max(1) as f64, c.0, c.1, c.2);
        }
        self.g.rectangle(-2.0, -2.0, W + 4.0, H + 4.0);
        let _ = self.g.set_source(&grad);
        let _ = self.g.fill();
    }

    fn ridge_y(u: f64, base: f64, amp: f64, freq: f64, phase: f64) -> f64 {
        let a = (u * freq * 2.0 * PI + phase).sin();
        let b = (u * freq * 5.4 * PI / 2.0 + phase * 1.7).sin();
        base + amp * (0.6 * a + 0.4 * b)
    }

    fn ridge(&self, base: f64, amp: f64, freq: f64, phase: f64) {
        self.g.move_to(-4.0, H * 1.05);
        for i in 0..=64 {
            let u = i as f64 / 64.0;
            self.g.line_to(-4.0 + u * (W + 8.0), Self::ridge_y(u, base, amp, freq, phase) * H);
        }
        self.g.line_to(W + 4.0, H * 1.05);
        self.g.close_path();
    }

    fn cloud(&self, x: f64, y: f64, s: f64, a: f64) {
        for (dx, dy, rx, ry) in [(0.0, 0.0, 0.07, 0.022), (-0.03, -0.016, 0.04, 0.022), (0.03, -0.012, 0.045, 0.02)] {
            self.oval(x + dx * s, y + dy * s, rx * s, ry * s);
            self.fill((1.0, 1.0, 1.0), a);
        }
    }

    fn pine(&self, x: f64, base: f64, height: f64, c: Rgb) {
        let tw = height * 0.16 * H / W;
        self.rect(x - tw / 2.0, base - height * 0.12, tw, height * 0.14);
        self.fill(c, 1.0);
        for k in 0..3 {
            let kf = k as f64;
            let top = base - height + kf * height * 0.24;
            let bot = base - height * 0.1 - (2.0 - kf) * height * 0.2;
            let half = (0.22 + 0.16 * kf) * height * H / W * 0.9;
            self.poly(&[(x, top), (x + half, bot), (x - half, bot)]);
            self.fill(c, 1.0);
        }
    }

    fn stars(&mut self, n: usize, top: f64, bottom: f64) {
        for _ in 0..n {
            let (x, y, r, a) = (self.rand(0.0, 1.0) * W, self.rand(top, bottom) * H, self.rand(0.5, 1.6) * 1.3, self.rand(0.4, 0.95));
            self.g.arc(x, y, r, 0.0, 2.0 * PI);
            self.fill((1.0, 1.0, 0.92), a);
        }
    }

    fn grain(&mut self) {
        for i in 0..240 {
            let (x, y, r) = (self.rand(0.0, 1.0) * W, self.rand(0.0, 1.0) * H, self.rand(0.4, 1.0) * 2.0);
            self.g.arc(x, y, r, 0.0, 2.0 * PI);
            if i % 2 == 0 { self.g.set_source_rgba(0.0, 0.0, 0.0, 0.035) } else { self.g.set_source_rgba(1.0, 1.0, 1.0, 0.05) }
            let _ = self.g.fill();
        }
    }

    fn open_road(&mut self) {
        self.sky(&[(0.58, 0.78, 0.92), (0.80, 0.90, 0.94), (0.99, 0.93, 0.82)], 0.65);
        self.cloud(0.2, 0.2, 1.4, 0.7);
        self.cloud(0.78, 0.14, 1.1, 0.6);
        self.cloud(0.55, 0.32, 0.8, 0.45);
        self.ridge(0.62, 0.02, 1.3, 0.5);
        self.fill((0.66, 0.78, 0.68), 1.0);
        self.ridge(0.645, 0.016, 1.9, 2.0);
        self.fill((0.52, 0.70, 0.48), 1.0);
        let hz = 0.65;
        self.rect(-0.01, hz, 1.02, 1.0 - hz + 0.02);
        self.fill((0.55, 0.72, 0.36), 1.0);
        let tones = [(0.62, 0.77, 0.38), (0.80, 0.74, 0.40), (0.48, 0.66, 0.34)];
        for i in -12i32..12 {
            let x0 = 0.5 + i as f64 * 0.22;
            let x1 = x0 + 0.22;
            self.poly(&[(0.5, hz), (x0 + (x0 - 0.5) * 3.0, 1.05), (x1 + (x1 - 0.5) * 3.0, 1.05)]);
            self.fill(tones[i.rem_euclid(3) as usize], if i % 2 == 0 { 0.9 } else { 0.5 });
        }
        for k in 0..9 {
            let t0 = (k as f64 / 9.0).powi(2);
            let t1 = ((k as f64 + 0.5) / 9.0).powi(2);
            self.rect(-0.01, hz + (1.0 - hz) * t0, 1.02, (1.0 - hz) * (t1 - t0));
            self.fill((1.0, 1.0, 0.8), 0.1);
        }
        self.poly(&[(0.496, hz), (0.504, hz), (0.67, 1.02), (0.33, 1.02)]);
        self.fill((0.42, 0.42, 0.46), 1.0);
        let n = 14;
        for k in (0..n).step_by(2) {
            let t0 = (k as f64 / n as f64).powi(2);
            let t1 = ((k as f64 + 1.0) / n as f64).powi(2);
            let (w0, w1) = (0.002 + 0.007 * t0, 0.002 + 0.007 * t1);
            let (y0, y1) = (hz + (1.02 - hz) * t0, hz + (1.02 - hz) * t1);
            self.poly(&[(0.5 - w0, y0), (0.5 + w0, y0), (0.5 + w1, y1), (0.5 - w1, y1)]);
            self.fill((0.96, 0.9, 0.62), 1.0);
        }
    }

    fn peak(&self, cx: f64, wd: f64, ph: f64, wh: f64) {
        self.poly(&[
            (cx - wd, wh),
            (cx - wd * 0.4, wh - ph * 0.62),
            (cx - wd * 0.12, wh - ph * 0.78),
            (cx, wh - ph),
            (cx + wd * 0.2, wh - ph * 0.72),
            (cx + wd * 0.5, wh - ph * 0.5),
            (cx + wd, wh),
        ]);
    }

    fn cap(&self, cx: f64, wd: f64, ph: f64, wh: f64) {
        self.poly(&[
            (cx - wd * 0.3, wh - ph * 0.64),
            (cx - wd * 0.12, wh - ph * 0.78),
            (cx, wh - ph),
            (cx + wd * 0.2, wh - ph * 0.72),
            (cx + wd * 0.28, wh - ph * 0.62),
            (cx + wd * 0.12, wh - ph * 0.68),
            (cx, wh - ph * 0.6),
            (cx - wd * 0.1, wh - ph * 0.68),
        ]);
    }

    fn mountain_lake(&mut self) {
        let wh = 0.62;
        self.sky(&[(0.72, 0.80, 0.95), (0.97, 0.86, 0.82), (0.99, 0.90, 0.78)], wh);
        let far = [(0.14, 0.3, 0.3), (0.45, 0.28, 0.15), (0.8, 0.32, 0.3)];
        let near = [(0.03, 0.28, 0.34), (0.3, 0.24, 0.2), (0.7, 0.22, 0.17), (0.96, 0.3, 0.4)];
        let (far_c, near_c) = ((0.62, 0.68, 0.84), (0.38, 0.48, 0.64));
        let mountains = |s: &Scene, alpha: (f64, f64, f64)| {
            for p in far {
                s.peak(p.0, p.1, p.2, wh);
                s.fill(far_c, alpha.0);
                s.cap(p.0, p.1, p.2, wh);
                s.fill((0.96, 0.96, 1.0), alpha.0);
            }
            for p in near {
                s.peak(p.0, p.1, p.2, wh);
                s.fill(near_c, alpha.1);
            }
            for p in near {
                s.cap(p.0, p.1, p.2, wh);
                s.fill((1.0, 1.0, 1.0), alpha.2);
            }
        };
        mountains(self, (1.0, 1.0, 1.0));
        // Still water, with the peaks upside down in it.
        let water = cairo::LinearGradient::new(0.0, wh * H, 0.0, H);
        let (a, b) = (self.pen.ink((0.58, 0.72, 0.82)), self.pen.ink((0.32, 0.50, 0.64)));
        water.add_color_stop_rgb(0.0, a.0, a.1, a.2);
        water.add_color_stop_rgb(1.0, b.0, b.1, b.2);
        self.g.rectangle(-2.0, wh * H, W + 4.0, H);
        let _ = self.g.set_source(&water);
        let _ = self.g.fill();
        self.g.save().ok();
        self.g.rectangle(0.0, wh * H, W, H);
        self.g.clip();
        self.g.translate(0.0, 2.0 * wh * H);
        self.g.scale(1.0, -1.0);
        mountains(self, (0.0, 0.5, 0.3));
        self.g.restore().ok();
        for _ in 0..14 {
            let (y, x, l) = (self.rand(wh + 0.03, 0.95), self.rand(0.1, 0.8), self.rand(0.04, 0.12));
            self.rect(x, y, l, 0.003 + 1.0 / H);
            self.fill((1.0, 1.0, 1.0), 0.3);
        }
        let shore = (0.20, 0.34, 0.30);
        self.ridge(0.94, 0.02, 1.4, 0.3);
        self.fill(shore, 1.0);
        for (x, hgt, b) in
            [(0.03, 0.34, 0.95), (0.09, 0.26, 0.96), (0.16, 0.2, 0.94), (0.22, 0.1, 0.9), (0.97, 0.36, 0.95), (0.9, 0.28, 0.96), (0.83, 0.2, 0.94), (0.77, 0.09, 0.9)]
        {
            self.pine(x, b, hgt, shore);
        }
        self.pine(0.04, 0.66, 0.1, (0.28, 0.40, 0.38));
        self.pine(0.95, 0.66, 0.09, (0.28, 0.40, 0.38));
    }

    fn northern_lights(&mut self) {
        self.sky(&[(0.02, 0.05, 0.15), (0.05, 0.14, 0.26), (0.08, 0.24, 0.32)], 0.8);
        self.stars(130, 0.0, 0.6);
        let ribbons = [
            (0.16, 0.1, 1.3, 0.4, (0.35, 0.95, 0.65)),
            (0.26, 0.12, 1.7, 2.3, (0.30, 0.85, 0.85)),
            (0.12, 0.08, 2.1, 4.4, (0.55, 0.45, 0.92)),
            (0.32, 0.09, 1.2, 1.1, (0.40, 0.92, 0.55)),
        ];
        // Each ribbon is laid down a few times, faint and a little apart,
        // which reads as a blur without blurring.
        for (y0, ht, f, ph, c) in ribbons {
            for spread in [-0.012f64, -0.006, 0.0, 0.006, 0.012] {
                for i in 0..=60 {
                    let u = i as f64 / 60.0;
                    let y = (y0 + spread + 0.07 * (u * f * 2.0 * PI + ph).sin()) * H;
                    if i == 0 { self.g.move_to(u * W, y) } else { self.g.line_to(u * W, y) }
                }
                for i in (0..=60).rev() {
                    let u = i as f64 / 60.0;
                    let top = y0 + spread + 0.07 * (u * f * 2.0 * PI + ph).sin();
                    self.g.line_to(u * W, (top + ht * (0.7 + 0.3 * (u * 9.0 + ph).sin())) * H);
                }
                self.g.close_path();
                let grad = cairo::LinearGradient::new(0.0, (y0 - 0.07) * H, 0.0, (y0 + ht + 0.07) * H);
                let (a, z) = (self.pen.ink(c), self.pen.ink((0.3, 0.8, 0.7)));
                grad.add_color_stop_rgba(0.0, a.0, a.1, a.2, 0.0);
                grad.add_color_stop_rgba(0.5, a.0, a.1, a.2, 0.6 / 5.0);
                grad.add_color_stop_rgba(1.0, z.0, z.1, z.2, 0.0);
                let _ = self.g.set_source(&grad);
                let _ = self.g.fill();
            }
        }
        self.ridge(0.68, 0.05, 1.2, 1.0);
        self.fill((0.30, 0.40, 0.56), 1.0);
        self.ridge(0.79, 0.035, 1.7, 3.2);
        self.fill((0.72, 0.80, 0.92), 1.0);
        let cx = 0.8;
        let cy = Self::ridge_y(0.8, 0.79, 0.035, 1.7, 3.2);
        self.rect(cx - 0.028, cy - 0.05, 0.056, 0.058);
        self.fill((0.30, 0.20, 0.20), 1.0);
        self.poly(&[(cx - 0.036, cy - 0.048), (cx, cy - 0.095), (cx + 0.036, cy - 0.048)]);
        self.fill((0.20, 0.14, 0.16), 1.0);
        self.rect(cx + 0.012, cy - 0.095, 0.01, 0.03);
        self.fill((0.20, 0.14, 0.16), 1.0);
        self.g.arc((cx - 0.005) * W, (cy - 0.028) * H, 0.026 * H, 0.0, 2.0 * PI);
        self.fill((1.0, 0.85, 0.4), 0.25);
        self.rect(cx - 0.014, cy - 0.036, 0.018, 0.02);
        self.fill((1.0, 0.85, 0.45), 1.0);
        self.pine(cx - 0.07, cy + 0.01, 0.1, (0.14, 0.22, 0.26));
        self.pine(cx + 0.075, cy + 0.01, 0.13, (0.14, 0.22, 0.26));
        self.ridge(0.92, 0.025, 1.3, 5.0);
        self.fill((0.86, 0.92, 1.0), 1.0);
    }
}

/// The seed for a theme: from its name, so each wall is its own.
fn seed(name: &str) -> u64 {
    name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)) | 1
}

/// `theme` painted for the light or dark look, as a PNG in the cache
/// (made once).
fn wall(theme: &str, dark: bool) -> Option<PathBuf> {
    let file = crate::engine::cache_dir().join("walls").join(format!("{theme}-{}.png", if dark { "dark" } else { "light" }));
    if file.exists() {
        return Some(file);
    }
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, W as i32, H as i32).ok()?;
    {
        let g = cairo::Context::new(&surface).ok()?;
        let mut scene = Scene { g: &g, pen: Pen { dark, night: theme == "northern-lights" }, state: seed(theme) };
        match theme {
            "open-road" => scene.open_road(),
            "mountain-lake" => scene.mountain_lake(),
            "northern-lights" => scene.northern_lights(),
            _ => return None,
        }
        scene.grain();
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?.to_vec();
    let bytes = glib::Bytes::from_owned(data);
    let texture = gdk::MemoryTexture::new(W as i32, H as i32, gdk::MemoryFormat::B8g8r8a8Premultiplied, &bytes, stride);
    std::fs::create_dir_all(file.parent()?).ok()?;
    texture.save_to_png(&file).ok()?;
    Some(file)
}

/// A file's path as a CSS url.
fn css_url(path: &std::path::Path) -> String {
    let uri = glib::filename_to_uri(path, None).map(|u| u.to_string()).unwrap_or_default();
    format!("url(\"{}\")", uri.replace('"', "%22"))
}

impl Browser {
    /// Puts the chosen theme behind the new tab's field.
    pub(crate) fn apply_home_theme(&self) {
        let (theme, colour, image) = {
            let c = self.config.borrow();
            (c.home_theme.clone(), c.home_colour.clone(), c.home_image.clone())
        };
        let dark = self.window.has_css_class("dark");
        let rule = |light: String, dark: String| {
            format!(
                "window.rippl .home-bg {{ {light} }}\nwindow.rippl.dark .home-bg {{ {dark} }}\n"
            )
        };
        let cover = "background-size: cover; background-position: center bottom; background-repeat: no-repeat;";
        let css = match theme.as_str() {
            "solid" => rule(format!("background: mix(#ffffff, {colour}, 0.16);"), format!("background: mix(#1c1c1c, {colour}, 0.22);")),
            "gradient" => rule(
                format!("background: linear-gradient(160deg, mix(#ffffff, {colour}, 0.30), mix(#ffffff, {colour}, 0.04));"),
                format!("background: linear-gradient(160deg, mix(#1c1c1c, {colour}, 0.36), mix(#1c1c1c, {colour}, 0.06));"),
            ),
            "image" if !image.is_empty() && std::path::Path::new(&image).is_file() => {
                let url = css_url(std::path::Path::new(&image));
                rule(format!("background-image: {url}; {cover}"), format!("background-image: {url}; {cover}"))
            }
            "off" | "image" => String::new(),
            wall_name => match wall(wall_name, dark) {
                Some(file) => {
                    let url = css_url(&file);
                    format!("window.rippl .home-bg {{ background-image: {url}; {cover} }}\n")
                }
                None => String::new(),
            },
        };
        if css.is_empty() {
            self.blank.remove_css_class("home-bg");
        } else {
            self.blank.add_css_class("home-bg");
        }
        self.home_css.load_from_string(&css);
    }
}
