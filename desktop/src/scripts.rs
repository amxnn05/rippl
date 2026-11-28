//! The script every page gets (ui/page.js, with the keys to listen for),
//! and the ones the settings turn on, shared with the GTK build: YouTube's
//! ads, video keys, autoscroll, elastic edges. WKWebView and WebView2 have
//! no separate script world through wry, so these run in the page's.

use rippl_core::config::Config;
use rippl_core::keys::Key;

const YOUTUBE_HOSTS: [&str; 6] =
    ["youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtube-nocookie.com", "www.youtube-nocookie.com"];
const YOUTUBE_PAGE: &str = include_str!("../../app/src/youtube_page.js");
const YOUTUBE_SKIPPER: &str = include_str!("../../app/src/youtube_skipper.js");
const AUTOSCROLL: &str = include_str!("../../app/src/autoscroll.js");
const ELASTIC: &str = include_str!("../../app/src/elastic.js");
const VIDEO_KEYS: &str = r#"
(() => {
  if (window.__ripplVideoKeys) return;
  window.__ripplVideoKeys = true;
  window.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
    const media = e.target;
    if (!(media instanceof HTMLMediaElement) || !media.controls) return;
    if (document.fullscreenElement || media.webkitDisplayingFullscreen) return;
    if (e.key === ' ' && !e.shiftKey) {
      if (media.paused || media.ended) media.play().catch(() => {}); else media.pause();
    } else if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && !e.shiftKey && isFinite(media.duration)) {
      const to = media.currentTime + (e.key === 'ArrowRight' ? 5 : -5);
      media.currentTime = Math.max(0, Math.min(media.duration, to));
    } else return;
    e.preventDefault();
  });
})();
"#;

/// Runs `js` once the page's document is there (the GTK build's
/// "document end").
fn at_end(js: &str) -> String {
    format!("(() => {{ const go = () => {{ {js} }}; if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go, {{ once: true }}); else go(); }})();\n")
}

pub fn page_script(keymap: &[(String, Key)], config: &Config, shield_off_youtube: bool) -> String {
    let keys = crate::commands::keymap_json(keymap).to_string();
    let mut js = String::new();
    if config.peek {
        js.push_str("window.__ripplPeek = true;\n");
    }
    js.push_str(&include_str!("../ui/page.js").replace("__RIPPL_KEYS__", &keys));
    if config.shield && !shield_off_youtube {
        let hosts = serde_json::to_string(&YOUTUBE_HOSTS).unwrap_or_else(|_| "[]".into());
        js.push_str(&YOUTUBE_PAGE.replace("__HOSTS__", &hosts));
        js.push_str(&YOUTUBE_SKIPPER.replace("__HOSTS__", &hosts));
    }
    // macOS has its own elastic edges; elsewhere, rippl's.
    if !cfg!(target_os = "macos") && config.elastic && config.motion_full {
        js.push_str(&at_end(ELASTIC));
    }
    if config.autoscroll {
        js.push_str(&at_end(AUTOSCROLL));
    }
    if config.video_keys {
        js.push_str(&at_end(VIDEO_KEYS));
    }
    js
}
