# Phase 12: everywhere (macOS, Windows, one install command each)

**Status:** 1.0.0 beta released 2026-11-28 for macOS (by the install command only) and Linux.
Windows waits for signing; the rest of the macOS port continues.

## Goal

rippl on macOS and Windows as well as Linux, installed with one command on each, with the
programs videos need (yt-dlp, ffmpeg, Deno), and built and released for all three by CI.

## How it is built

- **Linux stays the GTK 4 build** (`app/`) on WebKitGTK 6.0, packaged as a Flatpak for the
  install command (GNOME 50 runtime: WebKitGTK 2.52).
- **macOS and Windows** are `desktop/` (`rippl-desktop`, installed as `rippl`): a `tao` window
  holding `wry` views. The column, the strip above the page and the new tab are one view of
  rippl's own page (`desktop/ui/chrome.html`, `chrome.css`, `chrome.js`), drawn from a state
  message (`Browser::state`) and sending back what you do (`events.rs`). Each awake tab is a view
  of the system's engine laid exactly over the column's page card (it reports where the card is).
  - macOS: WKWebView, the system's WebKit, so pages behave as in Safari; Shield's rules are the
    WebKit JSON the Linux build compiles, as WebKit content rule lists.
  - Windows: WebView2 (Edge's engine, part of Windows 10 and 11). It has no content rules, so
    every request goes past Brave's filter engine (`adblock`), built from the same lists.
  - Linux (for developing this build only): wry's WebKitGTK 4.1 port.
- **Shared with the Linux build** (`rippl-core`): the tab model, spaces, groups, the session,
  history and bookmarks (`places.sqlite`), suggestions, every `rippl://` page, the config file
  (moved into `rippl_core::config`), Shield's lists and conversion, yt-dlp's arguments and
  progress, download names and the downloads list, and the page scripts (`app/src/*.js`).
  New in core: `dirs` (each system's folders) and `keys` (GTK's key names, read without GTK).
- **Keys**: the same command names and keys as the Linux build (`desktop/src/commands.rs`); on
  macOS Ctrl is ⌘ (not with Tab). Pages and the column listen for them (`ui/page.js`).
- **Videos**: yt-dlp is found in rippl's `tools/` folder or on PATH; if neither, the macOS and
  Windows builds fetch yt-dlp's standalone build, ffmpeg and Deno into `tools/` the first time
  (`desktop/src/grab.rs`). The Linux build looks in `tools/` too, so the Flatpak finds what the
  install command put there.

## Installing

- macOS and Linux: `curl -fsSL https://ripplbrowser.web.app/install.sh | sh` (rippl-website
  `public/install.sh`): rippl.app into /Applications (or ~/Applications), or the Flatpak for
  you alone; then yt-dlp, ffmpeg and Deno into rippl's tools folder.
- Windows: `irm https://ripplbrowser.web.app/install.ps1 | iex` (`public/install.ps1`): the
  installer, silently, for you alone (it gets WebView2 if missing), then the same tools into
  `%APPDATA%\rippl\tools`. The installer (`packaging/windows/rippl.iss`, Inno Setup) also stands
  alone, and registers rippl with Default apps.
- `.github/workflows/release.yml` builds all three on every push to a `release-*` branch, starts
  each once (`RIPPL_CONTROL`, a screenshot of Settings), and publishes them on a `v*` tag.

## Released, and held back

- **Windows is held back.** Smart App Control (on by default on new Windows 11 installs) refuses
  any unsigned program, from the terminal or a double-click, with no way past it but turning it
  off for good. Windows waits for a signed build: the Microsoft Store (which signs it), or a
  certificate (SignPath Foundation, Certum). CI still builds and tries it on every push.
- **No .dmg.** Unsigned, a downloaded .dmg needs Privacy & Security › Open Anyway on macOS 15.
  Installed by the command (curl leaves no quarantine mark), rippl opens as any app. The
  release carries only `rippl-macos-universal.tar.gz` and the Flatpak; `install.ps1` is gone
  from the website.

## Decisions

- **The column as a web page on macOS and Windows.** GTK 4 runs there, but WebKitGTK doesn't, and
  a native view laid into a GTK window would hide every bar rippl puts over a page. One page for
  the chrome gives both systems the same look as Linux from the same CSS values.
- **A shared tools folder, not bundled programs.** yt-dlp changes weekly (sites change); a copy
  in rippl's data folder can update itself, a copy inside the app can't (macOS signs it).
- **Assets named without versions** (`rippl-macos-universal.tar.gz`, …) so
  `releases/latest/download/…` always works for the install commands.
- **Unsigned for now**: the app is signed ad hoc on macOS; downloaded by curl it isn't
  quarantined. SmartScreen may warn about the Windows installer.

## Bugs found

- **A window that never appeared (Linux test).** Setting the window's theme fires a theme-change
  event, which set the theme again: a busy loop before the first frame. The theme is now set only
  when it differs, and the event only redraws.
- **`rippl://` failed in a second tab (WebKitGTK).** A scheme belongs to the engine there, not
  to a view; it is registered once.
- **Windows build: adblock 0.10** no longer compiled (its msgpack dependency changed); 0.13.
- **YouTube without ffmpeg**: "Requested format is not available" (YouTube serves picture and
  sound apart). That is why ffmpeg and Deno come with yt-dlp.

## Measurements

| What | Result |
|---|---|
| CI: Windows build, installer, start, open Settings | passes; installer 5.0 MB, portable zip 3.7 MB |
| CI: Linux Flatpak (GNOME 50) | builds in about 5 minutes |
| A short YouTube video, fetched yt-dlp + system ffmpeg (Linux test) | .mp4 in about 4 s, on the downloads list |
| Install command's tools (Linux) | yt-dlp 41 MB, ffmpeg 179 MB, Deno 96 MB |

## What the macOS and Windows builds have, and what is still to come

Done: the column (tabs, pins, groups, site groups, spaces with colours), the field with history
and search suggestions, new tab, back/forward/reload, the session, sleeping tabs, `rippl://`
history, bookmarks, downloads, settings, archive, welcome and passwords pages, every setting,
your own keys, Shield (with per-site off), downloads with progress, videos through yt-dlp, find
in page, zoom, bookmarks bar, tabs across the top, icon column, split view (2–4), duplicate,
reopen closed, copy clean address, paste and go, print, inspector, the site card and tab menus
(native), YouTube's ads, video keys, autoscroll, elastic edges (Windows), light and dark.

To come before 1.0.0: Ctrl+Tab switcher, peek, reading mode, hiding elements, capture, more
windows, saving and filling passwords, the AI add-on and Translate, folder sync, painted
new-tab walls, the player, tabs closing themselves, pin folders, permission questions of
rippl's own, HTTPS-only's fallback page, Global Privacy Control, client certificates, the
cache cap, agents over MCP, bringing things over from other browsers' folders on macOS and
Windows.

## Known limits

- A sleeping tab on macOS and Windows keeps its address, not its back/forward list.
- A pop-up opens as a tab without `window.opener`.
- Camera, microphone and location are asked by the system's own prompt.
