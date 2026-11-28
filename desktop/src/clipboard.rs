//! The system clipboard (copy the address, paste and go).

pub fn set(text: &str) {
    if let Ok(mut c) = arboard::Clipboard::new() {
        let _ = c.set_text(text.to_string());
    }
}

pub fn get() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}
