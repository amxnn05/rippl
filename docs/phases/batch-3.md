# Feature batch 3: uBlock's lists, glass, dark pages, your CSS, capture, icons, the new tab, YouTube ads, peek, player

**Status:** done (2026-11-26). Plan: [features.md](../features.md).

- **First part:** uBlock Origin's lists in Shield, the glass window, dark pages, your style for a
  site, capture, site icons, and the new tab.
- **Second part:** YouTube's ads, peek, unread counts, the player, video keys, saving a page as a
  web archive. Also: glass off by default, and the pins' cut-off fixed.

## uBlock Origin in Shield

**What was asked:** integrate `gorhill/uBlock` or `imputnet/uBlock` (Helium's fork).

**What can and can't be integrated:**

- **Neither extension can run in rippl.** uBlock Origin is a browser extension, and WebKitGTK has
  no extension API.
- **What carries over is its filter lists**, from its `uAssets` repository: *uBlock filters,
  Privacy, Badware risks, Quick fixes, Unbreak*. rippl now fetches them beside EasyList and
  EasyPrivacy and compiles them into WebKit's own blocker. No script runs in pages, which is the
  lightest way, and the way uBlock Origin Lite works too.
- **The upstream lists are the right source**: imputnet's repository is a fork of the extension's
  code, not of the lists.
- **What makes uBlock stronger and can't run in WebKit:** its *scriptlets* (`##+js(…)`, which
  defeat YouTube ads and anti-adblock walls) and *redirects*. About 16,000 such rules are skipped.
  YouTube ads need rippl's own small scriptlet support (later in batch 3).

**How it's built:**

- **Includes:** uBlock's lists are mostly `!#include` lines. They're fetched and spliced in, only
  from files beside the list (`rippl_core::shield::includes`, tested).
- **`!#if`:** uBlock's preprocessor is understood (`preprocess`, tested). rippl is none of the
  environments it names, so only `cap_user_stylesheet` is true.
- **New options:** `from=` (same as `domain=`), `$all`, `$strict1p`/`$strict3p`. `##^` (HTML
  filtering) and more procedural CSS are skipped.
- **Two bundles, each compiled as one WebKit filter:**
  - *ads* = EasyList + uBlock filters + Quick fixes + Unbreak
  - *privacy* = EasyPrivacy + uBlock Privacy + Badware + Unbreak

  Bundles are needed because WebKit only lets an exception (`@@…`) cancel rules **of its own
  filter**, and Unbreak is mostly exceptions to the other lists.
- **The converter is version 2**, so filters from version 1 are rebuilt.
- **`ublock = off`** in the config, or on the settings page, keeps EasyList and EasyPrivacy alone.

### Measured

| | Rules | Compile (background) |
|---|---|---|
| ads, before (EasyList) | 59,113 | 3.2 s |
| ads, now | **66,649** | 4.7 s |
| privacy, before (EasyPrivacy) | 56,726 | 2.6 s |
| privacy, now | **63,526** | 3.9 s |

A fixed test page asked for 60 well-known ad and tracker addresses (Google ads and analytics,
Criteo, Taboola, Outbrain, Hotjar, Mixpanel, Segment, the Facebook and TikTok pixels, Clarity,
Yandex Metrica, and others):

| | Blocked |
|---|---|
| Shield off | 4 / 60 (browser-level only) |
| EasyList + EasyPrivacy | **59 / 60** |
| + uBlock's lists | **59 / 60** |

The one that got through is Optimizely (A/B testing, on no list). So on the big networks
EasyList already does the work; uBlock's lists add about 14,000 site-specific rules for the long
tail, for 1.5 s more background compiling and nothing measurable while browsing. They stay on.

### A Shield bug the test found (fixed)

When a bundle had to be rebuilt (a list update, or `ublock` switched), Shield didn't load the old
compiled filter and fell back to the 26-host seed list until the rebuild finished: 7–9 s in which
most trackers got through (26 of 60 blocked, measured). It now loads whatever was compiled last,
even if out of date, and swaps the new one in when ready. Measured during a forced rebuild:
**59 of 60 blocked**. An earlier CNN comparison of "with and without uBlock" was invalid for the
same reason and was dropped.

## Glass: why blur didn't work, and the fix

**Asked:** why doesn't Hyprland blur rippl like it blurs other apps?

**Found:**

1. Hyprland only blurs behind parts of a window that are see-through. rippl painted a solid
   background, so GTK told the compositor the window was opaque.
