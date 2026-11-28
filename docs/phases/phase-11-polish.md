# Phase 11: polish (site groups, the best video, instant Shield, splits kept, a capped cache)

**Status:** done (2026-11-27).

## Goal

Finish the Polish items on the roadmap, and add two things asked for along the way: one site's tabs as
one row, and videos at their best quality and frame rate.

## What was built

- **Site groups** (`app/src/site_groups.rs`, rules in `rippl_core::tabs::group_site`).
  `Ctrl+Alt+G`, or **Group tabs from <site>** in a tab's right-click menu, gathers every unpinned
  tab of that site in the space into one row. The row is a hairline box as tall as a tab, with
  one small tile per tab: its icon, or its letter. The front tab's tile is wider and filled. On a
  tile, a click brings that tab to front, a middle-click closes it and a right-click opens its
  usual menu. The same shortcut on a grouped tab, **Ungroup <site>** in its menu, or a right-click
  on the row's free space lets them go. A site group ends by itself when one tab is left. It is
  kept in the session (`site <group> <host>`).
- **The best video** (`rippl_core::ytdlp::BEST`). Formats are ranked by resolution, then frame
  rate, then HDR, then yt-dlp's own order: the better codec, then bitrate. The container no longer
  takes part. Before, `res,ext:mp4:m4a` put `.mp4` ahead of frame rate, so at one resolution a
  30 fps H.264 file could beat a 60 fps VP9 or AV1 one. ffmpeg still merges into `.mp4`. Without
  ffmpeg, the best single file is ranked the same way.
- **Shield off for a site, at once** (`Engine::open_content`, `Browser::rehome`). Sites Shield is
  off for are no longer compiled into the filters as exceptions. Their pages get views from a
  second content manager that has the same scripts and style sheets but no filters. Switching a
  site gives each of its awake tabs a new view, keeping its history (the sleep/wake move).
  Sleeping tabs wake into the right one. A page going into or out of such a site in a view with
  the wrong filters is turned away at its response, before the page process gets any of it, and
  loads again in a view with the right ones.
- **Splits kept in the session** (`split <i> <i> …` lines). Restored splits come back side by side
  when one of their tabs comes to front.
- **A cap on the HTTP cache** (`cache`, default `512m`; `app/src/cache.rs`, rules in
  `rippl_core::cache`). A minute after launch, then every six hours, each kept session's disk
  cache is measured per site. When the whole is over the cap, the sites taking the most room are
  emptied until it is under three quarters of it.
- **Control socket**: `group-site <n>`, and `groups` now shows `site=`.

## Decisions

- **A site group is a group with a site.** `Group.site` reuses everything groups already had: the
  session, moving, new tabs from a grouped tab joining it, and spaces. Named groups keep their
  heading and fold; only site groups draw as a row of tiles.
- **Host, `www.` aside, is the site.** `m.youtube.com` and `youtube.com` stay apart. This is the
  same rule as Shield's sites and same-site process sharing.
- **`Ctrl+Alt+G`**, because `Ctrl+Shift+G` is Find previous. `shortcut.group-site` moves it.
- **Two content managers rather than recompiling.** A content filter applies to every view of its
  manager, and an exception (`@@`, `ignore-previous-rules`) only cancels rules in its own list.
  So the only quick way to unblock one site is a view without the filters. The cost is that
  every script and style sheet rippl adds goes into both managers (`Engine::contents`).
- **Switch at the response, not at the start.** The navigation's own decision can't tell the
  main frame from a subframe, and at `load-changed: Started` the address can still be the old
  page's (it was, for a script's `location.href`). `ResponsePolicyDecision` says
  `is_main_frame_main_resource`, and comes before any of the page is handed over.
- **The converter version went to 4**, so filters compiled with the old exceptions are rebuilt
  once, in the background. The old filter keeps blocking meanwhile.
- **The cache: biggest sites first.** WebKit's own limit grows with free disk space, up to
  gigabytes. Emptying the whole cache would make every site slow once; emptying the biggest keeps
  many small sites cached. `WebsiteDataManager` reports the size of each site's disk cache, so
  rippl never touches the cache's files itself.

## Bugs found

- **The Shield switch missed script navigations.** The first version checked at `Started`, where
  a `location.href` navigation still reports the old address, so the view was never switched. The
  end-to-end test (navigate a filtered tab into a site Shield is off for, check that
  `window.ga` loaded) caught it, and the check moved to the response decision.

- **A click on a tile didn't switch tabs** (reported after the first push). The tiles were
  `GtkButton`s in a `GtkFlowBox`, and both handle a click themselves, most likely before rippl's
  own click handler (the one every tab row uses) could bring the tab to front. They are now plain boxes in plain
  rows, with the same handler as a tab row. The control socket's `select` goes around the
  pointer, which is why the tests passed.

- **A new tab left the column without its field.** The field moves into the middle of a new tab
 , and the column's top stood empty. A stand-in of the same look now holds
  its place there while the field is away; a click or a key in it goes to the field.

- **A page loading in another tab drew its loading line on a new tab's field.** The line was only
  ever set by the front tab's page, never cleared when another tab came to front.
  `Browser::show_progress` now sets it from the front tab whenever the front changes, and never on
  a new tab. The line itself is now a soft blue glow that fades out at both ends, and it glides:
  `ease_progress` moves it toward WebKit's latest value on the frame clock (the screen's rate),
  frame-time based, so it doesn't jump in WebKit's few steps; a finished load sweeps to the end
  before the line goes. `motion = reduced` sets it directly.

