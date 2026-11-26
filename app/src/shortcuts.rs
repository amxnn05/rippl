//! Your own keyboard shortcuts:
//! `shortcut.<command> = <Ctrl>j, F2` in the config, or on rippl://settings,
//! where every command is listed with its keys. `none` takes a command's
//! keys away; removing the line (an empty value) gives it its own back.
//! A change applies at once.

use rippl_core::pages::Setting;
use webkit6::gtk;
use webkit6::prelude::*;

use crate::browser::Browser;

/// A setting's keys, checked: GTK's accelerator names, comma-separated, or
/// `none` for no keys at all.
pub(crate) fn parse(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if value == "none" {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| {
            if gtk::accelerator_parse(k).is_some_and(|(key, _)| key != gtk::gdk::Key::VoidSymbol) {
                Ok(k.to_string())
            } else {
                Err(format!("{k:?} isn't a key GTK knows (write it as <Ctrl><Shift>x, F2, …)"))
            }
        })
        .collect()
}

/// "zoom-in" → "Zoom in".
fn label(name: &str) -> String {
    let words = name.replace('-', " ");
    let mut c = words.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

impl Browser {
    /// The keys `name` answers to: yours if set, else its own.
    pub(crate) fn keys_for(&self, name: &str, own: &[&str]) -> Vec<String> {
        self.known_actions.borrow_mut().retain(|(n, _)| n != name);
        self.known_actions.borrow_mut().push((name.to_string(), own.iter().map(|k| k.to_string()).collect()));
        match self.config.borrow().shortcuts.iter().find(|(n, _)| n == name) {
            Some((_, value)) => parse(value).unwrap_or_else(|e| {
                eprintln!("rippl: shortcut.{name}: {e}; keeping its own keys");
                own.iter().map(|k| k.to_string()).collect()
            }),
            None => own.iter().map(|k| k.to_string()).collect(),
        }
    }

    /// A shortcut changed on rippl://settings: applied now.
    pub(crate) fn apply_shortcut(&self, name: &str) {
        let Some(own) = self.known_actions.borrow().iter().find(|(n, _)| n == name).map(|(_, k)| k.clone()) else {
            return;
        };
        let own: Vec<&str> = own.iter().map(String::as_str).collect();
        let keys = self.keys_for(name, &own);
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        if let Some(app) = self.window.application() {
            app.set_accels_for_action(&format!("win.{name}"), &keys);
        }
    }

    /// Every command and its keys, for rippl://settings.
    pub(crate) fn shortcut_rows(&self) -> Vec<Setting> {
        let app = self.window.application();
        let mut known = self.known_actions.borrow().clone();
        known.sort();
        known
            .into_iter()
            .map(|(name, own)| {
                let now = app.as_ref().map(|a| a.accels_for_action(&format!("win.{name}"))).unwrap_or_default();
                let now: Vec<String> = now.iter().map(|k| k.to_string()).collect();
                let mine = self.config.borrow().shortcuts.iter().any(|(n, _)| *n == name);
                Setting {
                    key: format!("shortcut.{name}"),
                    label: label(&name),
                    value: if now.is_empty() { "none".into() } else { now.join(", ") },
                    choices: Vec::new(),
                    note: if mine { format!("Yours; its own: {}", own.join(", ")) } else { String::new() },
                }
            })
            .collect()
    }
}
