//! A site that asks who you are with a certificate rather than a password
//!: a company's sign-in, a bank's, a
//! government's.
//!
//! Linux has no keychain of identities as the Mac does, so rippl keeps
//! them in a folder: `~/.local/share/rippl/certificates/`, each file a
//! certificate with its key (`.pem`, or `.p12` / `.pfx` without a
//! password). When a site asks, you pick one of them, or none, as Safari
//! has you do; nothing is sent unless you chose it. The choice holds for
//! that site until rippl is closed; a private tab asks for itself and
//! forgets.

use std::collections::HashMap;
use std::path::PathBuf;

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gio, gtk, AuthenticationRequest, AuthenticationScheme, Credential, CredentialPersistence};

use crate::browser::Browser;

/// Where your identities are kept.
pub(crate) fn folder() -> PathBuf {
    crate::engine::data_dir().join("certificates")
}

/// One identity from the folder, as the picker names it.
#[derive(Clone)]
pub(crate) struct Identity {
    pub certificate: gio::TlsCertificate,
    pub label: String,
}

/// "CN=Alice,O=Example" → "Alice".
fn common_name(dn: &str) -> String {
    dn.split(',').find_map(|p| p.trim().strip_prefix("CN=")).unwrap_or(dn).to_string()
}

/// Every identity in the folder that can be read with its key.
pub(crate) fn identities() -> Vec<Identity> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder()) else { return out };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        // GLib reads PEM and (since 2.72) PKCS #12 from a file alike.
        let certificate = match ext.as_str() {
            "pem" | "crt" | "p12" | "pfx" => gio::TlsCertificate::from_file(&path).ok(),
            _ => None,
        };
        let Some(certificate) = certificate.filter(|c| c.private_key_pem().is_some() || ext != "pem") else {
            eprintln!("rippl: certificates: {} has no key with it, or isn't one", path.display());
            continue;
        };
        let name = certificate.subject_name().map(|n| common_name(&n)).unwrap_or_else(|| {
            path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        });
        let mut label = name;
        if let Some(issuer) = certificate.issuer_name().map(|n| common_name(&n)).filter(|i| !i.is_empty()) {
            label.push_str(&format!(" — issued by {issuer}"));
        }
        if let Some(until) = certificate.not_valid_after().and_then(|d| d.format("%-d %b %Y").ok()) {
            label.push_str(&format!(", until {until}"));
        }
        out.push(Identity { certificate, label });
    }
    out
}

impl Browser {
    /// A page's request to authenticate: a certificate question is
    /// answered here; anything else gets WebKit's own sign-in box.
    pub(crate) fn authenticate(&self, id: TabId, request: &AuthenticationRequest) -> bool {
        if std::env::var_os("RIPPL_DEBUG").is_some() {
            eprintln!("rippl: authenticate {:?} for {:?} retry={}", request.scheme(), request.host(), request.is_retry());
        }
        match request.scheme() {
            AuthenticationScheme::ClientCertificateRequested => {}
            // A smart card's PIN: not offered.
            AuthenticationScheme::ClientCertificatePinRequested => {
                request.cancel();
                return true;
            }
            _ => return false,
        }
        let host = request.host().map(|h| h.to_string()).unwrap_or_default();
        let private = self.tabs.borrow().get(id).is_some_and(|t| t.private);
        // Answered for this site already (not in a private tab).
        if !private && let Some(choice) = self.cert_choices.borrow().get(&host).cloned() {
            send(request, choice.as_ref());
            return true;
        }
        let mine = identities();
        if mine.is_empty() {
            self.notice(&format!("{host} asks for a certificate: put one in {}", folder().display()));
            request.authenticate(None);
            return true;
        }
        let mut buttons: Vec<String> = mine.iter().map(|i| i.label.clone()).collect();
        buttons.push("Don't send one".into());
        let buttons: Vec<&str> = buttons.iter().map(String::as_str).collect();
        let dialog = gtk::AlertDialog::builder()
            .message(format!("{host} asks who you are"))
            .detail("Choose the certificate to send. Nothing is sent unless you choose one.")
            .buttons(buttons)
            .cancel_button(mine.len() as i32)
            .default_button(0)
            .modal(true)
            .build();
        let request = request.clone();
        let weak = self.weak();
        dialog.choose(Some(&self.window), None::<&gio::Cancellable>, move |r| {
            let Some(b) = weak.upgrade() else {
                request.cancel();
                return;
            };
            let choice = r.ok().and_then(|i| mine.get(i as usize)).map(|i| i.certificate.clone());
            if !private {
                b.cert_choices.borrow_mut().insert(host.clone(), choice.clone());
            }
            send(&request, choice.as_ref());
        });
        true
    }
}

fn send(request: &AuthenticationRequest, certificate: Option<&gio::TlsCertificate>) {
    match certificate {
        Some(c) => request.authenticate(Some(&Credential::for_certificate(Some(c), CredentialPersistence::ForSession))),
        None => request.authenticate(None),
    }
}

/// Kept for the session: each site's answer (`None`: send none).
pub(crate) type Choices = std::cell::RefCell<HashMap<String, Option<gio::TlsCertificate>>>;
