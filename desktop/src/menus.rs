//! Native menus: a menu shown at a point of the window, and on macOS the
//! menu bar (whose Edit menu is what makes ⌘C and ⌘V work in pages).

use muda::{ContextMenu, Menu};
use tao::window::Window;

pub fn show(menu: &Menu, window: &Window, pos: Option<muda::dpi::Position>) {
    #[cfg(target_os = "macos")]
    unsafe {
        use tao::platform::macos::WindowExtMacOS;
        menu.show_context_menu_for_nsview(window.ns_view() as _, pos);
    }
    #[cfg(windows)]
    unsafe {
        use tao::platform::windows::WindowExtWindows;
        menu.show_context_menu_for_hwnd(window.hwnd() as _, pos);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use tao::platform::unix::WindowExtUnix;
        menu.show_context_menu_for_gtk_window(window.gtk_window().as_ref(), pos);
    }
}

/// The menu bar (macOS): rippl, Edit, View, Window, with their keys.
#[cfg(target_os = "macos")]
pub fn menu_bar() -> Menu {
    use muda::{AboutMetadata, PredefinedMenuItem as P, Submenu};
    let bar = Menu::new();
    let app = Submenu::new("rippl", true);
    let _ = app.append_items(&[
        &P::about(None, Some(AboutMetadata { name: Some("rippl".into()), version: Some(env!("CARGO_PKG_VERSION").into()), ..Default::default() })),
        &P::separator(),
        &P::services(None),
        &P::separator(),
        &P::hide(None),
        &P::hide_others(None),
        &P::show_all(None),
        &P::separator(),
        &P::quit(None),
    ]);
    let edit = Submenu::new("Edit", true);
    let _ = edit.append_items(&[&P::undo(None), &P::redo(None), &P::separator(), &P::cut(None), &P::copy(None), &P::paste(None), &P::select_all(None)]);
    let window = Submenu::new("Window", true);
    let _ = window.append_items(&[&P::minimize(None), &P::maximize(None), &P::fullscreen(None), &P::separator(), &P::bring_all_to_front(None)]);
    let _ = bar.append_items(&[&app, &edit, &window]);
    bar
}
