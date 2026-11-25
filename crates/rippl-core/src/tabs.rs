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
    /// The space it belongs to, and its group there, if any.
    pub space: SpaceId,
    pub group: Option<GroupId>,
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
            space: 0,
            group: None,
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

pub type SpaceId = u32;
pub type GroupId = u32;

/// A set of tabs with its own sign-ins. There is
/// always at least one; the first (id 0) uses the store rippl always had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
}

/// A named, foldable section of ordinary tabs in one space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: GroupId,
    pub space: SpaceId,
    pub name: String,
    pub collapsed: bool,
}

/// The tabs of a window, in every space, and which one is in front.
///
/// `list` holds every space's tabs; a space's tabs are the ones with its
/// id, in list order: pinned first, then the rest, a group's tabs together.
/// `front` is the tab in front of the space on screen; `fronts` remembers
/// the others'.
#[derive(Debug)]
pub struct Tabs {
    pub list: Vec<Tab>,
    pub front: Option<TabId>,
    pub space: SpaceId,
    pub spaces: Vec<Space>,
    pub fronts: std::collections::HashMap<SpaceId, TabId>,
    pub groups: Vec<Group>,
    next: TabId,
    next_space: SpaceId,
    next_group: GroupId,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs {
            list: Vec::new(),
            front: None,
            space: 0,
            spaces: vec![Space { id: 0, name: "Space 1".into() }],
            fronts: Default::default(),
            groups: Vec::new(),
            next: 0,
            next_space: 1,
            next_group: 1,
        }
    }
}

impl Tabs {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tabs of the space on screen, in order.
    pub fn here(&self) -> impl Iterator<Item = &Tab> {
        self.list.iter().filter(move |t| t.space == self.space)
    }

    /// Where in `list` a new ordinary tab of `space` goes: after the front
    /// tab when it is in that space (never among the pins, nor inside a
    /// group), otherwise after the space's last tab.
    fn slot(&self, space: SpaceId) -> usize {
        let last_pin = self.list.iter().rposition(|t| t.space == space && t.pinned);
        let front = self.front.and_then(|f| self.index(f)).filter(|&i| self.list[i].space == space);
        let mut at = match front {
            Some(i) => i + 1,
            None => self.list.iter().rposition(|t| t.space == space).map_or(self.list.len(), |i| i + 1),
        };
        if let Some(p) = last_pin {
            at = at.max(p + 1);
        }
        // Not inside a group: past its last tab.
        while at > 0 && at < self.list.len() {
            let (before, here) = (&self.list[at - 1], &self.list[at]);
            if here.group.is_some() && here.group == before.group && here.space == space {
                at += 1;
            } else {
                break;
            }
        }
        at
    }

    /// Adds a tab to the space on screen, after the one in front.
    pub fn open(&mut self, url: impl Into<String>) -> TabId {
        self.next += 1;
        let mut tab = Tab::new(self.next, url);
        tab.space = self.space;
        let at = self.slot(self.space);
        self.list.insert(at, tab);
        self.next
    }

