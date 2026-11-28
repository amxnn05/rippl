rippl 1.0.0 beta, for macOS and Linux.

## Install

In a terminal, on macOS (Apple silicon and Intel) or Linux:

```sh
curl -fsSL https://ripplbrowser.web.app/install.sh | sh
```

It installs rippl, and yt-dlp, ffmpeg and Deno for downloading videos (Ctrl+Shift+Y). On Linux rippl comes as a Flatpak (any distro); `rippl-linux-x86_64.flatpak` below is the same.

Windows is coming through the Microsoft Store: Windows only runs signed apps on many PCs (Smart App Control), and the Store signs them.

## What runs where

- **Linux**: the full rippl, on WebKitGTK (GTK 4).
- **macOS**: on the system's WebKit: the column with tabs, pins, groups and spaces; the field with suggestions; every `rippl://` page and setting; your own shortcuts; Shield; downloads and videos; find; zoom; split view; Ctrl+Tab; the session; light and dark. Still to come there: peek, reading mode, hiding elements, capture, more windows, saving passwords, the AI add-on, sync and a few more.

The Mac app isn't signed by Apple yet. Installed with the command it opens as any app; that's why there is no .dmg for now.