- **A setting changed said "Saved." every time.** The change shows for itself; a note is left
  only for what it can't show (a restart needed, open pages unchanged).

- **The column widened while a download ran.** A download's status line ("12.3 MB of 412 MB ·
  2.1 MB/s · 3 min left", or yt-dlp's errors) had no width limit, so the column grew to fit it
  and changed width as the numbers did. It is ellipsized now, as the name was.

- **A click on a space at the column's foot didn't switch to it** (a pin's left click had the
  same fault). A click gesture on a `GtkButton` never sees the left click released: the button
  claims it first. Spaces now switch on the button's own `clicked`; pins (`Browser::on_click`)
  take theirs from `clicked` too, with Alt noted on the way down. Right and middle clicks stay
  with the gesture. The control socket's `space` goes around the pointer, which is why tests
  passed.

## A downloads page

`rippl://downloads` (`Ctrl+J`, or a click on *Downloads* at the column's foot) is a page of its
own, in Settings' look, without the History/Bookmarks bar: what is coming in now, with its
progress (the page reloads every 1.5 s while something is), then every download kept, newest
first, each with a tile for its kind. A finished one: *Open*, *Show in folder*, ×. A failed one:
*Try again* (a video goes back to yt-dlp, in the format it had), ×. *Clear list* empties it; the
files stay. The list is `downloads.log` in rippl's data folder (`rippl_core::downloads::Entry`,
the newest 500), written when a download ends; never from a private tab, and not while
`history = off`.

## Motion, retuned (after the first push)

Checked against Emil Kowalski's animation rules (frequency, easing, physicality):

- **What keys open many times a day moves least.** A new tab's field (Ctrl+T) no longer rises
  for 380 ms with a bounce; it is just there, and the offers under it fade in 120 ms. Ctrl+Tab's
  layer fades in 120 ms instead of popping for 280 ms, and its cards change at once while Tab is
  held. Ctrl+F's bar drops in 180 ms, the video bar's field in 180 ms, both without overshoot.
- **Exits are never `ease-in`.** A notice leaving, a closed tab's row and a group's tile used a
  curve that starts slow, at the moment you watch; they now take 150 ms with a strong ease-out.
- **One curve, as a CSS variable:** `--ease-out` `cubic-bezier(0.23, 1, 0.32, 1)` for everything
  that enters or leaves. It settles without overshoot, like Apple's critically damped spring
  (damping 1): nothing in rippl's chrome is flung, so nothing bounces; the page's elastic edges
  keep theirs. Popovers grow from their top, where their trigger is (`transform-origin`).
- **Presses answer at once:** 97% in 100 ms on press, back in 160 ms on release, without
  overshooting; group tiles give too.
- **Tracking by size** (Apple's typography): small bold labels (group headings, the downloads
  heading, letter marks, badges) +0.02em; the switcher's 28 px letter −0.02em; the new tab's field
  −0.01em.
- **One dim for every layer that takes the focus** (the video bar, Ctrl+Tab, peek): a soft grey
  wash, lighter around the middle, in place of two black tints and a near-invisible one.
- **Leaving the way they came** (Apple's spatial consistency): peek shrinks back as its dim
  lifts, and the AI panel slides out to the right it came from (`Browser::leave`, 150 ms).
  Opened again while leaving, they stay; a new peek replaces a leaving one at once.
- **Depth by size:** three shadows (`--lift-1` field and offers, `--lift-2` popovers, `--lift-3`
  peek and the switcher), each a contact shadow plus an ambient one, deeper in dark. In dark,
  floating surfaces also catch a faint light on their top edge (`--edge`), and the bars over
  the page (find, a question, notices) carry a hairline so they hold on a dark page.
- **Hit targets:** the player's buttons 20 px (were 15), a tab's × 20 px.
- **The desktop's "reduce animation"** (`gtk-enable-animations` off, read once at start) now
  makes rippl's motion reduced as well, elastic edges included.
- **A theme change eases** (240 ms) instead of flashing from light to dark.
- **Internal pages** (Settings, History, the welcome): the same ease-out without overshoot, a
  faster press, Settings' switches glide (220 ms) with a softer knob shadow, and large titles
  tracked tighter (−0.015em to −0.022em, reading mode's title too).

## Measurements

This machine, debug build, local test server, scratch profile:

| What | Result |
|---|---|
| Shield off for a site, until its tab loads the blocked script | **391 ms** (was about 6 s: both bundles recompiled) |
| A filtered tab navigating into a site Shield is off for | 313 ms to the script loading, history kept (6 entries) |
| Compiling the bundles once without exceptions (converter 4) | ads 3.9 s, privacy 3.1 s, in the background, old filter blocking |
| `Ctrl+Alt+G` with three tabs of a site among five | `Grouped(_, 3)`: together where the first was; the others in place |
| Closing grouped tabs down to one | the group ends, the last tab stays loose |
| A split, quit, launch | `split 1 4 front 4`: side by side again |
| Cache cap 32 MB, two sites of 19 MB each | 38 MB measured; `localhost` emptied, 19 MB left |
| yt-dlp, a 4K60 video | both rankings take 2160p60 AV1; the new one takes Opus audio over AAC |

## Known limits

- With tabs across the top (`tabs = top`), a site group shows as its separate tabs.
- A form sent (POST) into or out of a site Shield is off for is loaded again as a plain visit.
- A pop-up keeps its opener's filters, even if it shows another site (WebKit requires a related view).
- The cache is checked every six hours, so between checks it can grow past the cap.
