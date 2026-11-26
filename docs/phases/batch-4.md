# Feature batch 4: autoplay, autoscroll, tabs that close themselves, the tab switcher, the bookmarks bar, icons-only and top tabs, pages at the screen's rate

**Status:** first part done (2026-11-26). Plan: [features.md](../features.md).

- **First part (this):** autoplay choice, middle-button autoscroll, tabs that close themselves
  and the archive, the Ctrl+Tab switcher, the bookmarks bar, the column as icons only, tabs
  across the top, and pages at the screen's own refresh rate.
- **Second part (next):** new-tab walls and themes, pin folders, 3–4 page split, space colours and
  swipes, several windows, your own shortcuts.

## Pages at the screen's refresh rate (`frame_rate`)

**Asked for:** scrolling as smooth as the monitor allows: 144 Hz on a 144 Hz screen, 120 on 120.

**What was wrong:** WebKit holds a page's rendering (animations, `requestAnimationFrame`, and the
scrolling it animates) to the divisor of the refresh rate nearest 60. On the 144 Hz eDP-1 here,
pages ran at 72. GTK and Hyprland were not the limit: the window's own frame clock ticked 145
times a second.

**Fix:** WebKit's feature `PreferPageRenderingUpdatesNear60FPS` (stable, on by default
everywhere but visionOS) is turned off with `webkit_settings_set_feature_enabled`
(`Engine::set_full_rate`). Note the API's name for it has no `Enabled` suffix; the first try
used the preference's internal name and silently matched nothing.

| Measured on 144 Hz | rAF | window paints / s while a page scrolls |
|---|---|---|
| before (`frame_rate = 60`) | 73 | 72 |
| after (`frame_rate = display`, default) | 145 | 144 |

The rate is the monitor's, as WebKit reads it from the display the window is on. `frame_rate = 60`
gives the power back on battery. Smooth wheel scrolling was already on
(`enable-smooth-scrolling`); it now animates at the full rate too.

## Autoplay (`autoplay`)

the Autoplay, through WebKit's own website policies, given with each navigation
(`decide-policy` → `use_with_policies`):

- `muted` (default, WebKit's own): videos may start without sound.
- `never`: nothing starts until clicked.
- A site's card has **Play sound by itself** (not in private tabs): that site may start with sound.

Tested on a page with one `autoplay` video and one `autoplay muted`: default → loud paused, muted
playing; site allowed → both playing; `never` → both paused.

## Middle-button autoscroll (`autoscroll = on`)

the AutoScroll script, in rippl's isolated world: a middle click on a page (not on a link,
which still opens a new tab) leaves a round mark; the page scrolls towards the pointer, faster the
further away; a click, Esc or the wheel stops it. Tested: 5,160 px in 1.2 s with the pointer 320
px below the mark; Esc stopped it.

## Tabs that close themselves, and the archive (`close_after`)

the  and . `close_after = 12h | 1d | 7d | 30d` (never by default).

- Counted from when you last left the tab, **across restarts**: the session now keeps each
  tab's time (`left <index> <unix seconds>` lines; older sessions count from the restore).
- **Stays:** the page on screen, pinned, in a group, a site kept awake, private, blank, busy
  (sound, camera, loading) (`rippl_core::tabs::stays_open`). A split pair goes whole or not at
  all.
- **Nothing typed is lost:** an awake page is asked first, and one holding text typed and not
  sent stays.
- Closed tabs go to **rippl://archive** (also in the field as "Archived tabs"): a month, 500 at
  most, searchable, forget one or empty it. Kept only with `history = on`; clearing history
  empties it.

Tested with `RIPPL_CLOSE_AFTER=6`: two pages closed into the archive; the blank tab and the one
on screen stayed; a page with half a message typed stayed.

## Ctrl+Tab switcher (`tab_switcher`, on)

the TabSwitcher: the space's tabs as pictures, the one you're on first, then latest left.
Hold Ctrl and press Tab (or arrows) to walk, let go to go there, Esc to stay, or click a card. A
quick Ctrl+Tab goes to the previous tab without drawing anything (the cards appear after 140 ms).

- Pictures are taken as you leave a tab, cropped to the top of the page and drawn 320 wide on
  the window's renderer (about 300 KB each). They go with the tab.
- The grid shows as many cards to a row as fit, four at most, twelve in all.
- Ctrl let go is seen both as a key release and as a change of modifiers.

Tested with real key presses: Ctrl+Tab+Tab went to the third card, a quick Ctrl+Tab back to the
previous tab. `tab_switcher = off`: Ctrl+Tab is the next tab, as before. Not yet: the keys on
a card (⌃W closes it, ⌃R reloads).

## Bookmarks bar (`bookmarks_bar = on`)

Above the page's card, on the window's ground. Loose bookmarks first, then each top folder as a
menu (nested folders as headings in it). What doesn't fit goes into **»** at the end (a layout
manager places as many as fit, then the »), never a sideways-scrolling row. Click: open in this
tab; middle click: a new tab behind. Changes on rippl://bookmarks and the star update it.

## The column as icons only (`column = icons`)

the SideIcons: the column 52 wide, each tab its icon or letter in a tile, the title in a
tooltip; pins stack one per row; the foot's buttons stack. The field moves to a strip above the
page, and what it offers floats under it over the page. The player is off in this layout.

## Tabs across the top (`tabs = top`)

No column. The strip above the page holds the field (300 wide), the pins as icons, the tabs (icon,
title, × on hover and on the tab you're on), then + and the settings gear. Tabs share the row
from 220 px down to 96, then the row scrolls (the wheel scrolls it sideways), always bringing the
tab you're on into view. Right-click menus hang from the strip. The new tab still puts its field
in the middle of the page.

Not yet: the sliding pill and dragging tabs along the row; downloads and spaces' buttons
are in the column only (spaces stay on Alt+1–9).

## Not tested

- The field leaving the new tab and going back into the top strip, and its offers floating under
  it in the top and icons layouts: built, but not walked through, since that needs keystrokes
  into the window.
- Scrolling with a real wheel at 144 Hz: measured through scripted scrolling and the window's
  paint count, not a physical wheel.

## Tests

79 (75 core, 4 app): new for closing (`tabs_left_alone_close`), the archive
(`archive_keeps_newest_once`), the session's times (round trip), and the new settings.

## For testing

The control socket gained `set <key> <value>`, `switcher [next|go|esc]`, `bar [more]`,
`archived [words]`, `frames` (the window's frame clock) and `paints` (paints in a second).
