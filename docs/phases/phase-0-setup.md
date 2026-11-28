# Phase 0: Setup

**Status:** done (2026-11-25)

## Goal

Get a Rust program to show a web page with WebKit on Linux, and be able to measure its memory from
the first day.

## What was built

- A Cargo workspace: `crates/rippl-core` (pure logic) and `app` (the browser binary `rippl`).
- WebKitGTK 6.0 (the GTK4 version) via the `webkit6` crate 0.6.1. It re-exports `gtk`, `gio`, `glib`
  and `soup`, so rippl uses those re-exports and never has a second, mismatched GTK version.
- `scripts/mem.sh`: adds up the PSS of rippl and every process it started.
- Release profile: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip`. The binary is
  about **480 KB**.

## Decisions

| Decision | Why |
|---|---|
| WebKitGTK 6.0, not 4.1 | 4.1 is the GTK3 build; 6.0 is GTK4, which is current and what new work targets |
| WebKitGTK, not WPE | WPE is lighter, but the embedder has to do its own rendering and input. Start with WebKitGTK; WPE stays an option later |
| A separate `rippl-core` crate | Logic that needs no display can be unit-tested, which keeps its behaviour pinned down |
| No serde, no URL crate | Fewer dependencies, faster builds; the formats are tiny |

## Problems hit

- **`webkit6` 0.6.1 fails to build** (`cannot find type Accessible in crate gtk`). Its generated code
  needs GTK ≥ 4.10 types, but doesn't turn on the matching `gtk4` feature. Fix: enable `webkit6`'s
  `gtk_v4_18` feature (the system has GTK 4.24).
- Arch ships WebKitGTK 6.0 as the package `webkitgtk-6.0`; only `webkit2gtk-4.1` was installed.

## Setup on a new machine

```sh
sudo pacman -S --needed webkitgtk-6.0 gtk4        # Arch
# Debian/Ubuntu: sudo apt install libwebkitgtk-6.0-dev libgtk-4-dev
cargo build --release
```
