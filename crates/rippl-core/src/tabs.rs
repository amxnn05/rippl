//! The tabs of a window, and when a tab may give its page back.
//!
//! A page in a tab keeps a whole WebKit web process alive, usually 100 to
//! 300 MB, for as long as the tab has a view. Most of rippl's memory saving
//! comes from keeping few views alive:
//!
//! - **Lazy**: restored from last session, never shown. No view, no process.
//! - **Awake**: has a view and a web process.
//! - **Asleep**: had a view, gave it back. Keeps its serialized session state
//!   (back/forward list, scroll) so waking puts it back where it was.
//!
//! The same rules : a tab sleeps after `wait`
//! without being looked at, except the tab on screen, pinned tabs, and tabs
//! that would lose something by sleeping (sound playing, camera or mic in use).

use std::time::{Duration, Instant};

pub type TabId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Life {
    Lazy,
    Awake,
    Asleep,
}

/// Why an awake tab is kept awake, if it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeptAwake {
    OnScreen,
    Pinned,
    /// "Keep this site awake", chosen in the site card.
    Chosen,
    PlayingAudio,
    Capturing,
    Loading,
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub life: Life,
    pub pinned: bool,
    pub playing_audio: bool,
    /// Camera, microphone or screen in use.
    pub capturing: bool,
    pub loading: bool,
    /// Its own throwaway cookie jar; never saved to the session.
    pub private: bool,
    /// Its site is kept awake (the site card's switch).
    pub keep_awake: bool,
    /// Last time it was on screen.
    pub touched: Instant,
    /// WebKit's serialized session state, kept while asleep.
    pub memory: Option<Vec<u8>>,
}

impl Tab {
    pub fn new(id: TabId, url: impl Into<String>) -> Self {
        let url = url.into();
        Tab {
            id,
            title: crate::address::pretty(&url),
            url,
            life: Life::Lazy,
            pinned: false,
            playing_audio: false,
            capturing: false,
            loading: false,
            private: false,
            keep_awake: false,
            touched: Instant::now(),
            memory: None,
        }
    }
}

/// How hard the system is pressed for memory (Linux PSI, see the app).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pressure {
    None,
    Some,
    Critical,
}

/// How long a tab may sit unseen before it sleeps, given the pressure.
pub fn sleep_wait(chosen: Duration, pressure: Pressure) -> Duration {
    match pressure {
        Pressure::None => chosen,
        Pressure::Some => chosen.min(Duration::from_secs(5 * 60)),
        Pressure::Critical => Duration::ZERO,
    }
}

/// Why `tab` must stay awake, or `None` if it may sleep. `on_screen`: the
/// tab in front, and its partner when two are shown side by side.
pub fn kept_awake(tab: &Tab, on_screen: &[TabId]) -> Option<KeptAwake> {
    if on_screen.contains(&tab.id) {
        Some(KeptAwake::OnScreen)
    } else if tab.pinned {
        Some(KeptAwake::Pinned)
    } else if tab.keep_awake {
        Some(KeptAwake::Chosen)
    } else if tab.playing_audio {
        Some(KeptAwake::PlayingAudio)
    } else if tab.capturing {
        Some(KeptAwake::Capturing)
    } else if tab.loading {
        Some(KeptAwake::Loading)
    } else {
        None
    }
}

/// The awake tabs that should sleep now, the one left alone longest first.
pub fn due_for_sleep(tabs: &[Tab], on_screen: &[TabId], now: Instant, wait: Duration) -> Vec<TabId> {
    let mut due: Vec<&Tab> = tabs
        .iter()
        .filter(|t| t.life == Life::Awake)
        .filter(|t| now.saturating_duration_since(t.touched) >= wait)
        .filter(|t| kept_awake(t, on_screen).is_none())
        .collect();
    due.sort_by_key(|t| t.touched);
    due.into_iter().map(|t| t.id).collect()
}

/// The tabs of one window, in order, and which one is in front.
#[derive(Debug, Default)]
pub struct Tabs {
    pub list: Vec<Tab>,
    pub front: Option<TabId>,
    next: TabId,
}

