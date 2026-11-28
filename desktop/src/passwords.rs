//! Saved passwords on macOS and Windows: the secrets in the system's own
//! store (the Keychain, the Credential Manager), and which accounts there
//! are in a list beside them (the stores can't be listed).

use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub const STORE: &str = "your Keychain";
#[cfg(windows)]
pub const STORE: &str = "Windows Credential Manager";
#[cfg(all(unix, not(target_os = "macos")))]
pub const STORE: &str = "your system keyring";

fn index() -> PathBuf {
    rippl_core::dirs::data().join("accounts")
}

/// (origin, username), as the index has them.
pub fn accounts() -> Vec<(String, String)> {
    std::fs::read_to_string(index())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(o, u)| (o.to_string(), u.to_string()))
        .collect()
}

fn write(list: &[(String, String)]) {
    let text: String = list.iter().map(|(o, u)| format!("{o}\t{u}\n")).collect();
    let _ = std::fs::write(index(), text);
}

fn entry(origin: &str, username: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(&format!("rippl {origin}"), username).map_err(|e| e.to_string())
}

pub fn save(origin: &str, username: &str, password: &str) -> Result<(), String> {
    entry(origin, username)?.set_password(password).map_err(|e| e.to_string())?;
    let mut list = accounts();
    if !list.iter().any(|(o, u)| o == origin && u == username) {
        list.push((origin.to_string(), username.to_string()));
        write(&list);
    }
    Ok(())
}

pub fn password(origin: &str, username: &str) -> Option<String> {
    entry(origin, username).ok()?.get_password().ok()
}

pub fn delete(origin: &str, username: &str) -> Result<(), String> {
    if let Ok(e) = entry(origin, username) {
        let _ = e.delete_credential();
    }
    let mut list = accounts();
    list.retain(|(o, u)| !(o == origin && u == username));
    write(&list);
    Ok(())
}
