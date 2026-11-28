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
//! The rules: a tab sleeps after `wait`
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
    /// The pin folder a pinned tab is in, by name.
    pub pin_folder: Option<String>,
    /// Last time it was on screen.
    pub touched: Instant,
    /// The same, in unix seconds, kept in the session: tabs that close
    /// themselves count from it across restarts.
    pub left: i64,
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
            pin_folder: None,
            touched: Instant::now(),
            left: crate::places::now(),
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

/// Why `tab` stays open however long it is left,
/// or `None` if it may close itself: what you said to keep (pinned, in a
/// group, a site kept awake), what nothing could bring back (private), a
/// blank tab, and anything busy.
pub fn stays_open(tab: &Tab, on_screen: &[TabId]) -> Option<&'static str> {
    if tab.private {
        return Some("private");
    }
    if tab.group.is_some() {
        return Some("in a group");
    }
    if tab.url.is_empty() {
        return Some("blank");
    }
    kept_awake(tab, on_screen).map(|k| match k {
        KeptAwake::OnScreen => "on screen",
        KeptAwake::Pinned => "pinned",
        KeptAwake::Chosen => "kept awake",
        _ => "busy",
    })
}

/// The tabs left alone at least `after` by `now` (unix seconds), that may
/// close themselves. A split (`splits`, two to four tabs) goes whole or
/// not at all.
pub fn due_for_closing(tabs: &[Tab], on_screen: &[TabId], splits: &[Vec<TabId>], now: i64, after: Duration) -> Vec<TabId> {
    let since = now - after.as_secs() as i64;
    let due: Vec<TabId> =
        tabs.iter().filter(|t| t.left <= since && stays_open(t, on_screen).is_none()).map(|t| t.id).collect();
    due.iter()
        .copied()
        .filter(|id| {
            splits.iter().find(|s| s.contains(id)).is_none_or(|s| s.iter().all(|t| due.contains(t)))
        })
        .collect()
}

pub type SpaceId = u32;
pub type GroupId = u32;

/// A set of tabs with its own sign-ins. There is
/// always at least one; the first (id 0) uses the store rippl always had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    /// Its colour, or two for a gradient ("#rrggbb"); none: no wash.
    pub colours: Vec<String>,
}

/// A colour as rippl keeps them: "#rrggbb", lower case.
pub fn colour_ok(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// A named, foldable section of ordinary tabs in one space; or, with
/// `site`, one site's tabs gathered into a single row of tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: GroupId,
    pub space: SpaceId,
    pub name: String,
    pub collapsed: bool,
    /// Made by "Group this site's tabs": the site (host, `www.` aside).
    /// Such a group shows as one row and ends when one tab is left.
    pub site: Option<String>,
}

/// The site a tab belongs to, for grouping by site: its host, without
/// `www.` or a port. None for a blank tab.
pub fn site_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (!host.is_empty()).then_some(host)
}

