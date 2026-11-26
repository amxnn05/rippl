//! Scripts rippl puts in pages for features that can be switched: YouTube's
//! ads and the video keys. They are kept here so a change of setting (or of
//! Shield on a site) swaps them without touching anyone else's scripts.
//!
//! - **YouTube ads**, part of Shield. YouTube
//!   announces its ads in the player's own data and plays them from the
//!   video's servers, so no list stops them. On YouTube's hosts only:
//!   - a script in the page's world takes the ad schedule out of that data
//!     before the player reads it, answers YouTube's ad reports with an
//!     empty 200 (nothing is sent), and starts a held first video again;
//!   - a script in rippl's world mutes an ad that plays all the same and
//!     runs it at 16×, putting sound and speed back after it.
//!
//!   The blocking rules that go with them are compiled into Shield's filters
//!   (`rippl_core::shield::youtube_rules`). Both scripts are off with
//!   Shield, and on a YouTube host Shield is off for.
//! - **Autoscroll**: a click
//!   of the middle button on a page (not on a link, which it still opens in
//!   a new tab) leaves a mark, and the page scrolls towards the pointer,
//!   faster the further away it is. Another click, Esc or the wheel stops
//!   it; held and dragged, letting go stops it.
//! - **Autoplay**: not a script but the
//!   page's own policy, given with each navigation: muted by default (as
//!   WebKit's own), nothing until a click with `autoplay = never`, and with
//!   sound for a site whose card says so (never in a private tab).
//! - **Video keys**: Space and
//!   ← → on a video with the browser's own controls, once it has been
//!   clicked. Players a site draws itself handle their own keys.

use webkit6::{AutoplayPolicy, UserContentInjectedFrames, UserScript, UserScriptInjectionTime, WebsitePolicies};

use crate::browser::Browser;

/// YouTube's hosts: exact, as accounts.youtube.com and the like have no player.
const YOUTUBE_HOSTS: [&str; 6] =
    ["youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtube-nocookie.com", "www.youtube-nocookie.com"];

const YOUTUBE_PAGE: &str = include_str!("youtube_page.js");
const YOUTUBE_SKIPPER: &str = include_str!("youtube_skipper.js");

/// The site flag: may play sound by itself.
pub const SOUND: &str = "sound";

const AUTOSCROLL: &str = include_str!("autoscroll.js");

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

impl Browser {
    /// Puts in the scripts the settings ask for, taking out the old ones.
    /// For pages loaded from now on.
    pub(crate) fn install_page_scripts(&self) {
        let content = &self.engine.content;
        for script in self.page_scripts.borrow_mut().drain(..) {
            content.remove_script(&script);
        }
        let mut scripts = Vec::new();
        let config = self.config.borrow();
        if config.shield {
            // Each host Shield is off for keeps YouTube's ads.
            let off: Vec<String> = YOUTUBE_HOSTS
                .iter()
                .filter(|h| self.shield.is_allowed(h) || self.shield.is_allowed(h.trim_start_matches("www.")))
                .map(|h| format!("*://{h}/*"))
                .collect();
            if off.len() < YOUTUBE_HOSTS.len() {
                let hosts = serde_json::to_string(&YOUTUBE_HOSTS).unwrap_or_else(|_| "[]".into());
                let allow: Vec<String> = YOUTUBE_HOSTS.iter().map(|h| format!("*://{h}/*")).collect();
                let allow: Vec<&str> = allow.iter().map(String::as_str).collect();
                let block: Vec<&str> = off.iter().map(String::as_str).collect();
                scripts.push(UserScript::new(
                    &YOUTUBE_PAGE.replace("__HOSTS__", &hosts),
                    UserContentInjectedFrames::AllFrames,
                    UserScriptInjectionTime::Start,
                    &allow,
                    &block,
                ));
                scripts.push(UserScript::for_world(
                    &YOUTUBE_SKIPPER.replace("__HOSTS__", &hosts),
                    UserContentInjectedFrames::AllFrames,
                    UserScriptInjectionTime::Start,
                    "rippl",
                    &allow,
                    &block,
                ));
            }
        }
        if config.autoscroll {
            scripts.push(UserScript::for_world(
                AUTOSCROLL,
                UserContentInjectedFrames::AllFrames,
                UserScriptInjectionTime::End,
                "rippl",
                &[],
                &[],
            ));
        }
        if config.video_keys {
            scripts.push(UserScript::for_world(
                VIDEO_KEYS,
                UserContentInjectedFrames::AllFrames,
                UserScriptInjectionTime::End,
                "rippl",
                &[],
                &[],
            ));
        }
        for script in &scripts {
            content.add_script(script);
        }
        *self.page_scripts.borrow_mut() = scripts;
    }

    /// The policy a page loading `url` gets: may its videos start by
    /// themselves, and with sound?
    pub(crate) fn website_policies(&self, url: &str, private: bool) -> WebsitePolicies {
        let policy = if !self.config.borrow().autoplay {
            AutoplayPolicy::Deny
        } else if !private
            && crate::shield::host_of(url).is_some_and(|h| self.places.site_flag(&h, SOUND).unwrap_or(false))
        {
            AutoplayPolicy::Allow
        } else {
            AutoplayPolicy::AllowWithoutSound
        };
        WebsitePolicies::builder().autoplay(policy).build()
    }
}
