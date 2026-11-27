//! What was open last time, as plain text so it needs no parser crate and
//! stays readable by hand:
//!
//! ```text
//! rippl-session 2
//! space 0 Personal
//! space 3 Work
//! current 3
//! group 1 3 0 Reading
//! front 0 1
//! front 3 2
//! tab 0 0 - https://example.com/ Example Domain
//! tab 1 0 - https://news.ycombinator.com/ Hacker News
//! tab 0 3 1 https://docs.rs/ Docs.rs
//! ```
//!
//! `colour <space> <#rrggbb> [<#rrggbb>]` (a space's colour, or gradient),
//! `site <group> <host>` (a group made from one site's tabs),
//! `split <tab index> <tab index> …` (tabs side by side, two to four),
//! `pinfolder <tab index> <name>` (a pinned tab's folder),
//! `left <tab index> <unix seconds>` (when each tab was last on screen,
//! for tabs that close themselves; older files have none, and their tabs
//! count from the restore),
//! `space <id> <name>`, `group <id> <space> <collapsed> <name>`,
//! `front <space> <tab index>`, `tab <pinned> <space> <group or -> <url>
//! <title>`. URLs never contain spaces (WebKit percent-encodes them);
//! titles have line breaks flattened. Private tabs are never written.
//! Version 1 files (no spaces) are still read.

use crate::tabs::{Group, Space, SpaceId, Tab, TabId, Tabs};

const HEADER: &str = "rippl-session 2";
const HEADER_V1: &str = "rippl-session 1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub space: SpaceId,
    pub group: Option<u32>,
    /// When it was last on screen, unix seconds.
    pub left: Option<i64>,
    /// Its pin folder, if pinned into one.
    pub pin_folder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    pub tabs: Vec<Saved>,
    pub spaces: Vec<Space>,
    pub groups: Vec<Group>,
    pub current: SpaceId,
    /// For each space, the index into `tabs` of the tab that was in front.
    pub fronts: Vec<(SpaceId, usize)>,
    /// Tabs side by side, as indices into `tabs`.
    pub splits: Vec<Vec<usize>>,
}

