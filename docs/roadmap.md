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
| 8 | Same-site tabs share a page process, rippl's look (`look.rs`), reopen closed tab, clean copy, paste and go, click IDs stripped, keywords, link status line, the feature map | ✅ done, [notes](phases/phase-8-look.md) |
| Feature batches | Batches 1–6 in [features.md](features.md): every feature that can exist on Linux | batches 1–3 ✅ ([1](phases/batch-1.md), [2](phases/batch-2.md), [3](phases/batch-3.md)); batch 4 ✅ ([notes](phases/batch-4.md)), 5 ✅ ([notes](phases/batch-5.md)), 6 ✅ ([notes](phases/batch-6.md)); Phase 9 ✅ |
| 9 | **Ship**: logo, `.desktop` file, AppStream, default browser, links from other apps, install script, Arch `PKGBUILD`, Flatpak, welcome | ✅ done, [notes](phases/phase-9-ship.md) |
| 10 | **Videos**: Ctrl+Shift+Y, paste a link, Return: yt-dlp downloads it (video or audio) into the downloads list, with pause, resume and cancel | ✅ done, [notes](phases/phase-10-grab.md) |
| 12 | **Everywhere**: rippl on macOS (WKWebView) and Windows (WebView2), one install command per system with yt-dlp, ffmpeg and Deno, release builds for all three | 🚧 in progress, [notes](phases/phase-12-everywhere.md) |
| 11 | **Polish**: one site's tabs as one row (Ctrl+Alt+G), videos at their best resolution and frame rate, Shield off for a site at once, splits in the session, capped HTTP cache | ✅ done, [notes](phases/phase-11-polish.md) |

## Ideas to measure later

- WPE WebKit instead of WebKitGTK, for a lighter UI process.
- A lighter UI-process renderer on hybrid-GPU laptops (both Vulkan drivers load today).
- Fingerprinting defences of rippl's own (WebKit has some built in).
- Keeping sleeping-tab state across restarts.

## Out of scope

- Chrome extensions: WebKitGTK has no WebExtensions API.
- Sync, accounts, any server of ours.
- An auto-updater: Linux packages update through the system package manager; on macOS and Windows, the install command again updates.
