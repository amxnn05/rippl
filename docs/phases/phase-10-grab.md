# Phase 10: downloading videos with yt-dlp (Ctrl+Shift+Y)

**Status:** done (2026-11-27).

## Goal

Save a video from YouTube, Vimeo or any of the sites yt-dlp knows, without leaving rippl or
opening a terminal: a shortcut, paste the link, Return.

## What was built

- **The bar** (`app/src/grab.rs`). `Ctrl+Shift+Y` dims the page lightly and shows, in its middle,
  the same field a new tab shows (Ctrl+T): 560 wide at most, 50 tall, rounded 14, lifted 30 above
  centre, placed by the new tab's own `HomeLayout` and drawn with its `home-field` style, so the two
  can't drift apart. A card under it says what Return and Shift+Return do, or what went wrong.
  It opens holding the link on the clipboard if there is one, else the front page's address,
  selected, so on a video page it is `Ctrl+Shift+Y`, Return. `Esc`, a click on the dim, or the
  shortcut again closes it.
- **Return** downloads `grab_format` (default: the best video with its sound, as `.mp4`);
  **Shift+Return** the other (only the sound, as `.mp3`).
- **The download** is listed at the foot of the column with the others (`app/src/downloads.rs`):
  the video's title once yt-dlp knows it, a progress bar, "12.0 MB of 40.0 MB · 1.5 MB/s · 20 s
  left", "Finishing…" while ffmpeg merges or converts, then "Done. Click to open". Pause stops
  yt-dlp and keeps its `.part` files; resume runs it again and it carries on from them. × cancels
  and removes what it left (`.part` files, the piece finished before a merge, yt-dlp's `.ytdl`
  notes); on a finished one × only takes it off the list.
- **Rules** (`crates/rippl-core/src/ytdlp.rs`, unit-tested): which pasted text is a link, the
  arguments rippl runs yt-dlp with, reading its lines, and shortening its errors.
- **Settings**: `grab = on|off` and `grab_format = video|audio` (Settings › Browsing › Video), and
  `shortcut.grab` to move the key.
- **Control socket**: `grab <url> [video|audio]`, `grab-bar`, `download-remove <n>`; and two that
  help test any UI: `window-shot <file.png>` (the window as drawn, without touching the desktop)
  and `quit`.

## Decisions

- **yt-dlp as a separate program, not a library.** It is Python, updated every few weeks as sites
  change, and packaged by every distribution (`pacman -S yt-dlp`). rippl runs whatever is
  installed; the system's package manager keeps it current, as for rippl itself. If it isn't
  installed, the bar says so and how to install it.
- **No shell, ever.** The arguments are a list (`gio::SubprocessLauncher`), and the link is last,
  after `--`, so pasted text can't become an option (`--exec …`). Only `http`/`https` links with a
  host are accepted (`file:`, `javascript:`, `-o x`, lone words are refused); a link without its
  scheme (`youtu.be/abc`) gets `https://`.
- **Only rippl's options.** `--ignore-config` (a `~/.config/yt-dlp/config` can't add `--exec` or
  `--cookies-from-browser` behind rippl's back), `--no-update`, `--no-playlist` (a video in a
  playlist downloads that video, not hundreds).
- **No cookies.** rippl gives yt-dlp none of its own. Videos that need you signed in (members-only,
  some age checks) fail with yt-dlp's reason. Handing sign-ins to another program is a choice for
  later, as a setting, off by default.
- **The proxy applies.** `proxy = direct` passes `--proxy ""`; a proxy URL passes it on; `system`
  leaves yt-dlp to the desktop's settings, as rippl's own requests do.
- **It ends with rippl.** The child sets `PR_SET_PDEATHSIG`: if rippl quits or crashes, yt-dlp
  stops, so nothing goes on downloading unseen.
- **Files**: `Title [id].mp4` in the Downloads folder: the title cut to 150 bytes, and the video's
  id so two videos with one title don't collide. A video already there is not downloaded again.
- **Without ffmpeg**, nothing can be merged or converted: video takes the best single file, audio
  the best audio as it comes (often `.m4a`).
- **Progress** comes from yt-dlp's `--progress-template` (bytes, total or estimate, speed, time
  left, the `.part` file) and `--print` at its stages (title before downloading, "finishing"
  before post-processing, the final path after moving), parsed line by line, never from its
  human-readable output.

## Bugs found

- **A `RefCell` borrow held across a call** (found by the end-to-end test: rippl aborted on
  `download-remove`). `if n >= webkit && let Some(id) = self.grabs.borrow()…` keeps the borrow
  alive for the whole `if` body, and the body calls `grab_close`, which borrows again. The id is
  now taken in a statement of its own first. The real × button wasn't affected; the rule in
  `CLAUDE.md` is why it was looked for.
- **A test that proved nothing:** the first "quit while downloading" run found no yt-dlp child,
  because the file was already in the folder from an earlier run and yt-dlp had finished at once.
  The test now removes the file first.

## Measurements

Local server (Python, throttled to ~1.5 MB/s for `big.mp4`), this machine, yt-dlp 2026.10.26:

| What | Result |
|---|---|
| Return to first progress line (6 s 320×240 clip, generic extractor), 3 runs | 1,071 / 1,095 / 1,114 ms: almost all of it is yt-dlp starting Python and loading its extractors |
| Whole download of that clip | 1,120 / 1,160 / 1,172 ms |
| yt-dlp's peak memory, downloading a 15 MB file | 68 MB, in its own process, only while it runs |
| Pause at 14% of 15,403,106 bytes, resume | carried on from 14% (2,096,128 bytes kept), not from 0; the finished file is byte-identical to the source (`cmp`) |
| Cancel while running, and while paused | `.part` removed, yt-dlp gone, row gone |
| Quit rippl mid-download (`quit`) | the yt-dlp child (its parent was rippl) ended with it; its `.part` stays |
| Video from a page with a `<video>` (`page.html`) | `.mp4` with video and audio streams, 6.000 s (`ffprobe`) |
| Audio of the clip | `.mp3`, 6.000 s |
| A 404 | the row says "Unable to download webpage: HTTP Error 404: File not found …" |
| rippl at startup and with no download running | unchanged: nothing runs until Return |

## Known limits

- yt-dlp takes about a second to start, so the row says "Starting…" for that long.
- A `.part` left by quitting mid-download stays in Downloads (as for paused WebKit downloads); run
  the same link again and yt-dlp carries on from it, or delete it.
- Downloads are not remembered across restarts, as with every other download.
- No quality picker: best video or audio only. No playlists, no subtitles, no signed-in videos.
- Sites change and yt-dlp follows; an old yt-dlp fails on some sites until the system updates it.
