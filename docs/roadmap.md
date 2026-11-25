# Roadmap

| Phase | What | Status |
|---|---|---|
| 0 | Workspace, WebKitGTK 6.0, memory measuring | ✅ done, [notes](phases/phase-0-setup.md) |
| 1 | Field, tabs, shortcuts, session, private tabs | ✅ done, [notes](phases/phase-1-browser.md) |
| 2 | Lazy restore, tab sleep, PSI pressure, process limits, trim, control socket | ✅ done, [notes](phases/phase-2-memory.md) |
| 3 | Shield (EasyList + EasyPrivacy), privacy defaults, private search engines, config | ✅ done, [notes](phases/phase-3-shield.md) |
| 4 | History + ranked suggestions + inline completion, bookmarks, downloads, find in page, permission prompts, theme | ✅ done, [notes](phases/phase-4-daily-use.md) |
| 5 | Pinned tabs, reading mode, hide elements for good, `rippl://` history/bookmarks/settings pages, bookmark import (Chromium family, Firefox family) | ✅ done, [notes](phases/phase-5-extras.md) |
| 6 | **Passwords**: sign-in form detection, save after a sign-in works, offer saved accounts under the field, Secret Service keyring (`oo7`), never filled without a click, never offered to another site | next |
| 7 | **More**: split view, bookmark folders, editing settings from `rippl://settings`, Global Privacy Control header (web-process extension), HTTPS-only mode, instant per-site Shield toggle, capped HTTP cache | planned |
| 8 | **Ship**: `.desktop` file, icon, default-browser registration, AUR `PKGBUILD`, Flatpak | planned |

## Ideas to measure later

- WPE WebKit instead of WebKitGTK, for a lighter UI process.
- A lighter UI-process renderer on hybrid-GPU laptops (both Vulkan drivers load today).
- Sharing one web process between same-site tabs (less memory, weaker isolation).
- Keeping sleeping-tab state across restarts.

## Out of scope

- Chrome extensions: WebKitGTK has no WebExtensions API.
- Sync, accounts, any server of ours.
- An auto-updater: Linux packages update through the system package manager.
