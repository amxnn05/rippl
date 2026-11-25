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
       └── librippl_web_extension.so   GPC header, HTTPS-only (no measurable memory)
```

The UI process talks to the extension with WebKit user messages (`rippl-settings`,
`rippl-allow-http`), and gives each new page process its start-up data:
`(gpc, https_only, allowed hosts)`.

WebKitGTK runs each web process inside a `bwrap` sandbox with an `xdg-dbus-proxy`, which is why
those also show up in `ps` and `scripts/mem.sh`.

## Code layout

```
crates/rippl-web-extension/   loaded into every page process: Sec-GPC header, HTTPS upgrades
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
  src/passwords.rs        origins, which pages may save, save/update/nothing
  src/settings_file.rs    change one line of the config file, keep the rest
  src/clean.rs            click IDs and campaign tags off addresses
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
  internal_pages.rs       serves rippl://history|bookmarks|passwords|settings, token-checked buttons
  keyring.rs              the Secret Service through libsecret (or memory, for tests)
  signin.rs               form watching, the save question, the account list, filling
  split.rs                split view: place/detach views, pairs, pane focus
  look.rs                 the design: palette, column rows, letters, pins, CSS
  closed.rs               reopen closed tabs, copy clean address, paste and go
  sitecard.rs             site card, tab menu, duplicate, print, blocked pop-ups
  spaces.rs               spaces: switch, new, rename, delete, move tabs; the foot's buttons
  pagelook.rs             dark pages, your CSS per site, capture, site icons
  home.rs                 the new tab: the field in the middle of the page, the Home grid
  pagescripts.rs          scripts switched by settings: YouTube's ads, video keys
  youtube_page.js         YouTube: the ad schedule out of the player's data (page world)
  youtube_skipper.js      YouTube: an ad that plays anyway, muted at 16x (rippl's world)
  peek.rs                 Shift+click: a link's page in a panel over this one
  player.rs               what plays in another tab, at the column's foot
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
is *related* to the opener. WebKit requires this, and the two share one web process. Since Phase 8
a tab that wakes on the same site as an awake tab is made related to it too, sharing its process
(~160 MB saved per extra tab of a site). Each tab records its family, and a process is only ended
when no awake tab of the family is left.

## Signal safety

GTK and WebKit signals can fire while rippl is changing state; for example, closing a view fires
its `close` signal. Two rules keep that safe:

1. No `RefCell` borrow is held across a call into GTK or WebKit.
2. Every per-view signal handler first checks `Browser::ours(id, view)`. A view rippl is already
   taking away can no longer change its tab. Without this, sleeping a tab used to *delete* it.

## Isolated script world

rippl's own scripts (the reading-mode extractor, the hide picker, the sign-in form watcher) run in a separate JavaScript
world named `rippl`. They see the same page, but the page can't see their variables, call their
functions or post to their message handlers (`ripplHide` and `ripplPasswords` are registered only in that world). This
is the same idea `WKContentWorld "Search"`.

## rippl:// pages

`WebContext::register_uri_scheme("rippl")` serves pages built by `rippl_core::pages`. The scheme is
local + display-isolated + secure, so the web can't reach it. Buttons carry a per-launch random
token (see [phase 5](phases/phase-5-extras.md#rippls-own-pages-ripplhistory-ripplbookmarks-ripplsettings)).

## Shield (content blocking)

```
EasyList + uBlock filters + Quick fixes + Unbreak ─┐                ┌─ "ads" filter ─────┐
EasyPrivacy + uBlock Privacy + Badware + Unbreak ──┼─ convert ─▶ JSON ┤                    ├─ UserContentManager
seed (built in, first launch only) ────────────────┘  (~100 ms)       └─ "privacy" filter ─┘   (shared by every view)

A bundle is compiled as one filter because an exception only cancels rules of its own filter.
The last compiled filters load at launch in ~10 ms and block while a newer one is built.
```

Filters are enforced by WebKit itself in the network process and style engine, with no JavaScript
in pages. Details: [phases/phase-3-shield.md](phases/phase-3-shield.md).

## Files on disk

| Path | What |
|---|---|
| `~/.local/share/rippl/session` | open tabs (not private ones), spaces, groups: plain text, format 2 |
| `~/.local/share/rippl/spaces/<id>/web/` | a space's own cookies and site data (space 0 uses `web/`) |
| `~/.local/share/rippl/profiles/<name>/`, `~/.cache/rippl/profiles/<name>/` | a profile: all of the above, its own |
| `~/.local/share/rippl/places.sqlite` | history, bookmarks, site permission answers, hidden elements |
| System keyring (GNOME Keyring …) | saved passwords, items "rippl: user on site" |
| `~/.cache/rippl/web-extension/` | the copy of the web process extension WebKit loads |
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
