# Phase 8: Shared processes, rippl's look, the first feature batch

**Status:** done (2026-11-25)

## Goal

Make the biggest memory cost smaller, give rippl its look, map every feature
([features.md](../features.md)), and start building the ones rippl lacks.

## What was built

### Same-site tabs share a page process

- When a tab wakes and another awake tab is on the same site (same host, `www.` aside, same kind of
  session: never a private tab with a kept one), the new view is created *related* to it and shares
  its page process. A WebKit page process costs about 160 MB before any page content
  ([performance.md](../performance.md)), so each extra tab of a site saves most of that.
- The tab *family* bookkeeping from Phase 2 covers it: a shared process is only ended when the last
  awake tab using it sleeps or closes. Navigating one of the tabs to another site moves it to a
  process of its own (WebKit's process swap on navigation).
- **Measured:** 5 Wikipedia tabs went from **897 MB in 5 processes to 630 MB in 1 (−30%)**, each tab
  still with its own page, title and history.
- **Trade-off:** if that one process crashes, every tab of the site reloads together. Chrome makes the
  same trade under memory pressure. `RIPPL_NO_SHARE=1` turns sharing off.

### rippl's look (`look.rs`)

The values:

| | Value |
|---|---|
| Page | a white card, 8 px from the window's edges, 10 px corners, a hairline around it |
| Column ground | `#f4f4f4` light / `#131313` dark |
| Ink, muted, faint | `#171717`; black at 42% / 13% (white at 55% / 26% in dark) |
| Live tab, hover | black at 6.3% / 4.2% (white at 8.7% / 6.0%) |
| Column | 232 px; rows 28 px, 9 px corners, 12.5 px titles, 8 px between letter and title |
| Site letter | 15 px square, 6% ink, 3 px corners, 8.5 px medium letter |
| Pins | 30 px squares holding one letter, in a row at the top |
| Font | Inter / Adwaita Sans (close to SF Pro), falling back to the system UI font |

- The letter comes from the name people know a site by: **W** for en.wikipedia.org, **B** for
  bbc.co.uk (`address::monogram`, tested).
- A faint **+ New tab** row ends the list; the gear at the column's foot opens Settings.
- Every colour is a CSS variable, set twice: once for light and once for the window's `dark` class.
  It follows `theme = system | light | dark`.
- **Dark mode, fixed after this phase.** It didn't work at first, for three reasons:
  - With `theme = system`, rippl told GTK "Default", and GTK without libadwaita doesn't fill that in
    from the desktop, so everything stayed light on a dark desktop.
  - The dark colours sat under a `prefers-color-scheme` media query, which GTK didn't apply.
  - WebKit also reads darkness from the GTK theme's *name*: Omarchy's `Adwaita-dark` made pages
    dark even with `theme = light`.

  Now `apply_look` (look.rs) reads the desktop's preference from the XDG settings portal
  (`org.freedesktop.appearance color-scheme`) and follows changes live. It sets rippl's `dark`
  class, GTK's interface colour scheme and prefer-dark, and, in rippl only, the light or dark
  variant of the GTK theme. Checked: `system` on a dark desktop → dark column and dark pages;
  `dark` → dark; `light` → light column and light pages (`matchMedia` in the page).
- **About transparency:** Omarchy's Hyprland config makes every window slightly see-through, so the
  wallpaper shows behind rippl's card and column. For a solid look, give the window class
  `dev.rippl.Rippl` full opacity in your Hyprland window rules. rippl doesn't change your
  compositor settings itself.

### The first features

| Feature | rippl | How |
|---|---|---|
| ⇧⌘T reopen closed tab | **Ctrl+Shift+T** | the last 25 closed tabs (never private ones) keep their WebKit session state; reopened in their old place, with history and scroll (`closed.rs`) |
| Copy Clean Link | **Ctrl+Shift+C** | copies the address without click IDs and `utm_*` tags (`rippl_core::clean`) |
| ⇧⌘V paste and go | **Ctrl+Shift+V** | a new tab with the clipboard's address or search |
| ⌘K switch tab | **Ctrl+K** | the field, empty: typing finds open tabs |
| Click IDs off links | always on | the page-process extension removes `fbclid`, `gclid`, `msclkid` and 20 others from every request before it leaves. WebKit's own filter for this (`FilterLinkDecorationByDefaultEnabled`) exists in WebKitGTK, but its list comes from Apple's WebPrivacy service and is empty on Linux |
| Keywords ("yt cats") | `w`, `yt`, `gh`, `mdn`, `osm`, `r` | `address::keyword`; templates must be http(s) with one `%s`, never in the host |
| Where a link goes | at the foot of the page | WebKit's `mouse-target-changed` |

## Tested

| Check | Result |
|---|---|
| 5 Wikipedia tabs, shared vs not | 630 MB / 1 process vs 897 MB / 5 |
| Screenshots, light and dark | column, pins, letters, live pill, New tab, gear |
| Reopen a tab navigated Linux → WebKit and scrolled to 2,500 px | back at position 2, `scrollY` 2615, Back goes to "Linux - Wikipedia" |
| `?id=7&fbclid=TRACKME&utm_source=news` | the server received `/?id=7&utm_source=news` |
| `yt lofi beats`, `w rust language` | YouTube's results; Wikipedia's article |
| Tests | 64 (60 core, 4 app), clippy clean |

## Found along the way

- **The UI's keystrokes aren't only the tests'.** A test window takes the focus when it opens, so
  keys typed into another app can land in it; that is where stray Settings tabs came from in two
  runs. Tests read the tab list before trusting what is in front.
- **A system "Allow … to use the camera" dialog** appeared once during a run with six sites. Three
  attempts to reproduce it, each site alone and all together, never did, and the dialog's title
  was cut off, so its app is unknown. Logged as *unconfirmed*; if it comes back with only rippl
  open, it's for investigating WebKitGTK's device enumeration.

## Next

[features.md](../features.md) lists every feature with its batch. Batch 1: the
site card (permissions, Shield and "keep awake" for the site), duplicate tab, blocked pop-ups with
Open, a typo-tolerant field, site search and your own keywords, field commands, print.
