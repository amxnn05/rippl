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
| 5. Extras: passwords, reader mode, hide elements, pins, split view | next | [roadmap.md](roadmap.md) |

## Keeping these docs current

Every phase adds a page under `phases/` and updates the pages it touches: architecture, privacy,
configuration, testing and the roadmap. A change that alters what rippl sends over the network
**must** update [privacy.md](privacy.md) in the same commit.
