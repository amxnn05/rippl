# rippl documentation

rippl is a small, fast, private web browser for Linux. It is built on WebKit (WebKitGTK 6.0 + GTK4) and
written in Rust. These pages explain how it works, why it is built the way it is, and what each
development phase did.

| Page | What it covers |
|---|---|
| [architecture.md](architecture.md) | Processes, crates, the tab lifecycle, and where everything lives on disk |
| [privacy.md](privacy.md) | What leaves your machine, the search engine choice, and every protection |
| [configuration.md](configuration.md) | The config file, the Shield allow list, environment knobs, keys |
| [testing.md](testing.md) | Unit tests, the control socket, measuring memory, testing on Hyprland |
| [performance.md](performance.md) | Size, startup time, memory against Chromium and Brave, and what costs what |
| [features.md](features.md) | Every features, with rippl's status and the batch it's planned in |
| [roadmap.md](roadmap.md) | Every phase, done and planned |
| [phases/](phases/) | One page per finished phase: what was built, decisions, bugs, measurements |

## Phases so far

| Phase | Status | Page |
|---|---|---|
| 0. Setup: workspace, WebKit, measuring | done | [phase-0-setup.md](phases/phase-0-setup.md) |
| 1. A usable browser: field, tabs, session | done | [phase-1-browser.md](phases/phase-1-browser.md) |
| 2. Memory: lazy tabs, sleep, pressure, limits | done | [phase-2-memory.md](phases/phase-2-memory.md) |
| 3. Shield (ad/tracker blocker) and privacy | done | [phase-3-shield.md](phases/phase-3-shield.md) |
| 4. Daily use: history, suggestions, bookmarks, downloads, find, permissions, theme | done | [phase-4-daily-use.md](phases/phase-4-daily-use.md) |
| 5. Extras: pins, reading mode, hide elements, rippl:// pages, bookmark import | done | [phase-5-extras.md](phases/phase-5-extras.md) |
| 6. Passwords in the system keyring | done | [phase-6-passwords.md](phases/phase-6-passwords.md) |
| 7. Split view, bookmark folders, settings editing, HTTPS-only, Global Privacy Control | done | [phase-7-more.md](phases/phase-7-more.md) |
| 8. Shared processes for same-site tabs, the look, first features | done | [phase-8-look.md](phases/phase-8-look.md) |
| Feature batch 1: site card, keep awake, pop-ups, duplicate, typos, site search, print | done | [batch-1.md](phases/batch-1.md) |
| Feature batch 2: spaces, profiles, tab groups (and dark mode fixed) | done | [batch-2.md](phases/batch-2.md) |
| Feature batch 3: uBlock's lists, glass, dark pages, site CSS, capture, icons, new tab, YouTube ads, peek, unread, player, video keys, archives | done | [batch-3.md](phases/batch-3.md) |
| Feature batch 4: screen-rate pages, autoplay, autoscroll, closing tabs, archive, tab switcher, bookmarks bar, icon column, top tabs, 4-page split, shortcuts, space colours, new-tab walls, pin folders, windows | done | [batch-4.md](phases/batch-4.md) |
| Feature batch 5: moving in (history, tabs, passwords), folder sync, proxy, client certificates, webmail, download pause/resume, media keys | done | [batch-5.md](phases/batch-5.md) |
| Feature batch 6: Settings, the AI add-on, Translate page, agents over MCP | done | [batch-6.md](phases/batch-6.md) |
| Phase 9: shipping (desktop entry, default browser, packages) | next | [roadmap.md](roadmap.md) |
| 9. Ship: desktop entry, default browser, packages | planned | [roadmap.md](roadmap.md) |

## Keeping these docs current

Every phase adds a page under `phases/` and updates the pages it touches: architecture, privacy,
configuration, testing and the roadmap. A change that alters what rippl sends over the network
**must** update [privacy.md](privacy.md) in the same commit.
