//! What stands behind the new tab's field (`home_theme`): nothing, a solid
//! colour, a gradient of `home_colour`, your own image, or one of the
//! painted walls. The walls are painted by the column itself (chrome.js);
//! here is only what to paint.

use rippl_core::config::Config;
use serde_json::{Value, json};

pub fn look(config: &Config) -> Value {
    let c = &config.home_colour;
    let background = match config.home_theme.as_str() {
        "solid" => format!("{c}22"),
        "gradient" => format!("linear-gradient(160deg, {c}33, {c}08 70%)"),
        "image" if !config.home_image.is_empty() => "url(rippl-ui://chrome/home-image) center / cover".to_string(),
        "open-road" => "linear-gradient(180deg, #f6c99a 0%, #f2a97e 38%, #8f9fb8 39%, #5d6b80 100%)".into(),
        "mountain-lake" => "linear-gradient(180deg, #a9c8e8 0%, #d9e6f2 45%, #6d8aa6 46%, #2f4a63 100%)".into(),
        "northern-lights" => "radial-gradient(ellipse at 30% 30%, #3ee8a5aa, transparent 55%), radial-gradient(ellipse at 70% 20%, #5b8cffaa, transparent 50%), linear-gradient(180deg, #0b1630, #050912)".into(),
        _ => String::new(),
    };
    json!({ "background": background, "theme": config.home_theme })
}