impl Tabs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tab after the one in front (or at the end), returns its id.
    pub fn open(&mut self, url: impl Into<String>) -> TabId {
        self.next += 1;
        let tab = Tab::new(self.next, url);
        let after_front = self.front.and_then(|f| self.index(f)).map_or(self.list.len(), |i| i + 1);
        // Never inside the pinned group: opened from a pin, it goes first
        // among the ordinary tabs.
        let pins = self.list.iter().filter(|t| t.pinned).count();
        self.list.insert(after_front.max(pins), tab);
        self.next
    }

    /// Adds a tab at the end, as a restored session does.
    pub fn push(&mut self, url: impl Into<String>) -> TabId {
        self.next += 1;
        self.list.push(Tab::new(self.next, url));
        self.next
    }

    pub fn index(&self, id: TabId) -> Option<usize> {
        self.list.iter().position(|t| t.id == id)
    }

    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.list.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.list.iter_mut().find(|t| t.id == id)
    }

    /// Puts `id` in front, touching the tab that leaves the front too (it
    /// was being looked at until now).
    pub fn select(&mut self, id: TabId) {
        let now = Instant::now();
        if let Some(old) = self.front.and_then(|f| self.get_mut(f)) {
            old.touched = now;
        }
        if let Some(tab) = self.get_mut(id) {
            tab.touched = now;
            self.front = Some(id);
        }
    }

    /// Removes `id`; returns the tab that should come to front, if it was
    /// the front one: the next tab, or the previous one at the end.
    pub fn close(&mut self, id: TabId) -> Option<TabId> {
        let i = self.index(id)?;
        self.list.remove(i);
        if self.front != Some(id) {
            return self.front;
        }
        self.front = None;
        let next = self.list.get(i).or_else(|| self.list.last()).map(|t| t.id);
        if let Some(n) = next {
            self.select(n);
        }
        next
    }

    /// Pins or unpins `id`. Pinned tabs sit together at the top of the
    /// list, in the order they were pinned; an unpinned tab goes just
    /// below them.
    pub fn set_pinned(&mut self, id: TabId, pinned: bool) {
        let Some(i) = self.index(id) else { return };
        let mut tab = self.list.remove(i);
        tab.pinned = pinned;
        let pins = self.list.iter().filter(|t| t.pinned).count();
        self.list.insert(pins, tab);
    }

    /// Moves `id` to `index` (clamped to the list), never into or out of
    /// the pinned group by accident: an ordinary tab stays below the pins.
    pub fn move_to(&mut self, id: TabId, index: usize) {
        let Some(i) = self.index(id) else { return };
        let tab = self.list.remove(i);
        let pins = self.list.iter().filter(|t| t.pinned).count();
        let at = if tab.pinned { index.min(pins) } else { index.clamp(pins, self.list.len()) };
        self.list.insert(at, tab);
    }

    /// The tab `step` places from the front, wrapping round.
    pub fn neighbour(&self, step: isize) -> Option<TabId> {
        let n = self.list.len() as isize;
        if n == 0 {
            return None;
        }
        let i = self.front.and_then(|f| self.index(f)).unwrap_or(0) as isize;
        Some(self.list[(i + step).rem_euclid(n) as usize].id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn awake(id: TabId, ago: u64) -> Tab {
        let mut t = Tab::new(id, "https://example.com");
        t.life = Life::Awake;
        t.touched = Instant::now() - Duration::from_secs(ago);
        t
    }

    #[test]
    fn oldest_idle_sleep_first_and_front_never() {
        let tabs = vec![awake(1, 4000), awake(2, 100), awake(3, 9000), awake(4, 9999)];
        let due = due_for_sleep(&tabs, &[4], Instant::now(), Duration::from_secs(1800));
        assert_eq!(due, vec![3, 1]);
        // Side by side, both stay awake.
        let due = due_for_sleep(&tabs, &[4, 3], Instant::now(), Duration::from_secs(1800));
        assert_eq!(due, vec![1]);
    }

    #[test]
    fn exemptions_hold() {
        let mut a = awake(1, 9999);
        a.pinned = true;
        let mut b = awake(2, 9999);
        b.playing_audio = true;
        let mut c = awake(3, 9999);
        c.capturing = true;
        let mut d = awake(4, 9999);
        d.life = Life::Asleep;
        let mut e = awake(5, 9999);
        e.keep_awake = true;
        let due = due_for_sleep(&[a, b, c, d, e], &[], Instant::now(), Duration::from_secs(60));
        assert!(due.is_empty());
    }

    #[test]
    fn pressure_shortens_the_wait() {
        let half_hour = Duration::from_secs(1800);
        assert_eq!(sleep_wait(half_hour, Pressure::None), half_hour);
        assert_eq!(sleep_wait(half_hour, Pressure::Some), Duration::from_secs(300));
        assert_eq!(sleep_wait(half_hour, Pressure::Critical), Duration::ZERO);
    }

    #[test]
    fn open_goes_after_front_and_close_picks_next() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        let b = tabs.open("b.com");
        tabs.select(a);
        let c = tabs.open("c.com");
        assert_eq!(tabs.list.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a, c, b]);
        assert_eq!(tabs.close(a), Some(c));
        tabs.select(b);
        assert_eq!(tabs.close(b), Some(c));
        assert_eq!(tabs.close(c), None);
        assert_eq!(tabs.front, None);
    }

    #[test]
    fn pins_gather_at_the_top() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        let b = tabs.open("b.com");
        let c = tabs.open("c.com");
        tabs.set_pinned(c, true);
        tabs.set_pinned(a, true);
        let order = |t: &Tabs| t.list.iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(order(&tabs), vec![c, a, b]);
        tabs.set_pinned(c, false);
        assert_eq!(order(&tabs), vec![a, c, b]);
        // Opened from a pin: below the pins.
        tabs.select(a);
        let d = tabs.open("d.com");
        assert_eq!(order(&tabs), vec![a, d, c, b]);
        assert!(tabs.get(a).unwrap().pinned && !tabs.get(c).unwrap().pinned);
    }

    #[test]
    fn move_to_keeps_pins_apart() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        let b = tabs.open("b.com");
        let c = tabs.open("c.com");
        tabs.set_pinned(a, true);
        tabs.move_to(c, 0);
        assert_eq!(tabs.list.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a, c, b]);
        tabs.move_to(c, 99);
        assert_eq!(tabs.list.iter().map(|t| t.id).collect::<Vec<_>>(), vec![a, b, c]);
    }

    #[test]
    fn neighbour_wraps() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        let b = tabs.open("b.com");
        tabs.select(a);
        assert_eq!(tabs.neighbour(-1), Some(b));
        assert_eq!(tabs.neighbour(1), Some(b));
    }
}