2. Even with a see-through background, nothing was blurred: a close-up showed sharp wallpaper
   detail. A Wayland protocol trace (`WAYLAND_DEBUG=1`) showed why. GTK 4.24 opens the newer
   **`ext-background-effect-v1`** protocol for every window, through which an app tells the
   compositor *where* to blur, but sends no region. An unset region means "blur nothing", and
   Hyprland respects it. Apps on GTK 4.24 therefore don't get Hyprland's automatic blur.

**Fix:** with `glass = on` (the default):

- the column's ground is see-through (72% light / 70% dark);
- CSS `backdrop-filter: blur()` on the window makes GTK send `set_blur_region` for it.

The page card stays solid. The trace now shows `ext_background_effect_surface_v1.set_blur_region`.
A close-up shows smooth frosted glass, and fine detail in the column measured 1.67 → 0.85
(halved). `glass = off` gives the solid look.

## Dark pages

With `dusk = on` and rippl's dark look, a page **without** a dark look of its own is darkened:

- **The check:** when a page finishes loading, a check in rippl's isolated world reads the page's
  `color-scheme` and its background. A light page with no dark scheme gets
  `data-rippl-dusk`.
- **The darkening:** one style sheet inverts marked pages (`invert(0.9) hue-rotate(180deg)`) and
  turns pictures, video, canvases and frames back to their own colours.
- **Per site:** off from the site card ("Darken this site").

Tested:
- a plain light page: marked, filter applied;
- GitHub (it has its own dark look): left alone (`has-dark`).

## Your style for a site

- Written in the site card ("Your style for this site…"), stored per site (`places.sqlite`,
  `site_css`), and applied before the page draws.
- One user style sheet per site now carries both its hidden elements and its own CSS
  (`apply_site_styles`, pagelook.rs).

Tested: `p { color: rgb(1,2,3); letter-spacing: 3px }` applied after reload, together with a hidden
element on the same site.

## Capture

The whole page (WebKit's full-document snapshot) is saved as PNG in `~/Pictures/rippl/`, named after
the site and the time (never overwriting), and put on the clipboard. Tested: file written.

## Site icons

- WebKit's favicon store is on for every session.
- A tab shows its site's icon (15 px) once the page gives one. A sleeping or not-yet-opened tab
  asks the favicon store, once per address.
- Letters remain for sites without an icon, and with `icons = off`.

Tested: GitHub's 32×32 icon; a column with GitHub, Wikipedia, YouTube and Reddit icons.

## The new tab (home.rs)

- **The field stands on its own in the middle of the page card** `Omnibox` on a blank
  tab:
  - size: 560 wide (less in a narrow window, keeping 14 each side) and 50 tall;
  - look: rounded 14, a hairline, a soft shadow, 15.5 px text, "Enter a web address";
  - place: lifted 30 above centre.
- **It is the column's own field, moved.** Offers, keywords and Tab-to-search work the same.
  - On a page it goes back to the top of the column.
  - On the new tab the column keeps showing its tabs.
- **What it offers hangs under it** in its own rounded card (padding 6, rows rounded 9), so the field
  never moves. An empty field offers nothing.
- **Home grid** (`home_grid = on`, off by default): the sites of the bookmark folder
  *Home*. Four to a row, sixteen at most, tiles 50 tall with a hairline. Hidden while you type.
- **No bookmark star** in the field when there's no page.
- **Not copied:** the slow "breath" under the field, since an animation that never stops keeps
  the frame clock running on an idle window. Painted walls (Themes) come in batch 4.
- **Placement:** a small layout manager (`HomeLayout`), because GTK has no "at most this wide" and a
  fixed 560 would make the window at least that wide.

## Fix: the page card's corners

The page card was rounded (10 px), but a page painted square corners over it: GTK only cuts
children to a widget's rounded edge when the widget's overflow is hidden. The stage now sets it.
Checked on a white page (Wikipedia, light theme): all four corners are round.

## Glass off by default

You asked for no see-through. `glass` now defaults to **off**: the column is the solid frame
(`#131313` dark, `#f4f4f4` light) and the page card its ground (`#1c1c1c` / white). rippl's
palette already matched the `Palette` exactly.

What still shows the desktop through is Hyprland's rule (Omarchy gives every window a little
transparency), not rippl. A window rule for `dev.rippl.Rippl` with opacity 1 makes it fully solid.

## YouTube's ads (`pagescripts.rs`, `rippl_core::shield::youtube_rules`)

. YouTube announces its ads in the player's own data and
plays them from the video's servers, so no filter list stops them.

