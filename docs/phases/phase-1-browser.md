# Phase 1: A usable browser

**Status:** done (2026-11-25)

## Goal

A browser you can actually browse with: one field, tabs, keyboard shortcuts, and your tabs back
after a restart.

## What was built

- **The window** (`browser.rs`): a column on the left with the address field on top and the tabs
  below, and the page on the right. There is no toolbar. On tiling compositors (Hyprland, Sway,
  river, niri, i3) the window has no title bar; elsewhere GTK's own title bar keeps it movable.
- **One field**: it opens an address
  if it clearly is one, and searches otherwise.
  - `example.com` → `https://example.com`
  - `localhost:3000`, `192.168.1.1`, `printer.local` → `http://` (local servers have no certificates)
  - `0.0.0.0:8080` → `http://localhost:8080` (WebKit refuses `0.0.0.0`)
  - `hello world`, `todo`, `1.2`, `me@mail.com`, `note:buy` → searched
  - `/path/to/file`, `~/file` → `file://`, if the file exists
  - Only `http`, `https`, `file`, `about` and `data` schemes are followed; `javascript://` is refused.
- **Tabs**: new tabs open after the current one. Closing one moves to the next tab (or the previous
  one at the end). Middle-click closes a tab. Each tab shows a close button on hover.
- **Links that open a window** become tabs. A middle-clicked link opens behind, anything else in
  front. A page calling `window.close()` closes its tab.
- **Private tabs** (`Ctrl+Shift+N`): each has its own ephemeral network session.
- **Session** (`rippl_core::session`): a plain-text file, saved every 30 s if something changed, on
  window close and on `SIGTERM`. It is written atomically (temporary file, then rename) and never
  includes private tabs. A damaged line loses that tab, not the whole session.
- **Crash recovery**: if the front tab's web process crashes, the page reloads. A background tab
  whose process crashes goes to sleep and reloads when you select it.
- **Shortcuts**: see [configuration.md](../configuration.md#keys). They work while a page has focus.

## Decisions

| Decision | Why |
|---|---|
| Field at the top of the tab column | Like Arc: one place to type, no toolbar row stealing height |
| `gtk::Stack` of views | Switching tabs is a visibility change; the page doesn't reload or lose its scroll |
| User agent left as WebKitGTK writes it | It already claims Safari. it found that adding an app name made Google serve a degraded page |
| Session as text, not JSON | Readable by hand, no parser dependency, and it tolerates damage |

## Bugs found later and fixed

- **Sign-ins were lost on every restart** (found in Phase 3). WebKitGTK only keeps cookies on disk
  if the browser calls `CookieManager::set_persistent_storage`; there was no `cookies.sqlite`
  until then. Fixed in `engine.rs`.
- **Closing a tab could run twice** (found in Phase 2): see the `close` signal bug there.
