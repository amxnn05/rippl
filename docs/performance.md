# Performance and size

Measured 2026-11-25 on the development machine: Arch Linux (Omarchy), Hyprland on Wayland, a hybrid
NVIDIA + AMD laptop, WebKitGTK 2.54.1. Memory is **PSS** (`scripts/mem.sh`), so shared pages are
split fairly between processes. Every browser used a fresh profile. Rerun these when something
changes and update the tables.

## Size

| | On disk |
|---|---|
| `rippl` binary | **1.2 MB** |
| `librippl_web_extension.so` | 0.3 MB |
| WebKitGTK 6.0 (system library, shared) | 139 MB |
| GTK 4 (system library, shared) | 54 MB |
| Chromium | 431 MB |
| Google Chrome | 451 MB |
| Brave | 481 MB |

rippl is small because the engine is the system's WebKitGTK, shared with GNOME Web and anything
else that uses it, the way Safari uses the WebKit in macOS. Counting WebKitGTK and GTK, rippl takes
about **195 MB**, under half of any Chromium browser.

## Speed

| | Time |
|---|---|
| Launch → window ready (control socket answering) | **~0.35 s** (332–392 ms over 3 runs) |
| Launch → example.com fully loaded | **~0.5 s** (498–558 ms) |
| Cached Shield filters in place | 5 ms |
| A tab switch | instant: the page is already there, only the visible view changes |
| Waking a sleeping tab | a page reload, with history and scroll restored |

## Memory

### One light page

| | Total |
|---|---|
| rippl, example.com | ~345 MB: UI 120, network process 50, page process ~170 |

The UI process is mostly GPU driver code. On this hybrid laptop GTK's Vulkan renderer maps both
the NVIDIA and the AMD driver, plus LLVM.

### Ten real sites

Wikipedia, Hacker News, GitHub, MDN, BBC News, Stack Overflow, Reddit, CNN, YouTube, example.com,
all opened at once, measured 45 s later:

| | Total | Processes |
|---|---|---|
| **rippl, all 10 awake** | **~2,300 MB** (2,298 / 2,311) | 14 page processes |
| Chromium | 1,489 MB | 49 |
| Brave | 1,245 MB | 29 (its ad blocker is on, like rippl's Shield) |
| **rippl, 8 asleep** (CNN kept awake: it was playing sound) | **~920–980 MB** | 5 |

**With every tab awake, rippl uses about 1.6–1.9× what Chromium and Brave use.** A WebKitGTK page
process costs about 160 MB even for an empty page; Chromium forks its renderers from a "zygote"
that already holds the engine, so they share much more.

**rippl wins when tabs sleep**, which is how a browser is actually used: tabs you aren't looking at
go to sleep after 30 minutes, and tabs restored from last session cost nothing until clicked. With
the background tabs asleep, the same 10 sites take ~950 MB, less than either Chromium browser.
(Chrome's own Memory Saver discards tabs too, after a longer wait.)

### Where a page process's memory goes, and the GPU switch

An example.com page process, by mapping:

| | GPU drawing on (default) | `RIPPL_NO_GPU=1` |
|---|---|---|
| Total | 167 MB | **95 MB** |
| libwebkitgtk | 38 MB | 35 MB |
| libLLVM (GPU shader compiler) | 20 MB | — |
| NVIDIA GPU compiler | 15 MB | — |
| JavaScriptCore | 12 MB | 12 MB |

Turning GPU drawing off saves 43% on light pages, but on the 10 heavy sites only ~7% (2,140 vs
2,300 MB), and **scrolling costs about twice the CPU** (5 s of scrolling Wikipedia: page process
1.5 → 2.4 s CPU, rippl 1.8 → 3.8 s). It stays off by default; `RIPPL_NO_GPU=1` turns it on for
machines short on memory but not on CPU.

### Shield

| Same page, fresh cache | Shield on | Shield off |
|---|---|---|
| cnn.com | 726 MB | 908 MB (−20% with Shield) |
| speedtest.net | 561 MB | 1,068 MB (−47% with Shield) |

## How to measure

```sh
scripts/mem.sh                 # rippl
scripts/mem.sh --pid <pid>     # a browser you started yourself, its whole process tree
scripts/ctl.sh list            # last column: why a tab can't sleep (PlayingAudio, Capturing, …)
```

To compare with another browser **without touching your own**, start it with a throwaway profile
(`chromium --user-data-dir=/tmp/x --no-first-run <urls…> &`), measure it with
`scripts/mem.sh --pid $!`, and stop only that process tree. Never use `pkill chromium`, which
would also close your own windows.

### Same-site tabs share a page process (Phase 8)

| 5 Wikipedia tabs | Total | Page processes |
|---|---|---|
| A process each (`RIPPL_NO_SHARE=1`) | 897 MB | 5 |
| **Shared** (default) | **630 MB (−30%)** | 1 |

The 10-site test above uses 10 *different* sites, so sharing doesn't change it; it helps whenever
you have several tabs of one site (docs, mail, a wiki, a shop).

## What would make it lighter (ideas, measured before adopting)

- **Sleep sooner when memory is tight.** This already happens through PSI; a "save memory" setting
  could make 10 minutes the default wait.
- **WPE WebKit** instead of WebKitGTK, for a lighter UI process (see the roadmap).
