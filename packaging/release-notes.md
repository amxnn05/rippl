rippl 1.0.0: the first release, for Linux, macOS and Windows.

## Install

**macOS and Linux**, in a terminal:

```sh
curl -fsSL https://ripplbrowser.web.app/install.sh | sh
```

**Windows**, in PowerShell:

```powershell
irm https://ripplbrowser.web.app/install.ps1 | iex
```

Each installs rippl, and yt-dlp, ffmpeg and Deno for downloading videos (Ctrl+Shift+Y). Or take a file below: `rippl-macos-universal.dmg` (Apple silicon and Intel), `rippl-windows-x64-setup.exe`, `rippl-linux-x86_64.flatpak`.

## What runs where

- **Linux**: the full rippl, on WebKitGTK (GTK 4), as a Flatpak (any distro).
- **macOS**: on the system's WebKit. **Windows**: on WebView2, Edge's engine, part of Windows 10 and 11.
  Both have the column with tabs, pins, groups and spaces; the field with suggestions; every `rippl://` page and setting; your own shortcuts; Shield; downloads and videos; find; zoom; split view; Ctrl+Tab; the session; light and dark.
  Still to come there: peek, reading mode, hiding elements, capture, more windows, saving passwords, the AI add-on, sync, painted new-tab walls, the player and a few more (see docs/phases/phase-12-everywhere.md).

## Not signed yet

The macOS app and the Windows installer aren't signed by Apple or Microsoft yet. Installed with the commands above, they open without a warning. Downloaded in a browser:
- **macOS**: right-click rippl › Open the first time, or System Settings › Privacy & Security › Open Anyway.
- **Windows**: if SmartScreen says "Windows protected your PC", choose More info › Run anyway.