/// What grouping a site's tabs did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteGrouping {
    /// Its tabs are now one group.
    Grouped(GroupId, usize),
    /// It was a site group: its tabs are loose again.
    Ungrouped(usize),
    /// Fewer than two tabs of that site (or a pinned or blank tab).
    Alone,
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
            spaces: vec![Space { id: 0, name: "Space 1".into(), colours: Vec::new() }],
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
        let unix = crate::places::now();
        if let Some(old) = self.front.and_then(|f| self.get_mut(f)) {
            old.touched = now;
            old.left = unix;
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
            tab.left = unix;
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
        tab.pin_folder = None;
        let space = tab.space;
        let at = match self.list.iter().rposition(|t| t.space == space && t.pinned) {
            Some(p) => p + 1,
            None => self.list.iter().position(|t| t.space == space).unwrap_or(self.list.len()),
        };
        self.list.insert(at, tab);
        self.tidy_groups();
    }

    /// Puts pinned `id` into the pin folder `name` (`None`: out of it).
    /// A folder is no more than the name its pins share in a space.
    pub fn set_pin_folder(&mut self, id: TabId, name: Option<&str>) {
        if let Some(t) = self.get_mut(id).filter(|t| t.pinned) {
            t.pin_folder = name.map(str::trim).filter(|n| !n.is_empty()).map(String::from);
        }
    }

    /// Renames a pin folder of `space`.
    pub fn rename_pin_folder(&mut self, space: SpaceId, old: &str, new: &str) {
        let new = new.trim();
        if new.is_empty() {
            return;
        }
        for t in self.list.iter_mut().filter(|t| t.space == space && t.pin_folder.as_deref() == Some(old)) {
            t.pin_folder = Some(new.to_string());
        }
    }

    /// The pin folders of `space`, in the order of their first pin.
    pub fn pin_folders(&self, space: SpaceId) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in self.list.iter().filter(|t| t.space == space && t.pinned).filter_map(|t| t.pin_folder.clone()) {
            if !out.contains(&f) {
                out.push(f);
            }
        }
        out
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
        self.spaces.push(Space { id, name, colours: Vec::new() });
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

    /// A space's colour (one), gradient (two), or none (empty).
    pub fn colour_space(&mut self, space: SpaceId, colours: &[String]) {
        if let Some(s) = self.spaces.iter_mut().find(|s| s.id == space) {
            s.colours = colours.iter().filter(|c| colour_ok(c)).take(2).map(|c| c.to_lowercase()).collect();
        }
    }

    /// The space after (or before) the one on screen, if there is one.
    pub fn neighbour_space(&self, step: isize) -> Option<SpaceId> {
        let at = self.spaces.iter().position(|s| s.id == self.space)? as isize + step;
        usize::try_from(at).ok().and_then(|i| self.spaces.get(i)).map(|s| s.id)
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
        self.groups.push(Group { id: g, space, name, collapsed: false, site: None });
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

    /// Whether `group` was made from a site's tabs.
    pub fn is_site_group(&self, group: GroupId) -> bool {
        self.groups.iter().any(|g| g.id == group && g.site.is_some())
    }

    /// Gathers the unpinned tabs of `id`'s site, in its space, into one
    /// site group, beside the first of them, in their order; tabs taken
    /// from other groups leave those. If `id` is in a site group already,
    /// its tabs go loose instead, where they are.
    pub fn group_site(&mut self, id: TabId) -> SiteGrouping {
        let Some(tab) = self.get(id) else { return SiteGrouping::Alone };
        if let Some(g) = tab.group.filter(|g| self.is_site_group(*g)) {
            let n = self.ungroup(g);
            return SiteGrouping::Ungrouped(n);
        }
        let Some(site) = site_of(&tab.url).filter(|_| !tab.pinned) else { return SiteGrouping::Alone };
        let space = tab.space;
        let of_site = |t: &Tab| t.space == space && !t.pinned && site_of(&t.url).as_deref() == Some(site.as_str());
        let n = self.list.iter().filter(|t| of_site(t)).count();
        if n < 2 {
            return SiteGrouping::Alone;
        }
        let g = self.next_group;
        self.next_group += 1;
        self.groups.push(Group { id: g, space, name: site.clone(), collapsed: false, site: Some(site.clone()) });
        // Nothing before the first of them moves, so they go back in where
        // it was.
        let first = self.list.iter().position(&of_site).unwrap_or(0);
        let mut members = Vec::new();
        let mut i = first;
        while i < self.list.len() {
            if of_site(&self.list[i]) {
                members.push(self.list.remove(i));
            } else {
                i += 1;
            }
        }
        let old: Vec<GroupId> = members.iter().filter_map(|t| t.group).collect();
        for (k, mut t) in members.into_iter().enumerate() {
            t.group = Some(g);
            self.list.insert(first + k, t);
        }
        for o in old {
            self.drop_empty_group(o);
        }
        SiteGrouping::Grouped(g, n)
    }

    /// Takes every tab out of `group`, leaving them where they are; returns
    /// how many there were.
    pub fn ungroup(&mut self, group: GroupId) -> usize {
        let mut n = 0;
        for t in self.list.iter_mut().filter(|t| t.group == Some(group)) {
            t.group = None;
            n += 1;
        }
        self.groups.retain(|g| g.id != group);
        n
    }

    /// A group with no tabs goes; a site group goes with one tab left.
    fn drop_empty_group(&mut self, group: GroupId) {
        let n = self.list.iter().filter(|t| t.group == Some(group)).count();
        if n == 0 || (n == 1 && self.is_site_group(group)) {
            self.ungroup(group);
        }
    }

    fn tidy_groups(&mut self) {
        let used: Vec<GroupId> = self.list.iter().filter_map(|t| t.group).collect();
        self.groups.retain(|g| used.contains(&g.id));
        let sites: Vec<GroupId> = self.groups.iter().filter(|g| g.site.is_some()).map(|g| g.id).collect();
        for g in sites {
            self.drop_empty_group(g);
        }
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

/// What a page says is waiting for you, read off the front of its title
/// the way WhatsApp, Messenger, X and Discord put it there: "(3) WhatsApp",
/// "(12+) …". Only a count at the very front,
/// in brackets: a number further along is as likely to be a year
/// ("Dune (2021) - IMDb"). Over 99 is "99+".
pub fn unread(title: &str) -> Option<String> {
    let title = title.trim_start();
    let inside = title.strip_prefix('(')?.split_once(')')?.0.trim();
    let (digits, more) = match inside.strip_suffix('+') {
        Some(d) => (d, true),
        None => (inside, false),
    };
    if !(1..=5).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    match n {
        0 => None,
        100.. => Some("99+".into()),
        _ if more => Some(format!("{n}+")),
        _ => Some(n.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_folders_by_name() {
        let mut tabs = Tabs::default();
        let a = tabs.open("https://a.com/");
        let b = tabs.open("https://b.com/");
        let c = tabs.open("https://c.com/");
        for id in [a, b, c] {
            tabs.set_pinned(id, true);
        }
        tabs.set_pin_folder(a, Some("Work"));
        tabs.set_pin_folder(c, Some("Work"));
        tabs.set_pin_folder(b, Some("  "));
        assert_eq!(tabs.pin_folders(0), vec!["Work".to_string()]);
        assert_eq!(tabs.get(b).unwrap().pin_folder, None);
        tabs.rename_pin_folder(0, "Work", "Jobs");
        assert_eq!(tabs.get(c).unwrap().pin_folder.as_deref(), Some("Jobs"));
        tabs.set_pinned(a, false);
        assert_eq!(tabs.get(a).unwrap().pin_folder, None, "unpinned: out of its folder");
    }

    #[test]
    fn tabs_left_alone_close() {
        let mut list: Vec<Tab> = (1..=7).map(|i| Tab::new(i, format!("https://s{i}.com/"))).collect();
        for t in &mut list {
            t.left = 1000;
        }
        list[1].pinned = true;
        list[2].group = Some(1);
        list[3].private = true;
        list[4].url.clear();
        list[6].left = 5000; // looked at since
        let after = Duration::from_secs(3600);
        assert_eq!(due_for_closing(&list, &[], &[], 5000, after), vec![1, 6]);
        assert_eq!(due_for_closing(&list, &[1], &[], 5000, after), vec![6]);
        // Half of a pair still wanted: both stay.
        assert_eq!(due_for_closing(&list, &[], &[vec![6, 7]], 5000, after), vec![1]);
        assert!(due_for_closing(&list, &[], &[], 2000, after).is_empty());
    }

    #[test]
    fn a_sites_tabs_group_and_ungroup() {
        let mut tabs = Tabs::default();
        let a = tabs.open("https://www.youtube.com/watch?v=1");
        let b = tabs.open("https://docs.rs/");
        let c = tabs.open("https://youtube.com/feed");
        let d = tabs.open("https://example.com/");
        let e = tabs.open("https://m.youtube.com/x"); // another host: not the same site
        let f = tabs.open("https://youtube.com:443/shorts");
        let pin = tabs.open("https://youtube.com/pinned");
        tabs.set_pinned(pin, true);
        tabs.new_group(d, "Mine");
        let mine = tabs.get(d).unwrap().group;
        tabs.set_group(f, mine);
        let g = match tabs.group_site(c) {
            SiteGrouping::Grouped(g, 3) => g,
            other => panic!("{other:?}"),
        };
        // Together where the first was, in their order; the pin stays out.
        let order: Vec<TabId> = tabs.here().filter(|t| !t.pinned).map(|t| t.id).collect();
        assert_eq!(order, vec![a, c, f, b, d, e]);
        assert!([a, c, f].iter().all(|t| tabs.get(*t).unwrap().group == Some(g)));
        assert_eq!(tabs.get(pin).unwrap().group, None);
        assert!(tabs.is_site_group(g));
        assert_eq!(tabs.groups.iter().find(|x| x.id == g).unwrap().name, "youtube.com");
        // "Mine" kept d when f left it.
        assert!(tabs.get(d).unwrap().group.is_some());
        // Again on a member: loose again, where they are.
        assert_eq!(tabs.group_site(a), SiteGrouping::Ungrouped(3));
        assert!(!tabs.is_site_group(g) && tabs.get(a).unwrap().group.is_none());
        // One tab of a site, a blank tab: nothing to group.
        assert_eq!(tabs.group_site(b), SiteGrouping::Alone);
        let blank = tabs.open("");
        assert_eq!(tabs.group_site(blank), SiteGrouping::Alone);
    }

    #[test]
    fn a_site_group_ends_with_one_tab_left() {
        let mut tabs = Tabs::default();
        let a = tabs.open("https://a.com/1");
        let b = tabs.open("https://a.com/2");
        let c = tabs.open("https://a.com/3");
        let SiteGrouping::Grouped(g, 3) = tabs.group_site(a) else { panic!() };
        tabs.close(b);
        assert!(tabs.is_site_group(g));
        tabs.close(c);
        assert!(tabs.groups.is_empty() && tabs.get(a).unwrap().group.is_none());
    }

    #[test]
    fn unread_counts_from_titles() {
        assert_eq!(unread("(3) WhatsApp").as_deref(), Some("3"));
        assert_eq!(unread("(12+) Messenger").as_deref(), Some("12+"));
        assert_eq!(unread("(250) Inbox").as_deref(), Some("99+"));
        assert_eq!(unread("Dune (2021) - IMDb"), None);
        assert_eq!(unread("(0) Nothing"), None);
        assert_eq!(unread("(new) Chat"), None);
        assert_eq!(unread("(123456) x"), None);
    }

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
