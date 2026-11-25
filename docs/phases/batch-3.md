# Feature batch 3 (first part): uBlock's lists, glass, dark pages, your CSS, capture, icons

**Status:** in progress (2026-11-25). Done here: uBlock Origin's lists in Shield, the glass window,
dark pages, your style for a site, capture, site icons. Still to come in batch 3: peek, YouTube ads,
unread counts, video keys, the player in the column. Plan: [features.md](../features.md).

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

## Tests

75 (71 core, 4 app), clippy clean. Also checked by keyboard: Ctrl+T, type, Enter loads in the new
tab; Ctrl+L on a page uses the column's field.
