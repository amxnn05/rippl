# Architecture

## The one idea

A browser's memory is mostly its **web processes**: one per open page, typically 100–700 MB each.
rippl's own UI process is about 120 MB, and Rust only makes that part smaller. Most of rippl's
memory savings come from keeping **as few web processes alive as possible**, and from not letting
ads and trackers load into the ones that are.

This is the same approach as another browser, a WebKit browser for the Mac.
rippl started as a Linux port of its ideas (see the map at the bottom).

## Processes

```
rippl                      the UI: GTK4 window, tab model, Shield, places, timers (~120 MB)
 ├── WebKitNetworkProcess  one, shared: HTTP, cookies, cache, content filters     (~50 MB)
 └── WebKitWebProcess × N  one per *awake* tab; lazy and sleeping tabs have none  (100–700 MB each)
```

WebKitGTK runs each web process inside a `bwrap` sandbox with an `xdg-dbus-proxy`, which is why
those also show up in `ps` and `scripts/mem.sh`.

## Code layout

```
crates/rippl-core/        no GTK, no WebKit: everything here runs under `cargo test`
  src/address.rs          what was typed → URL or search; private search engines
  src/tabs.rs             Tab, Tabs, Life (Lazy/Awake/Asleep), the sleep policy
  src/session.rs          last session's tabs, as a small text file
  src/shield.rs           EasyList/ABP syntax → WebKit content-blocker JSON
  src/places.rs           places.sqlite: history, bookmarks, permission answers
  src/suggest.rs          ranking what the field offers; inline completion
  src/downloads.rs        safe, never-overwriting download file names
  src/pages.rs            rippl:// pages and the reader page, as escaped HTML
  src/import.rs           bookmarks from Chromium- and Firefox-family browsers
  examples/shield_convert.rs   runs the converter on a list file, for inspection

app/src/
  main.rs                 GtkApplication; one window; starts the control socket
  config.rs               ~/.config/rippl/config
  engine.rs               the ONLY place WebKit is set up: context, network
                          session, settings, memory limits, cookie policy
  browser.rs              the window: column (field + tabs), page stack,
                          tab lifecycle, shortcuts, timers, session saving
  shield.rs               downloads, compiles and swaps Shield's filters
  omnibox.rs              the field while typing: offers, completion, ↑↓
  find.rs                 find-in-page bar (Ctrl+F)
  permissions.rs          the permission question bar and its rules
  downloads.rs            the downloads list at the foot of the column
  reader.rs               reading mode: extractor (isolated world) + reader page
  curtain.rs              hide elements: picker, per-site user style sheets
  internal_pages.rs       serves rippl://history|bookmarks|settings, token-checked buttons
  pressure.rs             Linux PSI reading; malloc_trim
  control.rs              Unix socket for tests and measurement (opt-in)

scripts/mem.sh            PSS of rippl and all its WebKit processes
scripts/ctl.sh            sends one command to the control socket
```

Rule: anything that can be decided without a window or an engine goes in `rippl-core`, with tests.

## The tab lifecycle

```
            open / restore                    select (first time)
   (none) ───────────────▶ Lazy ─────────────────────────────▶ Awake
                       url + title only                 WebView + web process
                       no process                              │      ▲
                                                 unseen 30 min │      │ select
                                                 or pressure   ▼      │ restore state
                                                             Asleep ───┘
                                                   session state (~100 B–KB)
                                                   no view, no process
```

- **Lazy**: tabs restored from the last session, or opened in the background from the command line.
- **Awake**: has a `WebView`. The front tab is the `gtk::Stack`'s visible child; the others stay in
  the stack hidden, which WebKit throttles. Switching tabs only changes the visible child, so nothing
  reloads.
- **Asleep**: `WebView::session_state()` (back/forward list, scroll position) is serialized into the
  tab. The view is removed and its web process is ended with `terminate_web_process()`. Dropping the
  view alone is not enough, because WebKit keeps the process cached for reuse (see Phase 2). Waking
  makes a new view, calls `restore_session_state` and goes to the current history item.

