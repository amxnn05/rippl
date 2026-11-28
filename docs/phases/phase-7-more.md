# Phase 7: Split view, folders, settings, HTTPS-only, Global Privacy Control

**Status:** done (2026-11-25)

## Goal

Two pages side by side; bookmarks in folders; settings changed from the settings page; and two
privacy features that need code inside WebKit's page processes: HTTPS-only and Global Privacy
Control.

## What was built

### rippl's web process extension (`crates/rippl-web-extension`)

- A small library (about 310 KB) that WebKit loads into **every page process**. It sees every
  request before it leaves.
- It is installed by copying it from beside the binary (or `/usr/lib/rippl`, or `RIPPL_EXTENSION`)
  into `~/.cache/rippl/web-extension/`. WebKit loads every library in the folder it is given and
  lets that folder into the page sandbox, so the folder holds only this one.
- Its settings travel in the **start-up data** WebKit hands each new page process:
  `(gpc, https_only, hosts allowed on http)`. They are also sent as messages to running processes
  when they change.
- **Memory:** page process 172 MB with it, 169–176 MB without it, so no measurable cost.
- **Safety:** WebKit's `send-request` signal passes NULL as the redirect response when there was no
  redirect, but the generated Rust binding wraps that argument in a type that can't be NULL, which
  is undefined behavior. The extension connects the signal with its own C callback, keeps the
  pointer raw and never touches it. `connect_raw` frees its closure as a `Box`, so the extension
  passes a real (empty) one, not NULL.

### Global Privacy Control (`gpc = on`, the default)

- Every request carries **`Sec-GPC: 1`**: the page and every image, script and frame. This is the
  signal that tells sites not to sell or share your data, and that laws like the CCPA treat as an
  opt-out.
- `navigator.globalPrivacyControl` is `true`, for sites that check it in script.

### HTTPS-only (`https_only = on`, the default)

- Every request for `http://` to a public host is rewritten to `https://` **before it leaves**: the
  page itself, its frames, images and scripts (`rippl_core::address::https_upgrade`). Left alone:
  - localhost, the local network, `.local` / `.lan` / `.internal` names and names with no dot
    (`http://router/`);
  - explicit ports other than 80 (`http://example.com:8080` is a server of its own).
- **When HTTPS doesn't work**, the page explains it instead of showing a bare error. The address
  failed, its certificate was invalid, or the site didn't answer within **8 seconds** (rather than
  waiting for the connection to time out). The page offers *"Continue to http://site anyway"*.
  Continuing allows that exact host over http until rippl quits.
- **Why it's done in the page process:** rewriting the request inside the UI process's
  policy signal is ignored by WebKit (tried: the page stayed on `http://`). That signal also can't
  tell the main frame from an iframe, so it can't redirect safely. The extension's `send-request`
  can change the request for every frame correctly.
- **How rippl knows a main-frame load was an upgrade:** the policy signal still sees the original
  `http://` address (from any frame); the load-started signal fires only for the main frame. A
  main-frame start that matches a recently seen upgrade starts the 8-second watch.

### Split view

- `Ctrl+Alt+S` puts the tab in front beside the tab you were on before it, or separates a split.
  Alt-click on a tab in the column puts that tab beside the one in front. `Ctrl+Alt+←/→` move
  between the panes.
- The pane you click into becomes the tab in front: the field, shortcuts, find and reading mode act
  on it. It gets a thin accent line on top, and both tabs of a pair have an accent bar in the column.
- **Both pages are on screen, so neither sleeps** (`due_for_sleep` now takes every on-screen tab,
  tested).
- Selecting another tab shows it alone; going back to either tab of the pair shows the pair again.
  Closing one shows the other alone.
- A view lives either in the page stack (one page) or in the divider (two), never both. `place` and
  `detach` (`split.rs`) are the only ways views move, so sleeping and closing work in both.

### Bookmark folders

- Bookmarks have a folder path (`Work/Docs`). Old databases get the column automatically.
- **Imports keep folders:** Chrome/Chromium paths, and Firefox's parent folders. Each browser's own
  top-level folders (bookmarks bar, other, menu, toolbar) are left out of the path.
- `rippl://bookmarks` groups bookmarks by folder. **Move…** asks for a folder name (`/` for a folder
  inside a folder, empty for none).

### Settings from the settings page

- `rippl://settings` shows each setting as a control: search engine, HTTPS-only, Global Privacy
  Control, ad blocking, history, theme, and minutes before tabs sleep.
- A change is **checked** (`Config::apply`, tested), **saved** to the config file without
  disturbing its other lines or comments (`rippl_core::settings_file`, tested), and **applied
  now**:
  - theme: immediately;
  - search engine and history: the next search or visit;
  - GPC and HTTPS-only: running page processes are told, and new pages use the new setting;
  - ad blocking: on or off after a restart (the page says so).
- An invalid value is refused with the reason.

## Tested

| Check | Result |
|---|---|
| Local server logging headers | `/` and `/pixel.gif` both arrived with `Sec-GPC=1` |
| `navigator.globalPrivacyControl` | `true` |
| `http://example.com/` | ended at `https://example.com/` |
| A subdomain whose HTTPS fails (via neverssl.com) | warning page after ~7 s; "Continue" loaded it over http, and the extension logged "allowed on http" |
| A host that never answers (`203.0.113.1`) | warning after ~9 s: "didn't answer within 8 seconds" |
| Message delivery to the extension | `allow-http` received; the next request was left on http |
| Split tabs 2+1, sleep timer 6 s, 16 s idle | **both split tabs awake**, the third asleep |
| Another tab, then back | single, then the pair again |
| `Ctrl+Alt+S` on a pair | separated |
| Search engine changed on the settings page | saved (no note since phase 11); file has `search = brave`; the next search went to Brave Search |
| `sleep = 0` on the settings page | refused: "\"0\" isn't a value for sleep" |
| Move a bookmark to `Reading/Later` | grouped under that folder |
| Memory with the extension | no measurable difference |
| Tests | 59 (55 core, 4 app), clippy clean |

## Bugs found and fixed

- **The first "allow http" didn't stick.** WebKit often starts a new page process for a navigation
  to another site, and that process never received the message sent earlier. The allowed hosts are
  now part of the start-up data every new page process gets, as well as being messaged to running
  ones.
- **The 8-second watch never fired** at first. When the load starts, the view still reports the
  `http://` address it asked for (the page process rewrites it later), and the watch looked only
  for the `https://` form. It now accepts either. A second condition (`is_loading`) was also wrong
  for a connection still being made; "still waiting" now means "no response yet".
- **Testing mistake, recorded so it isn't repeated:** `neverssl.com` itself now answers over HTTPS;
  its *random subdomains* don't. Tests have to look at which host actually failed.

## Known limits

- Hosts allowed on http are forgotten at quit.
- Split view pairs aren't saved in the session; tabs come back separate.
- HTTPS-only upgrades subresources too. An http-only image on an http page you chose to continue to
  is allowed only if it's on the same host.
- If a site answers HTTPS with a *different* page than over http, rippl can't tell.
- Turning ad blocking on or off still needs a restart.
