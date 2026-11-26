# Roadmap

| Phase | What | Status |
|---|---|---|
| 0 | Workspace, WebKitGTK 6.0, memory measuring | ✅ done, [notes](phases/phase-0-setup.md) |
| 1 | Field, tabs, shortcuts, session, private tabs | ✅ done, [notes](phases/phase-1-browser.md) |
| 2 | Lazy restore, tab sleep, PSI pressure, process limits, trim, control socket | ✅ done, [notes](phases/phase-2-memory.md) |
| 3 | Shield (EasyList + EasyPrivacy), privacy defaults, private search engines, config | ✅ done, [notes](phases/phase-3-shield.md) |
| 4 | History + ranked suggestions + inline completion, bookmarks, downloads, find in page, permission prompts, theme | ✅ done, [notes](phases/phase-4-daily-use.md) |
| 5 | Pinned tabs, reading mode, hide elements for good, `rippl://` history/bookmarks/settings pages, bookmark import (Chromium family, Firefox family) | ✅ done, [notes](phases/phase-5-extras.md) |
| 6 | Passwords in the system keyring (libsecret): saved after a sign-in works, offered under the box for the exact origin, never filled without a click | ✅ done, [notes](phases/phase-6-passwords.md) |
| 7 | Split view, bookmark folders (import keeps them), settings edited from `rippl://settings`, HTTPS-only, Global Privacy Control (web process extension) | ✅ done, [notes](phases/phase-7-more.md) |
| 8 | Same-site tabs share a page process, the look (`look.rs`), reopen closed tab, clean copy, paste and go, click IDs stripped, keywords, link status line, the features map | ✅ done, [notes](phases/phase-8-look.md) |
| Feature batches | Batches 1–6 in [features.md](features.md): every features that can exist on Linux | batches 1–3 ✅ ([1](phases/batch-1.md), [2](phases/batch-2.md), [3](phases/batch-3.md)); batch 4 ✅ ([notes](phases/batch-4.md)), 5 ✅ ([notes](phases/batch-5.md)), 6 ✅ ([notes](phases/batch-6.md)); next: Phase 9, shipping |
| 9 | **Ship**: `.desktop` file, icon, default-browser registration, install layout (`/usr/lib/rippl` for the extension), AUR `PKGBUILD`, Flatpak, welcome | planned |
| Polish | instant per-site Shield toggle, split pairs in the session, capped HTTP cache | planned |

## Ideas to measure later

- WPE WebKit instead of WebKitGTK, for a lighter UI process.
- A lighter UI-process renderer on hybrid-GPU laptops (both Vulkan drivers load today).
- Sharing one web process between same-site tabs (less memory, weaker isolation).
- Keeping sleeping-tab state across restarts.

## Out of scope

- Chrome extensions: WebKitGTK has no WebExtensions API.
- Sync, accounts, any server of ours.
- An auto-updater: Linux packages update through the system package manager.
