//! The web cache kept under `cache` (default 512 MB): at launch (a minute
//! in, out of the way) and every six hours, the disk cache of every kept
//! network session is measured, per site, and when the whole is over the
//! cap the sites taking the most room are emptied (`rippl_core::cache`).
//! Private tabs keep nothing on disk.

use std::time::Duration;

use webkit6::{glib, WebsiteDataTypes};

use crate::browser::Browser;

macro_rules! debug {
    ($($t:tt)*) => {
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!("rippl cache: {}", format!($($t)*));
        }
    };
}

impl Browser {
    pub(crate) fn watch_cache(&self) {
        if !self.primary {
            return;
        }
        let first = std::env::var("RIPPL_CACHE_CHECK").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        let weak = self.weak();
        glib::timeout_add_seconds_local_once(first, move || {
            if let Some(b) = weak.upgrade() {
                b.trim_cache();
            }
        });
        let weak = self.weak();
        glib::timeout_add_local(Duration::from_secs(6 * 60 * 60), move || {
            let Some(b) = weak.upgrade() else { return glib::ControlFlow::Break };
            b.trim_cache();
            glib::ControlFlow::Continue
        });
    }

    /// Measures each kept session's disk cache and empties its biggest
    /// sites if the whole is over the cap. Done in the background.
    pub(crate) fn trim_cache(&self) {
        let cap = self.config.borrow().cache_mb << 20;
        let managers: Vec<_> = self.engine.kept_sessions().iter().filter_map(|s| s.website_data_manager()).collect();
        glib::MainContext::default().spawn_local(async move {
            let mut all = Vec::new();
            for (m, manager) in managers.iter().enumerate() {
                match manager.fetch_future(WebsiteDataTypes::DISK_CACHE).await {
                    Ok(sites) => all.extend(sites.into_iter().map(|d| (m, d))),
                    Err(e) => debug!("could not measure: {e}"),
                }
            }
            let sizes: Vec<u64> = all.iter().map(|(_, d)| d.size(WebsiteDataTypes::DISK_CACHE)).collect();
            let total: u64 = sizes.iter().sum();
            let empty = rippl_core::cache::to_empty(&sizes, cap);
            debug!("{} sites, {} MB, cap {} MB: emptying {}", sizes.len(), total >> 20, cap >> 20, empty.len());
            for (m, manager) in managers.iter().enumerate() {
                let these: Vec<&webkit6::WebsiteData> =
                    empty.iter().filter(|&&i| all[i].0 == m).map(|&i| &all[i].1).collect();
                if these.is_empty() {
                    continue;
                }
                let names: Vec<String> = these.iter().filter_map(|d| d.name().map(|n| n.to_string())).collect();
                manager.remove(WebsiteDataTypes::DISK_CACHE, &these, None::<&webkit6::gio::Cancellable>, move |r| {
                    debug!("emptied {names:?}: {r:?}");
                });
            }
        });
    }
}
