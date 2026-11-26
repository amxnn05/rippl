//! Where passwords are kept: the system keyring, through the Secret Service
//! (GNOME Keyring, KeePassXC, KWallet's Secret Service mode), via libsecret.
//! rippl stores nothing secret in its own files.
//!
//! Each account is one keyring item:
//! - schema `dev.rippl.Password`
//! - attributes `origin` (`https://example.com`) and `username`
//! - label "rippl: bob on example.com", which is what keyring apps
//!   (Seahorse, "Passwords and Keys") show
//!
//! Listing accounts reads only attributes, so it doesn't need the secret
//! and doesn't make the keyring ask to unlock. Reading a password happens
//! only when you pick an account to fill or confirm a save.
//!
//! `RIPPL_KEYRING=memory` keeps everything in memory instead, for tests,
//! so a test run never touches your real keyring.

use std::cell::RefCell;
use std::collections::HashMap;

use libsecret::prelude::*;
use libsecret::{Schema, SchemaAttributeType, SchemaFlags, SearchFlags};
use zeroize::Zeroizing;

use rippl_core::passwords::label;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Account {
    pub origin: String,
    pub username: String,
}

pub enum Keyring {
    Secret(Schema),
    Memory(RefCell<Vec<(Account, Zeroizing<String>)>>),
}

const SCHEMA: &str = "dev.rippl.Password";

impl Keyring {
    pub fn new() -> Keyring {
        if std::env::var("RIPPL_KEYRING").as_deref() == Ok("memory") {
            return Keyring::Memory(RefCell::new(Vec::new()));
        }
        let attributes = HashMap::from([("origin", SchemaAttributeType::String), ("username", SchemaAttributeType::String)]);
        Keyring::Secret(Schema::new(SCHEMA, SchemaFlags::NONE, attributes))
    }

    pub fn is_memory(&self) -> bool {
        matches!(self, Keyring::Memory(_))
    }

    /// The accounts for `origin`, or every account rippl keeps.
    pub async fn accounts(&self, origin: Option<&str>) -> Result<Vec<Account>, String> {
        let mut out = match self {
            Keyring::Memory(items) => items
                .borrow()
                .iter()
                .filter(|(a, _)| origin.is_none_or(|o| a.origin == o))
                .map(|(a, _)| a.clone())
                .collect(),
            Keyring::Secret(schema) => {
                let mut attrs = HashMap::new();
                if let Some(o) = origin {
                    attrs.insert("origin", o);
                }
                let items = libsecret::password_search_future(Some(schema), attrs, SearchFlags::ALL)
                    .await
                    .map_err(|e| e.to_string())?;
                items
                    .iter()
                    .filter_map(|item| {
                        let a = item.attributes();
                        Some(Account { origin: a.get("origin")?.clone(), username: a.get("username").cloned().unwrap_or_default() })
                    })
                    .collect::<Vec<_>>()
            }
        };
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// The password for one account. Unlocks the keyring if it has to
    /// (the keyring daemon asks you, not rippl).
    pub async fn password(&self, origin: &str, username: &str) -> Result<Option<Zeroizing<String>>, String> {
        match self {
            Keyring::Memory(items) => Ok(items
                .borrow()
                .iter()
                .find(|(a, _)| a.origin == origin && a.username == username)
                .map(|(_, p)| p.clone())),
            Keyring::Secret(schema) => {
                let attrs = HashMap::from([("origin", origin), ("username", username)]);
                let found = libsecret::password_lookup_future(Some(schema), attrs).await.map_err(|e| e.to_string())?;
                Ok(found.map(|p| Zeroizing::new(p.to_string())))
            }
        }
    }

    /// Saves (or replaces) one account's password.
    pub async fn save(&self, origin: &str, username: &str, password: &str) -> Result<(), String> {
        match self {
            Keyring::Memory(items) => {
                let mut items = items.borrow_mut();
                items.retain(|(a, _)| !(a.origin == origin && a.username == username));
                let account = Account { origin: origin.into(), username: username.into() };
                items.push((account, Zeroizing::new(password.to_string())));
                Ok(())
            }
            Keyring::Secret(schema) => {
                let attrs = HashMap::from([("origin", origin), ("username", username)]);
                libsecret::password_store_future(
                    Some(schema),
                    attrs,
                    Some(libsecret::COLLECTION_DEFAULT),
                    &label(origin, username),
                    password,
                )
                .await
                .map_err(|e| e.to_string())
            }
        }
    }

    pub async fn delete(&self, origin: &str, username: &str) -> Result<(), String> {
        match self {
            Keyring::Memory(items) => {
                items.borrow_mut().retain(|(a, _)| !(a.origin == origin && a.username == username));
                Ok(())
            }
            Keyring::Secret(schema) => {
                let attrs = HashMap::from([("origin", origin), ("username", username)]);
                libsecret::password_clear_future(Some(schema), attrs).await.map_err(|e| e.to_string())
            }
        }
    }
}

/// The key a Chromium-family browser keeps its passwords under: "<Browser>
/// Safe Storage" in the keyring, found by its `application` attribute. The
/// keyring may ask to unlock. `None` if there is none (then its passwords
/// are under the fixed `v10` key, or out of reach).
pub async fn chromium_key(browser: &str) -> Option<Zeroizing<String>> {
    let application = rippl_core::import_more::keyring_application(browser)?;
    if std::env::var("RIPPL_KEYRING").as_deref() == Ok("memory") {
        return None;
    }
    let schema = Schema::new(
        "chrome_libsecret_os_crypt_password_v2",
        SchemaFlags::DONT_MATCH_NAME,
        HashMap::from([("application", SchemaAttributeType::String)]),
    );
    let found = libsecret::password_lookup_future(Some(&schema), HashMap::from([("application", application)])).await.ok()??;
    Some(Zeroizing::new(found.to_string()))
}