- **In the page's world, on YouTube's hosts only, before any page script:**
  - it takes `adPlacements`, `adSlots`, `playerAds` and `adBreakHeartbeatParams` out of the
    player's data (`ytInitialPlayerResponse`, `JSON.parse`, `Response.json`);
  - it answers YouTube's ad reports and ad-id request with an empty 200 before they leave, so
    nothing is sent and the blocker test doesn't fail;
  - it keeps a video playable when YouTube writes its anti-blocker wall over an OK answer;
  - it starts a first video held at 0:00 again, once.
- **In rippl's world:** an ad that plays all the same is muted and run at 16×, and sound and speed
  come back after it.
- **Rules, compiled into every Shield filter (converter version 3):**
  - YouTube's ad paths, ad quality reports, and Google's ad paths and video-ads SDK on YouTube pages;
  - the exception for `ad_status.js`, which must sit in the same filter as the EasyList rule it
    cancels.
- **When it's off:** with Shield off, or on a YouTube host Shield is off for. The scripts are
  swapped at once; pages loaded afterwards get the change.

Tested on `youtube.com/watch`:

| | Ads in the player's data | Result |
|---|---|---|
| Shield off | yes | ads |
| Shield on | **none** | played 8 s in 8 s, `ad-showing` never set |

A video waits at 0:00 until clicked either way: that is WebKit's autoplay policy, not the script.

## Peek (`peek.rs`)

the (and Arc's) peek.

- **Opening:** Shift+click a link and its page opens in a panel over the one you're reading.
  - size: 82% × 86% of the page, rounded 12, a hairline and a deep shadow;
  - the page under it is dimmed 22%;
  - buttons beside it: × and "Open as a tab".
- **Putting it away:** Esc, a click on the dimming, or ×. Its page process ends.
- **Keeping it:** Ctrl+Return or the button. It becomes a tab after the one it came from, in that
  tab's group, already loaded (nothing loads twice).
- **Session:** the same space and kind of session as the tab it came from (same sign-ins; private
  from a private tab).
- `peek = off` turns it off.

Tested through the control socket (`peek <url>`):
- the panel draws over Wikipedia, and the page beside it is dimmed (white measured 185);
- Esc closes it and leaves one tab;
- Ctrl+Return keeps it as a second tab, in front, with its page.

**Not tested: a real Shift+click.** A click a script fakes carries no button or modifier for
WebKit (by design), and this machine has no way to send a real one without root. The check
follows WebKit's documented API: button 1 with Shift held, as a link click or a new-window click.

## Unread counts (`rippl_core::tabs::unread`)

As the :
- **The count:** read off the front of a page's title, as WhatsApp, Messenger, X and Discord put it
  there: "(3) WhatsApp" gives 3, "(12+)" gives 12+, over 99 gives 99+. A number further along
  ("Dune (2021)") is not a count.
- **The badge:** a red badge on the pin, hung from its letter's corner. A pin shows no title, so
  this is the only place the count shows. None while the pin sleeps.

Tested: "(5) Chat" pinned shows a 5. Six cases are in the core tests.

**Fixed with it:** pins were cut in half. The FlowBox that held them got no height inside the
scrolling column (measured: 0 px), so only the bottom of each pin showed, and that was older than
this round. Pins are now a grid of six per row, each filling its share do; measured
30 px tall.

## The player (`player.rs`)

As the , off unless `player = on`, as there:
- **What it shows:** the newest tab making sound out of sight (not on screen, not its split
  partner, this space only), as a row with a hairline above the column's foot: its icon or letter,
  its title, and a pause.
- **Pausing:** two lines of script in rippl's world pause what plays and remember it. The row then
  offers play, and × to put it away.
- **Going there:** a click on the rest of the row goes to the tab.

Tested with a tab playing a tone and another in front:
- the row appeared;
- pause paused the page's audio, and the row showed play and ×;
- play resumed it.

## Video keys (`video_keys = on`, off by default)

As the , on a `<video>` or `<audio>` with the browser's own controls, once
clicked:
- Space plays or pauses;
- ← and → go 5 s back or on.

Players a site draws itself handle their own keys. Runs in rippl's world. Tested: Space on a
focused video started it.

## Save a page as a web archive (Ctrl+Alt+A)

The page and everything it shows go in one `.mhtml` file in Downloads (WebKit's MHTML save), which
rippl opens again as it was. Tested: Wikipedia's "Linux" saved as 1.9 MB in 19 parts; opened again
offline, with its title and all 45 images.

## Tests

77 (73 core, 4 app), clippy clean.
