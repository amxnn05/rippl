# Phase 4: Daily use

**Status:** done (2026-11-25)

## Goal

What you need to use a browser every day: history you can find things in, bookmarks, downloads,
find in page, sites asking permission for camera or location, and light/dark mode.

## What was built

### Places: history, bookmarks, permissions (`rippl_core::places`)

One SQLite file, `~/.local/share/rippl/places.sqlite`, in WAL mode. It uses the **system** SQLite
through `rusqlite`, not a bundled copy, which keeps the binary small and gets security updates
from the distribution.

- **History** keeps one row per URL: title, visit count, how often it was *typed*, and the last
  visit. A visit is recorded when a page finishes loading, for `http(s)` pages only, and **never
  from a private tab**. A typed visit counts double in ranking. `history = off` in the config
  stops recording.
- **Bookmarks**: `Ctrl+D` or the star at the left of the field toggles the page in front. The star
  is filled when the page is bookmarked. Private tabs can't bookmark.
- **Permission answers**: one row per (site, kind) (see below).
- `LIKE` patterns are escaped, so typing `%` or `_` matches those characters literally (tested).

### The field's offers (`omnibox.rs`, `rippl_core::suggest`)

- While you type, the column's tab list is replaced by matches:
  1. open tabs ("Switch to tab", up to 3),
  2. bookmarks and history, ranked.
- **Ranking** (`suggest::score`): visits fading with age (half weight after a week). Typing the
  start of a site counts ×10, the start of the URL ×5, the start of a word in the host ×3, the start
  of a word in the title ×2. Bookmarks count ×2. Shorter URLs rank above deep pages of the same site.
- **Inline completion** (`suggest::complete`): "you" becomes "you**tube.com**", with the added
  part selected. It completes up to the host unless a `/` was typed, keeps the case you typed, and
  only completes while you type forward, never after Backspace.
- **Nothing typed** (focus the field, or `Ctrl+H`): bookmarks first, then recent pages.
- `↓`/`↑` move through the offers, `Return` opens the picked one (or what is typed), `Esc` puts
  the page's address back.
- Rows never take focus from the field, so clicking a row works before the field loses focus.
- **Nothing typed leaves the machine.** Offers come only from local history, bookmarks and tabs.
  There are no search-engine suggestions.

### Downloads (`downloads.rs`, `rippl_core::downloads`)

- Saved in your XDG Downloads folder (`~/Downloads`), under the site's file name, **cleaned**: path
  parts, a leading dot and control characters are removed, and long names are cut while keeping
  the extension. A file is **never overwritten**: `report.pdf`, then `report (1).pdf`; `.tar.gz`
  stays together.
- Listed at the foot of the column with progress ("1.2 of 5.0 MB"). Click a finished one to open it
  with its default app; × cancels a running download or removes a finished one.
- Works for private tabs too, since each private session gets the same hook (`Engine::on_download`).
- Nothing about downloads is kept after quitting; the files themselves are the record.

### Find in page (`find.rs`)

`Ctrl+F` opens a bar over the top right of the page. It searches as you type (case-insensitive,
wrapping round) and shows "N matches". `Return`/`Shift+Return` (or `Ctrl+G`/`Ctrl+Shift+G`) go to
the next/previous match, `Esc` closes it and clears the highlights. Switching tabs closes it.

### Permission prompts (`permissions.rs`)

A bar over the page: *"meet.example wants to use your camera"*, with **Deny** and **Allow**.

| Request | What rippl does |
|---|---|
| camera, microphone, both, screen | ask |
| location, notifications, clipboard read, DRM, storage access | ask |
| pointer lock | allow (games; Esc always releases it) |
| camera and microphone *names* | allow only if the site already has camera or microphone |
| anything else (VR, …) | deny |

- Answers are remembered per site, except in private tabs, where they last only for the page.
- One answer also settles the same question from the same site in other tabs.
- Questions only show for the tab in front. A closed tab's questions are dropped, and WebKit
  denies them.
- `scripts/ctl.sh permissions` lists answers; `permissions forget [host]` forgets them.

### Theme

`theme = system | light | dark` in the config. `system` (the default) follows the desktop through
GTK. `light`/`dark` set GTK 4.20's `gtk-interface-color-scheme`; pages that offer both looks
follow it.

## Tested (all through the control socket, scratch profile)

| Check | Result |
|---|---|
| Three pages visited → `history` | three rows, titles filled in |
| `type you` | field completed to `youtube.com`; offers: the YouTube tab, then history |
| `type news y` | HN, by two words |
| `bookmark` → `bookmarks` | HN listed; star filled in the field |
| `find the` on Wikipedia | 267 matches |
| Local page asks for location → bar shown → `deny` | page got error code 1 (PERMISSION_DENIED) |
| Same page again | refused at once from the saved answer, no bar |
| Download the same file twice | `report.bin`, then `report (1).bin`, byte-identical; original untouched |
| Memory, one page open | rippl 122 MB, total ~351 MB (same as before Phase 4) |
| Binary | 863 KB |

## Bugs found and fixed

- **A long title in the offers widened the column to half the window.** GTK gives a box its
  children's *natural* width, which for an ellipsized label is its whole text. Every label in the
  column now caps its natural width (`max_width_chars(1)`), so the column stays at its set width
  and labels ellipsize.
- **The permission bar's text was unreadable** with a translucent theme: first faint, then light
  on light. The find and permission bars now have a fixed dark, opaque look with white text, which
  is readable over any page and any theme.
- **The Phase 3 ping setting did nothing.** WebKitGTK 2.54 logs "`webkit_settings_set_enable_hyperlink_auditing()`
  is deprecated and does nothing". The call was removed. Shield's rule that blocks third-party
  pings is the real protection, and [privacy.md](../privacy.md) now says so.

## Known limits

- History has no page of its own yet; you search it from the field. `ctl clear-history` clears
  everything; there's no "last hour" in the UI.
- Bookmarks are a flat list: no folders, no import from other browsers yet.
- Downloads don't ask where to save, and the list is lost at quit.
- Site permissions can only be reviewed and revoked through the control socket for now.
- Notifications use WebKitGTK's own display.
