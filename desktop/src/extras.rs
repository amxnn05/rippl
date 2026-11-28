//! Commands still to come on macOS and Windows; each says so plainly.

use crate::browser::Browser;

impl Browser {
    pub fn spawn_window(&self) {
        self.notice("More windows come in a later update on this system; spaces (Alt+number) hold more tabs meanwhile");
    }
    pub fn capture(&self) {
        self.notice("Capturing a page comes in a later update on this system");
    }
    pub fn toggle_reader(&self) {
        self.notice("Reading mode comes in a later update on this system");
    }
    pub fn pick_to_hide(&self) {
        self.notice("Hiding elements comes in a later update on this system");
    }
    pub fn unhide_here(&self) {
        let url = self.front_url();
        if let Some(host) = crate::shield::host_of(&url) {
            let _ = self.places.unhide(Some(&host));
            self.apply_site_styles();
            self.notice("Hidden elements are back on this site");
        }
    }
}
