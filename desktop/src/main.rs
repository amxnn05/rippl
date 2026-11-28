//! rippl on macOS and Windows: the same browser as the GTK build on Linux
//! (`app/`), on the system's own engine. See `browser.rs` for the window.

mod actions;
mod browser;
mod clipboard;
mod commands;
mod control;
mod downloads;
mod events;
mod extras;
mod grab;
mod home;
mod internal;
mod menus;
mod net;
mod passwords;
mod platform;
mod scripts;
mod shield;
mod switcher;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;

use crate::browser::{Browser, Ev};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rippl {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let urls: Vec<String> = args.into_iter().filter(|a| !a.starts_with("--")).collect();

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = Arc::new(Mutex::new(event_loop.create_proxy()));

    #[cfg(target_os = "macos")]
    let bar = {
        let bar = menus::menu_bar();
        bar.init_for_nsapp();
        bar
    };

    let builder = WindowBuilder::new().with_title("rippl").with_inner_size(tao::dpi::LogicalSize::new(1280.0, 820.0)).with_min_inner_size(tao::dpi::LogicalSize::new(480.0, 360.0)).with_window_icon(platform::window_icon());
    #[cfg(target_os = "macos")]
    let builder = {
        use tao::platform::macos::WindowBuilderExtMacOS;
        builder.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true)
    };
    let window = builder.build(&event_loop).expect("a window");

    {
        let p = proxy.clone();
        muda::MenuEvent::set_event_handler(Some(Arc::new(move |e: muda::MenuEvent| browser::post(&p, Ev::Menu(e.id.0)))));
    }
    {
        let p = proxy.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                browser::post(&p, Ev::Tick);
            }
        });
    }

    control::start(proxy.clone());
    let b = Browser::new(window, proxy, urls);
    let started = Instant::now();
    let mut counts: std::collections::BTreeMap<String, u64> = Default::default();
    let mut shown = Instant::now();
    let trace = std::env::var_os("RIPPL_TRACE").is_some();
    event_loop.run(move |event, _target, control_flow| {
        if trace {
            let k = format!("{event:?}").chars().take(110).collect::<String>();
            *counts.entry(k).or_default() += 1;
            if shown.elapsed() > Duration::from_secs(2) {
                eprintln!("{counts:?}");
                counts.clear();
                shown = Instant::now();
            }
        }
        #[cfg(target_os = "macos")]
        let _ = &bar;
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(ev) => b.handle(ev),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => b.quit(),
            Event::WindowEvent { event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }, .. } => b.resized(),
            Event::WindowEvent { event: WindowEvent::ThemeChanged(_), .. } => b.theme_changed(),
            Event::MainEventsCleared => {
                b.run_pending();
                if b.render_due.get() {
                    b.render();
                }
                if std::env::var_os("RIPPL_DEBUG").is_some() && started.elapsed() < Duration::from_millis(50) {
                    eprintln!("rippl: up in {:?}", started.elapsed());
                }
            }
            _ => {}
        }
    });
}