fn flat(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

impl Session {
    /// The tabs worth keeping from `tabs`: everything but private and
    /// blank tabs.
    pub fn of(tabs: &Tabs) -> Session {
        // Blank tabs aren't kept either: read back, they'd be skipped, and
        // every index after them (fronts, times, folders) would point at the
        // wrong tab.
        let kept: Vec<&Tab> = tabs.list.iter().filter(|t| !t.private && !t.url.is_empty()).collect();
        let mut fronts = Vec::new();
        let mut all_fronts: Vec<(SpaceId, TabId)> = tabs.fronts.iter().map(|(s, t)| (*s, *t)).collect();
        if let Some(f) = tabs.front {
            all_fronts.retain(|(s, _)| *s != tabs.space);
            all_fronts.push((tabs.space, f));
        }
        for (space, id) in all_fronts {
            if let Some(i) = kept.iter().position(|t| t.id == id) {
                fronts.push((space, i));
            }
        }
        fronts.sort();
        Session {
            tabs: kept
                .iter()
                .map(|t| Saved {
                    url: t.url.clone(),
                    title: t.title.clone(),
                    pinned: t.pinned,
                    space: t.space,
                    group: t.group,
                    left: Some(t.left),
                    pin_folder: t.pin_folder.clone(),
                })
                .collect(),
            spaces: tabs.spaces.clone(),
            groups: tabs.groups.clone(),
            current: tabs.space,
            fronts,
            splits: Vec::new(),
        }
    }

    /// `of`, with the window's splits: each kept while two or more of its
    /// tabs are (a private tab in a split isn't).
    pub fn of_with_splits(tabs: &Tabs, splits: &[Vec<TabId>]) -> Session {
        let mut session = Session::of(tabs);
        let kept: Vec<TabId> = tabs.list.iter().filter(|t| !t.private && !t.url.is_empty()).map(|t| t.id).collect();
        for split in splits {
            let at: Vec<usize> = split.iter().filter_map(|id| kept.iter().position(|k| k == id)).collect();
            if at.len() >= 2 {
                session.splits.push(at);
            }
        }
        session
    }

    pub fn write(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for s in &self.spaces {
            out.push_str(&format!("space {} {}\n", s.id, flat(&s.name)));
            if !s.colours.is_empty() {
                out.push_str(&format!("colour {} {}\n", s.id, s.colours.join(" ")));
            }
        }
        out.push_str(&format!("current {}\n", self.current));
        for g in &self.groups {
            out.push_str(&format!("group {} {} {} {}\n", g.id, g.space, u8::from(g.collapsed), flat(&g.name)));
            if let Some(site) = &g.site {
                out.push_str(&format!("site {} {}\n", g.id, flat(site)));
            }
        }
        for (space, i) in &self.fronts {
            out.push_str(&format!("front {space} {i}\n"));
        }
        for t in &self.tabs {
            let group = t.group.map_or("-".to_string(), |g| g.to_string());
            out.push_str(&format!("tab {} {} {group} {} {}\n", u8::from(t.pinned), t.space, t.url, flat(&t.title)));
        }
        for (i, t) in self.tabs.iter().enumerate() {
            if let Some(left) = t.left {
                out.push_str(&format!("left {i} {left}\n"));
            }
            if let Some(f) = &t.pin_folder {
                out.push_str(&format!("pinfolder {i} {}\n", flat(f)));
            }
        }
        for split in &self.splits {
            let at: Vec<String> = split.iter().map(usize::to_string).collect();
            out.push_str(&format!("split {}\n", at.join(" ")));
        }
        out
    }

    /// Reads what `write` wrote (or a version 1 file). Lines it doesn't
    /// understand are skipped, so a damaged file loses those tabs, not the
    /// whole session.
    pub fn read(text: &str) -> Session {
        let mut lines = text.lines();
        let v1 = match lines.next() {
            Some(HEADER) => false,
            Some(HEADER_V1) => true,
            _ => return Session::default(),
        };
        let mut session = Session::default();
        let mut v1_front = None;
        let mut lefts = Vec::new();
        let mut folders: Vec<(usize, String)> = Vec::new();
        for line in lines {
            let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
            match kind {
                "space" => {
                    if let Some((id, name)) = rest.split_once(' ')
                        && let Ok(id) = id.parse()
                    {
                        session.spaces.push(Space { id, name: name.to_string(), colours: Vec::new() });
                    }
                }
                "colour" => {
                    if let Some((id, colours)) = rest.split_once(' ')
                        && let Ok(id) = id.parse::<SpaceId>()
                        && let Some(space) = session.spaces.iter_mut().find(|s| s.id == id)
                    {
                        space.colours =
                            colours.split_whitespace().filter(|c| crate::tabs::colour_ok(c)).take(2).map(String::from).collect();
                    }
                }
                "current" => session.current = rest.trim().parse().unwrap_or(0),
                "group" => {
                    let mut p = rest.splitn(4, ' ');
                    if let (Some(Ok(id)), Some(Ok(space)), Some(c), Some(name)) =
                        (p.next().map(str::parse), p.next().map(str::parse), p.next(), p.next())
                    {
                        session.groups.push(Group { id, space, collapsed: c == "1", name: name.to_string(), site: None });
                    }
                }
                "site" => {
                    if let Some((id, site)) = rest.split_once(' ')
                        && let Ok(id) = id.parse::<u32>()
                        && let Some(g) = session.groups.iter_mut().find(|g| g.id == id)
                        && !site.trim().is_empty()
                    {
                        g.site = Some(site.trim().to_string());
                    }
                }
                "split" => {
                    let at: Vec<usize> = rest.split_whitespace().filter_map(|i| i.parse().ok()).collect();
                    session.splits.push(at);
                }
                "front" if v1 => v1_front = rest.trim().parse::<usize>().ok(),
                "front" => {
                    if let Some((space, i)) = rest.split_once(' ')
                        && let (Ok(space), Ok(i)) = (space.parse(), i.trim().parse())
                    {
                        session.fronts.push((space, i));
                    }
                }
                "tab" if v1 => {
                    let mut parts = rest.splitn(3, ' ');
                    let (Some(pin), Some(url)) = (parts.next(), parts.next()) else { continue };
                    if url.is_empty() {
                        continue;
                    }
                    let title = parts.next().unwrap_or("").to_string();
                    session.tabs.push(Saved { url: url.into(), title, pinned: pin == "1", space: 0, group: None, left: None, pin_folder: None });
                }
                "tab" => {
                    let mut p = rest.splitn(5, ' ');
                    let (Some(pin), Some(Ok(space)), Some(group), Some(url)) =
                        (p.next(), p.next().map(str::parse), p.next(), p.next())
                    else {
                        continue;
                    };
                    if url.is_empty() {
                        continue;
                    }
                    session.tabs.push(Saved {
                        url: url.to_string(),
                        title: p.next().unwrap_or("").to_string(),
                        pinned: pin == "1",
                        space,
                        group: group.parse().ok(),
                        left: None,
                        pin_folder: None,
                    });
                }
                "pinfolder" => folders.extend(
                    rest.split_once(' ').and_then(|(i, f)| Some((i.parse::<usize>().ok()?, f.trim().to_string()))),
                ),
                "left" => lefts.extend(
                    rest.split_once(' ').and_then(|(i, l)| Some((i.parse::<usize>().ok()?, l.trim().parse::<i64>().ok()?))),
                ),
                _ => {}
            }
        }
        if let Some(f) = v1_front {
            session.fronts.push((0, f));
        }
        for (i, f) in folders {
            if let Some(t) = session.tabs.get_mut(i).filter(|t| t.pinned && !f.is_empty()) {
                t.pin_folder = Some(f);
            }
        }
        for (i, left) in lefts {
            if let Some(t) = session.tabs.get_mut(i) {
                t.left = Some(left);
            }
        }
        let n = session.tabs.len();
        session.fronts.retain(|(_, i)| *i < n);
        // A split is two to four tabs of one space, each in one split only.
        let mut taken = Vec::new();
        let tabs = &session.tabs;
        session.splits.retain_mut(|s| {
            s.dedup();
            s.retain(|i| *i < n && !taken.contains(i));
            let one_space = s.first().is_some_and(|f| s.iter().all(|i| tabs[*i].space == tabs[*f].space));
            let ok = (2..=4).contains(&s.len()) && one_space;
            if ok {
                taken.extend(s.iter().copied());
            }
            ok
        });
        // A tab in a space that wasn't listed goes to the first space.
        let known: Vec<SpaceId> = session.spaces.iter().map(|s| s.id).collect();
        if !known.is_empty() {
            for t in &mut session.tabs {
                if !known.contains(&t.space) {
                    t.space = known[0];
                    t.group = None;
                }
            }
        }
        session
    }

    /// Puts the saved tabs into `tabs` as Lazy tabs: no view, no process,
    /// until each is first shown. Returns the id that should come to front
    /// (in the space that was on screen).
    pub fn restore_into(&self, tabs: &mut Tabs) -> Option<TabId> {
        self.restore_all(tabs).0
    }

    /// `restore_into`, and the splits as the restored tabs' ids.
    pub fn restore_all(&self, tabs: &mut Tabs) -> (Option<TabId>, Vec<Vec<TabId>>) {
        tabs.restore_frame(self.spaces.clone(), self.groups.clone(), self.current);
        let mut ids = Vec::new();
        for saved in &self.tabs {
            let id = tabs.push(&saved.url);
            if let Some(t) = tabs.get_mut(id) {
                if !saved.title.is_empty() {
                    t.title = saved.title.clone();
                }
                t.pinned = saved.pinned;
                t.space = saved.space;
                t.group = saved.group;
                if let Some(left) = saved.left {
                    t.left = left;
                }
                t.pin_folder = saved.pin_folder.clone().filter(|_| saved.pinned);
            }
            ids.push(id);
        }
        // Groups whose tabs didn't come back go.
        let used: Vec<u32> = tabs.list.iter().filter_map(|t| t.group).collect();
        tabs.groups.retain(|g| used.contains(&g.id));
        for (space, i) in &self.fronts {
            if *space != tabs.space {
                tabs.fronts.insert(*space, ids[*i]);
            }
        }
        let current = tabs.space;
        let front = self
            .fronts
            .iter()
            .find(|(s, _)| *s == current)
            .map(|(_, i)| ids[*i])
            .or_else(|| tabs.here().last().map(|t| t.id));
        let splits = self.splits.iter().map(|s| s.iter().map(|i| ids[*i]).collect()).collect();
        (front, splits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_spaces_groups_fronts() {
        let mut tabs = Tabs::new();
        let a = tabs.open("https://a.com/");
        tabs.select(a);
        let p = tabs.open("https://secret.com/");
        tabs.get_mut(p).unwrap().private = true;
        let b = tabs.open("https://b.com/");
        tabs.get_mut(b).unwrap().title = "B\nsite".into();
        tabs.set_pinned(a, true);
        tabs.set_pin_folder(a, Some("Reading list"));
        let work = tabs.add_space("Work");
        tabs.colour_space(work, &["#4285F5".into(), "#e65466".into()]);
        tabs.switch_space(work);
        let w = tabs.open("https://jira.com/");
        tabs.select(w);
        let g = tabs.new_group(w, "Sprint").unwrap();
        let w2 = tabs.open("https://ci.com/");
        tabs.set_group(w2, Some(g));
        tabs.switch_space(0);
        tabs.select(b);
        tabs.switch_space(work);

        let blank = tabs.open("");
        let _ = blank;
        let text = Session::of(&tabs).write();
        assert!(!text.contains("secret"));
        assert!(!text.contains("New tab"), "blank tabs aren't kept");
        let back = Session::read(&text);
        assert_eq!(back.spaces.len(), 2);
        assert_eq!(back.current, work);
        assert_eq!(back.tabs.len(), 4);

        let mut fresh = Tabs::new();
        let front = back.restore_into(&mut fresh).unwrap();
        assert_eq!(fresh.space, work);
        assert_eq!(fresh.list.iter().find(|t| t.pinned).unwrap().pin_folder.as_deref(), Some("Reading list"));
        assert_eq!(fresh.spaces.iter().find(|s| s.id == work).unwrap().colours, vec!["#4285f5", "#e65466"]);
        assert_eq!(fresh.get(front).unwrap().url, "https://jira.com/");
        assert_eq!(fresh.groups.len(), 1);
        assert_eq!(fresh.here().count(), 2);
        assert!(fresh.list.iter().all(|t| t.life == crate::tabs::Life::Lazy));
        // Space 0 remembers b in front.
        assert_eq!(fresh.switch_space(0).and_then(|f| fresh.get(f).map(|t| t.url.clone())).as_deref(), Some("https://b.com/"));
        assert_eq!(fresh.list.iter().find(|t| t.url == "https://b.com/").unwrap().title, "B site");
    }

    #[test]
    fn site_groups_and_splits_come_back() {
        let mut tabs = Tabs::new();
        let a = tabs.open("https://a.com/1");
        let p = tabs.open("https://secret.com/");
        tabs.get_mut(p).unwrap().private = true;
        let b = tabs.open("https://a.com/2");
        let c = tabs.open("https://c.com/");
        let d = tabs.open("https://d.com/");
        let crate::tabs::SiteGrouping::Grouped(g, 2) = tabs.group_site(a) else { panic!() };
        // b+c side by side; a split with the private tab has one kept tab: dropped.
        let text = Session::of_with_splits(&tabs, &[vec![b, c], vec![p, d]]).write();
        assert!(text.contains(&format!("site {g} a.com")));
        let back = Session::read(&text);
        assert_eq!(back.splits.len(), 1);
        let mut fresh = Tabs::new();
        let (_, splits) = back.restore_all(&mut fresh);
        let urls: Vec<&str> = splits[0].iter().map(|id| fresh.get(*id).unwrap().url.as_str()).collect();
        assert_eq!(urls, vec!["https://a.com/2", "https://c.com/"]);
        assert!(fresh.groups[0].site.as_deref() == Some("a.com"));
        // Damaged lines: out of range, one tab, a tab in two splits.
        let bad = Session::read("rippl-session 2\nspace 0 One\ntab 0 0 - https://x.com/ x\ntab 0 0 - https://y.com/ y\nsplit 0 9\nsplit 1\nsplit 0 1\nsplit 1 0\n");
        assert_eq!(bad.splits, vec![vec![0, 1]]);
    }

    #[test]
    fn version_1_files_still_open() {
        let s = Session::read("rippl-session 1\nfront 1\ntab 1 https://a.com/ A\ntab 0 https://b.com/ B c\n");
        assert_eq!(s.tabs.len(), 2);
        assert!(s.tabs[0].pinned);
        assert_eq!(s.tabs[1].title, "B c");
        let mut tabs = Tabs::new();
        let front = s.restore_into(&mut tabs).unwrap();
        assert_eq!(tabs.get(front).unwrap().url, "https://b.com/");
    }

    #[test]
    fn junk_is_empty_and_bad_lines_are_skipped() {
        assert_eq!(Session::read("nonsense"), Session::default());
        let s = Session::read("rippl-session 2\nspace 0 One\nfront 0 9\ntab 0\ntab 0 0 - https://ok.com/ Ok\ntab 0 7 - https://lost.com/ x\n");
        assert_eq!(s.tabs.len(), 2);
        assert!(s.fronts.is_empty());
        assert_eq!(s.tabs[1].space, 0, "an unknown space's tab joins the first");
    }
}
