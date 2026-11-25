# Feature batch 1: the small daily things

**Status:** done (2026-11-25). The plan is in [features.md](../features.md).

| Feature | rippl | Key |
|---|---|---|
| Site card | ✅ `sitecard.rs` | click the tab you're on, or **Ctrl+I** |
| Keep a site awake | ✅ in the site card | |
| Blocked pop-ups with Open / Always allow | ✅ | |
| Duplicate tab (⌘D) | ✅ with history and scroll | **Ctrl+Shift+D**, tab menu |
| Tab menu (right-click a tab) | ✅ Duplicate, Pin, Put beside this tab, Copy address, Reload, Close, Close other tabs | right-click |
| Print | ✅ | **Ctrl+P**, site card |
| Typos in the field | ✅ | |
| Search a site from the field | ✅ as a row ("Search Reddit") that puts the site's keyword in the field | |
| Your own keywords (Settings › Site shortcuts) | ✅ `keyword.<word> = <address with %s>` in the config | |
| Commands in the field | ✅ rippl's pages offered by name ("sett" → Open Settings), as rows only: Return still does what it did | |
| Search engine suggestions | ✅ **off by default** (`suggestions = on`) | |

## How each works

### The site card

A popover under the field shows:

- the site, and whether the connection is private (green), not private (amber: plain http, or a
  certificate with problems, from WebKit's TLS information), or rippl's own page;
- **Copy address** (clean), **Print…**, and the page's zoom (− 100% +);
- three switches for the site:
  - **Block ads and trackers**: Shield's per-site exception;
  - **Keep this site awake**: its tabs never sleep (a new `KeptAwake::Chosen` reason in the sleep
    policy, tested);
  - **Allow pop-up windows**;
- what the site was allowed or denied (with *Forget*), and how many things are hidden on it (with
  *Show again*);
- **Settings…**.

The two new switches are kept per site in `places.sqlite` (`site_flags`), never for a private tab.

### Blocked pop-ups

WebKit is told pages may ask for windows (`javascript-can-open-windows-automatically`), and rippl
decides in its `create` handler:

- A window asked for **right after a click or a key** (WebKit's `is_user_gesture`) opens as a tab,
  as before.
- **Without one**, it is refused, unless the site's *Allow pop-up windows* is on. A bar over the
  page says *"site tried to open a window (address)"*, with **Open** (a tab of its own), **Always
  allow** and ×.
- The last five refused per tab are kept; nothing opens by itself from the bar.

### The field

While you type, the list offers, in this order:

1. open tabs;
2. history and bookmarks (exact matches);
3. history that matches **with one typo**: a letter wrong, missing, extra or two swapped, for words
   of four letters or more (`suggest::near`, `rank_fuzzy`, tested; a third of an exact match's
   score);
4. **Search <site>** rows when what's typed starts a known site's name;
5. rippl's own pages by name;
6. the engine's suggestions, only with `suggestions = on`.

Engine suggestions are asked 180 ms after the typing stops, from a separate session with no cookies,
never for something that looks like an address, and answers to older keystrokes are dropped.
DuckDuckGo and Brave Search offer them; the other engines don't.

## Tested

| Check | Result |
|---|---|
| A page that opens a window on its own | refused; the bar showed; **Open** made it a tab |
| Keep example.com awake, sleep after 6 s, 18 s idle | example.com stayed awake (reason "Chosen"); its duplicate too |
| Duplicate a tab navigated Linux → WebKit and scrolled to 1,800 px | the copy beside it, `scrollY` 1915, history 2 |
| Site card on github.com | "Connection is private", the buttons and three switches |
| `githbu` | GitHub from history |
| `red` | "Search Reddit", which puts `r ` in the field |
| `sett` | "Open Settings" |
| `rust owners`, suggestions off / on | 0 engine rows / 5 from DuckDuckGo |
| Tests | 68 (64 core, 4 app), clippy clean |

## Bugs found and fixed

- **Keys pressed the column's buttons.** When a test window opened it took the keyboard focus. If
  the gear or "+ New tab" button had it, a Space or Enter typed for another app pressed it, opening
  stray Settings or blank tabs (seen in several earlier runs). The column's buttons, pins and close
  buttons no longer take keyboard focus.
- **The site card hung off the window's left edge**: a popover is centred on what it points at, and
  the card is wider than the field. It now points at the column's middle.

## Not done (yet)

- the site search turns the site into a chip in the field; rippl puts the keyword in front of
  your words, which does the same thing more plainly.
- it can read a site's own search (OpenSearch) to add it to the list; rippl's list is fixed
  (plus your keywords).
- The card has no per-site sound or autoplay switch yet (batch 3, with video keys).