**Never sleeps:** the front tab, pinned tabs, tabs playing audio, tabs using the camera, microphone
or screen, and tabs still loading. The policy is `rippl_core::tabs::due_for_sleep`.

**Families:** a page that opens a window (`window.open`, `target=_blank`) gets a new tab whose view
is *related* to the opener. WebKit requires this, and the two share one web process. Each tab
records its family, and a process is only ended when no awake tab of the family is left.

## Signal safety

GTK and WebKit signals can fire while rippl is changing state; for example, closing a view fires
its `close` signal. Two rules keep that safe:

1. No `RefCell` borrow is held across a call into GTK or WebKit.
2. Every per-view signal handler first checks `Browser::ours(id, view)`. A view rippl is already
   taking away can no longer change its tab. Without this, sleeping a tab used to *delete* it.

## Isolated script world

rippl's own scripts (the reading-mode extractor, the hide picker) run in a separate JavaScript
world named `rippl`. They see the same page, but the page can't see their variables, call their
functions or post to their message handler (`ripplHide` is registered only in that world). This
is the same idea `WKContentWorld "Search"`.

## rippl:// pages

`WebContext::register_uri_scheme("rippl")` serves pages built by `rippl_core::pages`. The scheme is
local + display-isolated + secure, so the web can't reach it. Buttons carry a per-launch random
token (see [phase 5](phases/phase-5-extras.md#rippls-own-pages-ripplhistory-ripplbookmarks-ripplsettings)).

## Shield (content blocking)

```
easylist.txt ─┐                         ┌─ UserContentFilterStore.save ─▶ compiled filter (on disk)
easyprivacy ──┼─ rippl_core::shield ─▶ JSON ┘                                   │ load at launch (5 ms)
seed (built in)┘   convert (~60 ms, off the main thread)                        ▼
                                              UserContentManager (shared by every view)
```

Filters are enforced by WebKit itself in the network process and style engine, with no JavaScript
in pages. Details: [phases/phase-3-shield.md](phases/phase-3-shield.md).

## Files on disk

| Path | What |
|---|---|
| `~/.local/share/rippl/session` | open tabs (not private ones), plain text |
| `~/.local/share/rippl/places.sqlite` | history, bookmarks, site permission answers, hidden elements |
| `~/.cache/rippl/import/` | a moment's copy of a Firefox database while importing, deleted after |
| `~/.local/share/rippl/web/cookies.sqlite` | cookies (sign-ins) |
| `~/Downloads` (XDG download dir) | downloaded files |
| `~/.local/share/rippl/web/storage/`, `itp/` | site storage, WebKit's tracking-prevention data |
| `~/.cache/rippl/web/` | HTTP cache |
| `~/.cache/rippl/shield/*.txt`, `*.stamp` | downloaded filter lists and when they were fetched |
| `~/.cache/rippl/shield/compiled/` | WebKit's compiled filters |
| `~/.config/rippl/config` | settings ([configuration.md](configuration.md)) |
| `~/.config/rippl/shield-allow` | sites Shield is off for |
| `$XDG_RUNTIME_DIR/rippl.sock` | control socket, only with `RIPPL_CONTROL=1` |

## Map from the Mac browser (macOS) to rippl (Linux)

| the Mac browser (Swift, WKWebView) | rippl (Rust, webkit6) |
|---|---|
| `WKWebView` | `webkit6::WebView` |
| shared `WKProcessPool` | one `WebContext` |
| `WKWebsiteDataStore` / `.nonPersistent()` | `NetworkSession::new(data, cache)` / `new_ephemeral()` |
| `WKContentRuleListStore` | `UserContentFilterStore` (same JSON format) |
| `interactionState` | `session_state()` / `restore_session_state()` |
| DispatchSource memory pressure | PSI, `/proc/pressure/memory` |
| `malloc_zone_pressure_relief` | `malloc_trim(0)` |
|  socket | `control.rs` socket |
| (none on macOS) | `MemoryPressureSettings`: a memory limit for each web process |
| `WKWebExtension` (Chrome extensions) | none in WebKitGTK, so no extensions |
