//! The script every page gets (ui/page.js), with the keys to listen for.

use rippl_core::config::Config;
use rippl_core::keys::Key;

pub fn page_script(keymap: &[(String, Key)], config: &Config) -> String {
    let keys = crate::commands::keymap_json(keymap).to_string();
    let mut js = include_str!("../ui/page.js").replace("__RIPPL_KEYS__", &keys);
    if config.peek {
        js.insert_str(0, "window.__ripplPeek = true;\n");
    }
    js
}
