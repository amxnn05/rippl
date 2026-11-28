# Phase 2: Memory

**Status:** done (2026-11-25)

## Goal

Keep as few web processes alive as possible, since they are where almost all of a browser's memory
goes.

## What was built

1. **Lazy restore.** Tabs from the last session are `Lazy`: an address and a title, with no view and
   no process. Only the front tab is loaded at launch.
2. **Tab sleep** (`rippl_core::tabs::due_for_sleep`, `Browser::sleep/wake`). A tab unseen for 30
   minutes (`sleep` in the config) saves its session state, loses its view, and **its web process is
   ended**. Selecting it makes a new view, restores the history and scroll position, and reloads
   the current page. It never sleeps if it is the front tab, pinned, playing audio, using the
   camera/microphone/screen, or still loading. The oldest-unseen tab sleeps first.
3. **Memory pressure** (`pressure.rs`). Every 5–30 s rippl reads `/proc/pressure/memory` (Linux PSI).
   Under pressure (`some avg10 ≥ 10%`) the sleep wait drops to 5 minutes; when critical
   (`full avg10 ≥ 5%` or `some ≥ 40%`) idle tabs sleep immediately and the heap is trimmed. PSI
   measures *time stalled waiting for memory*, which is a better signal than "free RAM" because
   Linux keeps free RAM near zero on purpose.
4. **A memory limit for each web process** (`MemoryPressureSettings`, Linux-only WebKit API). The
   limit is one eighth of RAM, between 512 MB and 2 GB, with conservative and strict thresholds at
   33% and 50% of it. WebKit's own default is most of the machine. Pages are never killed for
   reaching it; they drop caches sooner. The network process gets the same limits.
5. **Heap trim**: `malloc_trim(0)` two seconds after the window loses focus, after Shield compiles
   (Phase 3), and under critical pressure. Freed
   memory stays counted against the process until it is handed back.
6. **Control socket** (`control.rs`), so all of this can be
   tested without sending keys. See [testing.md](../testing.md).

## Measurements (this machine, PSS, all processes)

| State | Total |
|---|---|
| example.com in front, Wikipedia tab lazy | ~335 MB (1 web process) |
| Both awake | 457 MB (2 web processes) |
| Background tab asleep (process ended) | 384 MB |
| `ctl sleep` (only the front tab awake) | 448 → 327 MB |
| Sleeping state kept for example.com | 133 bytes |

GTK renderers, rippl process alone: Vulkan (default) ~118–160 MB, cairo ~130 MB, GL ~196 MB. The
default stays. Most of the UI process is GPU driver code: on a hybrid laptop, the Vulkan loader maps
both the NVIDIA and the AMD driver, plus LLVM.

## Bugs found while testing, and fixed

- **Sleeping a tab deleted it.** The first version called `try_close()` on the view being put to
  sleep. WebKit answered with the page's `close` signal, and the handler took that as the page
  closing itself and removed the tab. The same thing made closing a tab run twice. Fix: every
  per-view signal handler checks `Browser::ours(id, view)` first; a view rippl is already taking
  away can't change its tab any more.
- **Sleeping freed almost nothing.** Once the view was dropped, the web process stayed alive: WebKit
  keeps a finished page's process cached for reuse, and `CacheModel::DocumentBrowser` didn't change
  that. Fix: `terminate_web_process()` when the last awake tab of a *family* sleeps. Tabs opened by
  a page share their opener's process, so ending it while the opener is awake would crash the opener.
- **Signals from a dying view overwrote the sleeping tab** (title, URL, loading state). Fixed by
  the same `ours()` check.
- **Keyboard tests were unreliable**: keys sent with `wtype` went to whichever window had focus.
  Fixed by building the control socket instead.

## Not done

- A picture of the page while a sleeping tab wakes (some browsers show one). It costs memory while
  asleep, so it would have to be small.
- Hibernating tabs to disk across restarts: sleeping state is in memory only, so after a restart a
  tab reloads from its URL rather than from its saved history.