    /// Adds a tab at the end of the list, as a restored session does.
    pub fn push(&mut self, url: impl Into<String>) -> TabId {
        self.next += 1;
        let mut tab = Tab::new(self.next, url);
        tab.space = self.space;
        self.list.push(tab);
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
    /// was being looked at until now). A tab of another space brings its
    /// space with it.
    pub fn select(&mut self, id: TabId) {
        let now = Instant::now();
        if let Some(old) = self.front.and_then(|f| self.get_mut(f)) {
            old.touched = now;
        }
        let Some(space) = self.get(id).map(|t| t.space) else { return };
        if space != self.space {
            if let Some(f) = self.front {
                self.fronts.insert(self.space, f);
            }
            self.space = space;
        }
        if let Some(tab) = self.get_mut(id) {
            tab.touched = now;
            self.front = Some(id);
        }
    }

    /// Removes `id`; returns the tab that should come to front: the next
    /// tab of its space, or the previous one at the end, or none if the
    /// space is left empty.
    pub fn close(&mut self, id: TabId) -> Option<TabId> {
        let i = self.index(id)?;
        let tab = self.list.remove(i);
        self.fronts.retain(|_, f| *f != id);
        if let Some(g) = tab.group {
            self.drop_empty_group(g);
        }
        if self.front != Some(id) {
            return self.front;
        }
        self.front = None;
        let next = self.list[i..]
            .iter()
            .find(|t| t.space == tab.space)
            .or_else(|| self.list[..i].iter().rev().find(|t| t.space == tab.space))
            .map(|t| t.id);
        if let Some(n) = next {
            self.select(n);
        }
        next
    }

    /// Pins or unpins `id`. A space's pins sit together at its top, in the
    /// order they were pinned; an unpinned tab goes just below them.
    pub fn set_pinned(&mut self, id: TabId, pinned: bool) {
        let Some(i) = self.index(id) else { return };
        let mut tab = self.list.remove(i);
        tab.pinned = pinned;
        tab.group = None;
        let space = tab.space;
        let at = match self.list.iter().rposition(|t| t.space == space && t.pinned) {
            Some(p) => p + 1,
            None => self.list.iter().position(|t| t.space == space).unwrap_or(self.list.len()),
        };
        self.list.insert(at, tab);
        self.tidy_groups();
    }

    /// Moves `id` to position `index` among its space's tabs (clamped),
    /// never into or out of the pins by accident.
    pub fn move_to(&mut self, id: TabId, index: usize) {
        let Some(i) = self.index(id) else { return };
        let tab = self.list.remove(i);
        let space: Vec<usize> = (0..self.list.len()).filter(|&j| self.list[j].space == tab.space).collect();
        let pins = space.iter().filter(|&&j| self.list[j].pinned).count();
        let k = if tab.pinned { index.min(pins) } else { index.clamp(pins, space.len()) };
        let at = space.get(k).copied().unwrap_or_else(|| space.last().map_or(self.list.len(), |&j| j + 1));
        self.list.insert(at, tab);
    }

    /// The tab `step` places from the front in the space on screen,
    /// wrapping round.
    pub fn neighbour(&self, step: isize) -> Option<TabId> {
        let here: Vec<TabId> = self.here().map(|t| t.id).collect();
        let n = here.len() as isize;
        if n == 0 {
            return None;
        }
        let i = self.front.and_then(|f| here.iter().position(|&t| t == f)).unwrap_or(0) as isize;
        Some(here[(i + step).rem_euclid(n) as usize])
    }

    // ---- spaces --------------------------------------------------------

    /// Shows `space`; returns the tab to put in front (the one it had, or
    /// its first), or none if the space has no tabs.
    pub fn switch_space(&mut self, space: SpaceId) -> Option<TabId> {
        if !self.spaces.iter().any(|s| s.id == space) {
            return self.front;
        }
        if let Some(f) = self.front {
            self.fronts.insert(self.space, f);
        }
        self.space = space;
        let remembered = self.fronts.get(&space).copied().filter(|f| self.get(*f).is_some_and(|t| t.space == space));
        self.front =
            remembered.or_else(|| self.here().find(|t| !t.pinned).or_else(|| self.here().next()).map(|t| t.id));
        self.front
    }

    pub fn add_space(&mut self, name: &str) -> SpaceId {
        let id = self.next_space;
        self.next_space += 1;
        let name =
            if name.trim().is_empty() { format!("Space {}", self.spaces.len() + 1) } else { name.trim().to_string() };
        self.spaces.push(Space { id, name });
        id
    }

    /// Removes `space` (never the last one) and returns its tabs, for the
    /// caller to close. The space on screen moves to the first one left.
    pub fn remove_space(&mut self, space: SpaceId) -> Vec<TabId> {
        if self.spaces.len() <= 1 || !self.spaces.iter().any(|s| s.id == space) {
            return Vec::new();
        }
        let doomed: Vec<TabId> = self.list.iter().filter(|t| t.space == space).map(|t| t.id).collect();
        self.list.retain(|t| t.space != space);
        self.groups.retain(|g| g.space != space);
        self.spaces.retain(|s| s.id != space);
        self.fronts.remove(&space);
        if self.space == space {
            self.front = None;
            let first = self.spaces[0].id;
            self.switch_space(first);
        }
        doomed
    }

    pub fn rename_space(&mut self, space: SpaceId, name: &str) {
        if let Some(s) = self.spaces.iter_mut().find(|s| s.id == space)
            && !name.trim().is_empty()
        {
            s.name = name.trim().to_string();
        }
    }

    /// Moves a tab to the end of another space, out of any group.
    pub fn move_to_space(&mut self, id: TabId, space: SpaceId) {
        if !self.spaces.iter().any(|s| s.id == space) {
            return;
        }
        let Some(i) = self.index(id) else { return };
        let mut tab = self.list.remove(i);
        tab.space = space;
        tab.group = None;
        tab.pinned = false;
        self.fronts.retain(|_, f| *f != id);
        if self.front == Some(id) {
            self.front = None;
        }
        let at = self.list.iter().rposition(|t| t.space == space).map_or(self.list.len(), |j| j + 1);
        self.list.insert(at, tab);
        self.tidy_groups();
    }

    // ---- groups --------------------------------------------------------

    /// A new group holding `id`; returns its id.
    pub fn new_group(&mut self, id: TabId, name: &str) -> Option<GroupId> {
        let tab = self.get(id)?;
        if tab.pinned {
            return None;
        }
        let space = tab.space;
        let g = self.next_group;
        self.next_group += 1;
        let name = if name.trim().is_empty() { "Group".to_string() } else { name.trim().to_string() };
        self.groups.push(Group { id: g, space, name, collapsed: false });
        let old = self.get(id)?.group;
        self.get_mut(id)?.group = Some(g);
        if let Some(o) = old {
            self.drop_empty_group(o);
        }
        Some(g)
    }

    /// Puts `id` in `group` (beside its other tabs), or out of any.
    pub fn set_group(&mut self, id: TabId, group: Option<GroupId>) {
        let Some(i) = self.index(id) else { return };
        if self.list[i].pinned {
            return;
        }
        let old = self.list[i].group;
        if let Some(g) = group {
            let Some(space) = self.groups.iter().find(|x| x.id == g).map(|x| x.space) else { return };
            let mut tab = self.list.remove(i);
            tab.group = Some(g);
            tab.space = space;
            let at = self.list.iter().rposition(|t| t.group == Some(g)).map_or(self.list.len(), |j| j + 1);
            self.list.insert(at, tab);
        } else {
            self.list[i].group = None;
        }
        if let Some(o) = old {
            self.drop_empty_group(o);
        }
    }

    pub fn toggle_group(&mut self, group: GroupId) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.collapsed = !g.collapsed;
        }
    }

    pub fn rename_group(&mut self, group: GroupId, name: &str) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group)
            && !name.trim().is_empty()
        {
            g.name = name.trim().to_string();
        }
    }

    fn drop_empty_group(&mut self, group: GroupId) {
        if !self.list.iter().any(|t| t.group == Some(group)) {
            self.groups.retain(|g| g.id != group);
        }
    }

    fn tidy_groups(&mut self) {
        let used: Vec<GroupId> = self.list.iter().filter_map(|t| t.group).collect();
        self.groups.retain(|g| used.contains(&g.id));
    }

    // ---- restoring -------------------------------------------------------

    /// Puts back spaces and groups from a saved session, keeping their ids.
    pub fn restore_frame(&mut self, spaces: Vec<Space>, groups: Vec<Group>, current: SpaceId) {
        if !spaces.is_empty() {
            self.next_space = spaces.iter().map(|s| s.id).max().unwrap_or(0) + 1;
            self.spaces = spaces;
        }
        self.next_group = groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
        self.groups = groups;
        if self.spaces.iter().any(|s| s.id == current) {
            self.space = current;
        }
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
    fn spaces_keep_their_tabs_and_fronts() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        tabs.select(a);
        let work = tabs.add_space("Work");
        assert_eq!(tabs.switch_space(work), None, "a new space is empty");
        let w1 = tabs.open("jira.com");
        tabs.select(w1);
        let w2 = tabs.open("docs.com");
        tabs.select(w2);
        assert_eq!(tabs.here().map(|t| t.id).collect::<Vec<_>>(), vec![w1, w2]);
        assert_eq!(tabs.switch_space(0), Some(a));
        assert_eq!(tabs.here().count(), 1);
        assert_eq!(tabs.switch_space(work), Some(w2), "each space remembers its front");
        tabs.close(w1);
        assert_eq!(tabs.close(w2), None, "an empty space has nothing in front");
        tabs.select(a);
        assert_eq!(tabs.space, 0, "selecting a tab brings its space");
        assert!(tabs.remove_space(work).is_empty());
        assert_eq!(tabs.spaces.len(), 1);
        assert!(tabs.remove_space(0).is_empty(), "never the last space");
    }

    #[test]
    fn groups_stay_together_and_vanish_empty() {
        let mut tabs = Tabs::new();
        let a = tabs.open("a.com");
        tabs.select(a);
        let b = tabs.open("b.com");
        let c = tabs.open("c.com");
        let g = tabs.new_group(a, "Read").unwrap();
        tabs.set_group(c, Some(g));
        let order: Vec<TabId> = tabs.list.iter().map(|t| t.id).collect();
        assert_eq!(order, vec![a, c, b], "c joined a");
        tabs.select(a);
        let d = tabs.open("d.com");
        assert_eq!(tabs.index(d), Some(2), "a new tab goes after the group, not into it");
        tabs.set_group(a, None);
        tabs.set_group(c, None);
        assert!(tabs.groups.is_empty(), "an empty group goes");
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
