# Feature batch 2: spaces, profiles, tab groups

**Status:** done (2026-11-25). Also in this round: dark mode fixed (see
[phase 8](phase-8-look.md)). Plan: [features.md](../features.md).

## Spaces (`spaces.rs`, `rippl_core::tabs`)

Separate sets of tabs in the one window, **each with its own cookies
and sign-ins**.

- **The first space is everything there was before**: its tabs and the store rippl always had, so
  making a second space signs nobody out.
- **A new space has its own store** (`spaces/<id>/web` in the data folder, made on first use). Signed
  in to a site in one space, you aren't signed in to it in another.
- **Switching** swaps the column's tabs and brings back the tab that space had in front. Tabs left
  behind are parked: still playing, and they sleep after the usual wait.
- **Keys:** `Alt+1–9` go to a space (`Ctrl+1–9` are tabs, and now count within the
  space on screen). `Ctrl+Alt+T` makes a new one.
- **The foot:** one button per space (its first letter), plus **+**. Nothing about
  spaces shows until there are two: only the +.
- **Right-click a space's button** to rename it, or delete it with its tabs. Deleting also clears its
  sign-ins and site data (WebKit's data manager, then the folder).
- **Moving tabs:** the tab menu has *Move to <space>*. A tab with a page sleeps and wakes in the new
  space's store, keeping its history.
- **Same-site sharing** (Phase 8) only happens within a space: two spaces' tabs of one site never
  share a process, since their stores differ.

## Tab groups

- **Making and changing groups** (tab menu): *Add to a new group*, *Add to "<group>"*, *Remove from
  group*.
- **In the column:** a heading `▾ Name  n` with the group's tabs indented under it. A click folds or
  unfolds it; right-click renames it or ungroups its tabs.
- **Model rules (tested):**
  - a group's tabs stay together;
  - a new tab opened from inside a group goes after the group, not into it;
  - an empty group disappears;
  - pinned tabs are never grouped.

## Profiles (`--profile NAME`)

**Whole separate browsers**.

- **What's separate:** each profile has its own spaces, tabs, sign-ins, history, bookmarks and
  session, in `profiles/<name>/` under the data and cache folders.
- **What's shared:** settings, passwords (the keyring), downloads, and Shield's
  compiled lists, so a second profile doesn't fetch and compile 48 MB again.
- **Separate windows:** each profile has its own application id (`dev.rippl.Rippl.profile_<name>`),
  so it gets its own window and can run beside the default one. Its window title ends in
  "· <name>", and its control socket is `rippl-<name>.sock` (`RIPPL_PROFILE=<name> scripts/ctl.sh`).
- Names are letters, digits, `-` and `_`; anything else is refused.

## Session format 2

```text
rippl-session 2
space 0 Space 1
space 1 Work
current 1
group 1 0 1 Reading
front 0 1
front 1 4
tab 0 0 - https://example.com/ Example Domain
tab 0 0 1 https://news.ycombinator.com/ Hacker News
```

Spaces, groups (folded or not) and each space's front tab are kept. Version 1 files still open, with
every tab in the first space (tested).

## Tested

| Check | Result |
|---|---|
| Cookie set in Space 1; same site opened in a new space "Work" | Work: **no cookie**; back in Space 1, still there |
| Columns per space | Space 1 shows only its tabs; Work only its own; each remembers its front |
| Group "Reading" of 2 tabs; fold | heading `▾ Reading 2`, tabs under it; folded: `▸ Reading 2` alone |
| Quit and start again | both spaces, the folded group, each space's tabs and fronts back |
| `--profile work` beside the default | 2 processes, 2 windows (`dev.rippl.Rippl`, `dev.rippl.Rippl.profile_work`), separate tabs; data in `profiles/work/`; Shield lists shared |
| `--profile "bad name"` | refused with the reason |
| Tests | 71 (67 core, 4 app), clippy clean |

## Bugs found and fixed

- **The column took half the window** once the space buttons were in its foot. An expanding child
  makes every ancestor expand in GTK. The buttons no longer expand, and the column is told
  explicitly not to.

## Not done (yet)

- **Pin folders**: rippl's pins are letter squares, not
  rows. Folders need pins as rows first (batch 4, with the icon-only column).
- **Send links to a space**: links from other apps arrive with default-browser
  support (Phase 9).
- **Space colours and gradients**: batch 4, with themes.
- **Profiles in one window**: each profile is its own window.
