//! What differs between systems: how a view is put in the window and
//! raised over the others, how `rippl://` addresses look to the engine,
//! opening files, and the engine's name. Everything else in this crate is
//! the same on every system.
//!
//! - **macOS**: WKWebView, each view an NSView in the window's content view.
//! - **Windows**: WebView2 (Edge's engine, part of Windows 10 and 11), each
//!   view a child window. WebView2 has no custom schemes, so `rippl://x`
//!   is served as `https://rippl.x` (wry's workaround).
//! - **Linux**: wry's WebKitGTK (GTK 3) port, only for developing this
//!   build; rippl on Linux is the GTK 4 build in `app/`.

use tao::window::Window;
use wry::{WebView, WebViewBuilder};

#[cfg(target_os = "macos")]
pub const NAME: &str = "macos";
#[cfg(windows)]
pub const NAME: &str = "windows";
#[cfg(all(unix, not(target_os = "macos")))]
pub const NAME: &str = "linux";

/// Where views go in a window.
pub struct Host {
    #[cfg(all(unix, not(target_os = "macos")))]
    pub fixed: gtk::Fixed,
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn host(window: &Window) -> Host {
    use gtk::prelude::*;
    use tao::platform::unix::WindowExtUnix;
    let fixed = gtk::Fixed::new();
    let vbox = window.default_vbox().expect("tao's window has a box");
    vbox.pack_start(&fixed, true, true, 0);
    fixed.show_all();
    Host { fixed }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn host(_window: &Window) -> Host {
    Host {}
}

/// A view, built into the window.
pub fn build(builder: WebViewBuilder, window: &Window, host: &Host) -> wry::Result<WebView> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use wry::WebViewBuilderExtUnix;
        let _ = window;
        let v = builder.build_gtk(&host.fixed)?;
        Ok(v)
    }
    #[cfg(windows)]
    {
        use wry::WebViewBuilderExtWindows;
        let _ = host;
        builder
            .with_https_scheme(true)
            .with_browser_accelerator_keys(false)
            .with_scroll_bar_style(wry::ScrollBarStyle::FluentOverlay)
            .build_as_child(window)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = host;
        builder.with_back_forward_navigation_gestures(true).build_as_child(window)
    }
}

/// Puts a view over every other view in the window.
#[allow(dead_code)]
pub fn raise(view: &WebView, window: &Window, host: &Host) {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use gtk::prelude::*;
        use wry::WebViewExtUnix;
        let _ = window;
        let w = view.webview();
        let (x, y) = (host.fixed.child_x(&w), host.fixed.child_y(&w));
        host.fixed.remove(&w);
        host.fixed.put(&w, x, y);
        w.show();
    }
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE};
        use wry::WebViewExtWindows;
        let _ = (window, host);
        unsafe {
            let _ = SetWindowPos(view.hwnd(), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
    #[cfg(target_os = "macos")]
    {
        use wry::WebViewExtMacOS;
        let _ = host;
        let _ = window;
        let ns = view.webview();
        unsafe {
            if let Some(parent) = ns.superview() {
                ns.removeFromSuperview();
                parent.addSubview(&ns);
            }
        }
    }
}

/// `rippl://settings?x` as the engine has it on this system.
pub fn to_engine(url: &str) -> String {
    if cfg!(windows) {
        if let Some(rest) = url.strip_prefix("rippl://") {
            return format!("https://rippl.{rest}");
        }
        if let Some(rest) = url.strip_prefix("rippl-ui://") {
            return format!("https://rippl-ui.{rest}");
        }
    }
    url.to_string()
}

/// The engine's address back as rippl shows and keeps it.
pub fn from_engine(url: &str) -> String {
    if cfg!(windows) {
        if let Some(rest) = url.strip_prefix("https://rippl.") {
            // WebView2 adds a "/" after the page's name: rippl's has none.
            let (head, tail) = rest.split_at(rest.find(['?', '#']).unwrap_or(rest.len()));
            return format!("rippl://{}{tail}", head.trim_end_matches('/'));
        }
        if let Some(rest) = url.strip_prefix("https://rippl-ui.") {
            return format!("rippl-ui://{rest}");
        }
    }
    url.to_string()
}

/// One of rippl's pages, its links made right for this system.
pub fn internal_html(html: String) -> String {
    if cfg!(windows) { html.replace("rippl://", "https://rippl.") } else { html }
}

/// The engine, as Settings › About names it.
pub fn engine_name() -> String {
    let v = wry::webview_version().unwrap_or_default();
    #[cfg(target_os = "macos")]
    return format!("WebKit (WKWebView) {v}");
    #[cfg(windows)]
    return format!("WebView2 (Microsoft Edge) {v}");
    #[cfg(all(unix, not(target_os = "macos")))]
    return format!("WebKitGTK {v} (development)");
}

/// Opens a file or folder with what the system opens it with.
pub fn open(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Shows a file in its folder, chosen.
pub fn show_in_folder(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Opens an address in the system's own handler (mailto:, tel:, apps).
pub fn open_uri(uri: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(uri).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", uri]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
}

/// Is rippl the system's default browser?
pub fn is_default_browser() -> &'static str {
    #[cfg(windows)]
    {
        // The http association's ProgId, as Windows records your choice.
        let out = std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice", "/v", "ProgId"])
            .output();
        return match out {
            Ok(o) if String::from_utf8_lossy(&o.stdout).contains("rippl") => "yes",
            _ => "no",
        };
    }
    #[cfg(not(windows))]
    "no"
}

/// Asks the system to make rippl the default browser. Both systems leave
/// the final word to you, in their own settings.
pub fn make_default_browser() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        open_uri("x-apple.systempreferences:com.apple.Desktop-Settings.extension");
        return Ok("Choose rippl under “Default web browser” in System Settings › Desktop & Dock.".into());
    }
    #[cfg(windows)]
    {
        open_uri("ms-settings:defaultapps?registeredAppUser=rippl");
        return Ok("Choose rippl as the web browser in Windows Settings › Default apps.".into());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    Err("On Linux, use rippl's GTK build to set it as the default browser.".into())
}

/// rippl's icon for the window (and the taskbar or Dock while it runs).
pub fn window_icon() -> Option<tao::window::Icon> {
    let bytes: &[u8] = include_bytes!("../../data/icons/hicolor/128x128/apps/dev.rippl.Rippl.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        _ => return None,
    };
    tao::window::Icon::from_rgba(rgba, info.width, info.height).ok()
}
