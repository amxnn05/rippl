# Every feature, and where it stands

another browser is the macOS WebKit browser rippl takes its ideas and its look
from. This page lists the features, read from its README, release notes (NOTES.md), roadmap and
the header of each of its source files, with rippl's status for each.

| Mark | Meaning |
|---|---|
| ✅ | In rippl |
| 🔨 | Can be built on Linux / WebKitGTK; planned, in the batch shown |
| 🍎 | Tied to macOS or Apple services; a Linux equivalent is noted where there is one |
| ⛔ | Not possible on WebKitGTK today |

## Look

| Feature | rippl |
|---|---|
| Palette: every colour a light/dark pair | ✅ `look.rs`: the same greys, CSS variables, `prefers-color-scheme` |
| Page as a card beside the column (8 px gap, 10 px corners) | ✅ |
| Column of tabs, 28 px rows, 12.5 pt titles, live tab on a grey pill | ✅ |
| Site letter in a faint square | ✅ letters; site icons 🔨 batch 3 |
| Pinned tabs as letter squares at the top | ✅ |
| Quiet "+ New tab" row | ✅ |
| Settings from the column's foot | ✅ (gear) |
| Tabs across the top instead | 🔨 batch 4 |
| Column folds away | ✅ Ctrl+S hides it; the slide animation 🔨 |
| Column as icons only | 🔨 batch 4 |
| Glass window, Liquid Glass | 🍎 (Hyprland/compositor blur is the Linux route) |
| Traffic lights, title bar | 🍎; rippl hides the title bar on tiling compositors |
| Blank-tab walls, stickers, themes | 🔨 batch 4 |
| Home grid of bookmarks on a new tab | 🔨 batch 4 |

## Tabs and windows

| Feature | rippl |
|---|---|
| One web view per tab, instant switching | ✅ |
| Lazy restore of last session | ✅ |
| Tabs sleep and give memory back | ✅ plus Linux PSI pressure |
| Keep a site awake (site card) | ✅ |
| Reopen closed tab with history and scroll (⇧⌘T) | ✅ Ctrl+Shift+T |
| Pinned tabs | ✅ Ctrl+Shift+P |
| Pin folders, live folders from feeds | 🔨 batch 4 (pins as rows first) |
| Tab groups | ✅ fold, rename, ungroup |
| Split view, up to 4 pages | ✅ 2 pages; 3–4 🔨 batch 4 |
| Spaces: sets of tabs, each with its own sign-ins | ✅ Alt+1–9, own store per space |
| Space swipes and colours | 🔨 batch 4 |
| Profiles | ✅ `rippl --profile NAME`, a window each |
| Profiles in one window | 🔨 later |
| Send links to a space | 🔨 Phase 9 (with links from other apps) |
| Several windows, same tabs in every window | 🔨 batch 4 |
| ⌃Tab tab switcher with pictures | 🔨 batch 4 |
| ⌘K switch to a tab | ✅ Ctrl+K: the field finds open tabs |
| ⌘1–⌘9 | ✅ Ctrl+1–9 |
| Tabs close themselves; archive of closed tabs | 🔨 batch 4 |
| Duplicate tab (⌘D) | ✅ Ctrl+Shift+D, tab menu |
| Tab menu (right-click a tab) | ✅ |
| Peek at a link in a panel | 🔨 batch 3 |
| Little window for links from other apps | 🔨 with Phase 9 (default browser) |
| Choose the space a link opens in | 🔨 Phase 9 |
| Private tabs (⇧⌘N) | ✅ Ctrl+Shift+N |
| Unread counts from titles | 🔨 batch 3 |
| Media player at the foot of the column | 🔨 batch 3 |
| Floating video / picture in picture | ⛔ WebKitGTK has no picture-in-picture API |
| Media keys for the playing tab | 🔨 batch 5 (MPRIS) |

## The field

| Feature | rippl |
|---|---|
| One field: address or search, never sends before Return | ✅ |
| Completion from history, typo tolerance | ✅ |
| Site keywords, "yt cats" | ✅ built-in set and your own (`keyword.<word>` in the config) |
| Search a site from the field | ✅ as a row; OpenSearch discovery 🔨 later |
| Commands in the field | ✅ rippl's pages by name; menu commands 🔨 with batch 4 shortcuts |
| Search engine suggestions, off by default | ✅ off by default |
| Address always in sight | ✅ the field |
| Where a link goes, at the foot of the page | ✅ |
| Copy address / copy clean link | ✅ Ctrl+Shift+C, always clean |
| Paste and go (⇧⌘V) | ✅ Ctrl+Shift+V |

## Privacy and blocking

| Feature | rippl |
|---|---|
| Ad blocker before the page | ✅ EasyList + EasyPrivacy |
| YouTube ads | 🔨 batch 3 |
| Off per site | ✅ Ctrl+Shift+B (6 s recompile; instant 🔨) |
| Hide anything, for good | ✅ Ctrl+Shift+H / U |
| Your own CSS for a site, Boosts | 🔨 batch 3 |
| Dark pages for sites without | 🔨 batch 3 |
| Click IDs off links | ✅ in every request |
| Global Privacy Control | ✅ |
| HTTPS first (`HTTPSFirst`) | ✅ HTTPS-only with a warning page |
| Site card and site permissions | ✅ Ctrl+I or a click on the tab you're on |
| Blocked pop-ups with Open / Always Allow | ✅ |
| Proxy | 🔨 batch 5 (NetworkProxySettings) |
| Client certificates | 🔨 batch 5 |
| Private tab with its own cookie jar | ✅ |

## Passwords and forms

| Feature | rippl |
|---|---|
| Passwords in the system keychain | ✅ Secret Service (GNOME Keyring) |
| Offered under the box, saved after a sign-in works | ✅ |
| Import from Chrome, Arc, Brave, Edge | 🔨 batch 5 (Chromium on Linux keeps its key in the keyring) |
| Passkeys | ⛔ no WebAuthn platform authenticator in WebKitGTK yet |
| Apple Passwords app, verification codes | 🍎 |
| Touch ID before filling | 🍎 (polkit/fprintd is a possible Linux route) |
| Your contact card under form boxes | 🍎 (Evolution Data Server would be the route) |

## Reading and pages

| Feature | rippl |
|---|---|
| Reading mode | ✅ F9 |
| Find on page | ✅ Ctrl+F |
| Translate on device | 🔨 batch 6 (needs a local model) |
| Capture part or all of a page | 🔨 batch 3 (WebView snapshot) |
| Save page as web archive | 🔨 batch 3 (WebView save as MHTML) |
| Print | ✅ Ctrl+P |
| Web Inspector | ✅ Ctrl+Shift+I |
| Developer tools packages | ⛔/🍎 (the Web Inspector is built in) |
| Zoom, smooth wheel, 120 Hz | ✅ zoom; WebKitGTK scrolls smoothly; refresh rate follows the compositor |
| Video keys, autoplay choice | 🔨 batch 3 |
| Swipe back/forward | ✅ WebKit's gestures |
| Middle-button autoscroll | 🔨 batch 4 |

## History, bookmarks, downloads

| Feature | rippl |
|---|---|
| History, searchable | ✅ rippl://history |
| Bookmarks with folders | ✅ |
| Bookmarks bar | 🔨 batch 4 |
| Import from other browsers: bookmarks, history, open tabs, Arc/Zen spaces | ✅ bookmarks (Chromium family, Firefox family); history and tabs 🔨 batch 5 |
| Downloads list, pause/resume | ✅ list; pause/resume 🔨 batch 5 |
| Email links in a webmail | 🔨 batch 5 |

## Sync and devices

| Feature | rippl |
|---|---|
| Sync with iCloud | 🍎 |
| Sync through a shared folder | 🔨 batch 5: the same idea works with Syncthing or any synced folder |
| Handoff | 🍎 (KDE Connect would be the Linux route) |
| AppleScript | 🍎 (the control socket is rippl's scripting door) |
| Share sheet | 🍎 (the XDG portal could share) |

## Extensions, agents, AI

| Feature | rippl |
|---|---|
| Chrome extensions | ⛔ WebKitGTK has no WebExtensions API |
| Safari extensions from apps | 🍎 |
| Script-driven test socket | ✅ `control.rs` + `scripts/ctl.sh` |
| Coding agents using the browser over MCP | 🔨 batch 6 (an MCP server over the control socket) |
| AI add-on: summarize/ask a page, on device | 🔨 batch 6 |

## App

| Feature | rippl |
|---|---|
| Settings as a page, searchable | ✅ rippl://settings (search 🔨) |
| Your own keyboard shortcuts | 🔨 batch 4 |
| Welcome on first launch | 🔨 Phase 9 |
| What's new | 🔨 Phase 9 |
| Updater | 🍎 — packages update through the distribution |
| Default browser, links from other apps | 🔨 Phase 9 |
| Address field from any app | 🔨 Phase 9: `rippl --field` bound to a key in the compositor |
| Notifications | ✅ asked per site; WebKitGTK shows them |
| Screen recording for extensions | ⛔ (no extensions); screen sharing in pages ✅ through the portal |

## Batches

1. ✅ **Small, daily** ([notes](phases/batch-1.md)): site card, keep awake, duplicate,
   tab menu, blocked pop-ups, typos, site search, your own keywords, page commands, print, opt-in
   engine suggestions.
2. ✅ **Organising** ([notes](phases/batch-2.md)): spaces with their own sign-ins, profiles,
   tab groups. (Pin folders moved to 4, sending links to a space to Phase 9.)
3. **Pages**: dark pages (Dusk), your own CSS per site, capture, save as archive, site icons, peek,
   YouTube ads, unread counts, video keys, player in the column.
4. **Layout**: tabs across the top, icon column, several windows, Ctrl+Tab switcher, bookmarks bar,
   home grid and themes, tabs closing themselves and their archive, 3–4 pages in split view, your
   own shortcuts.
5. **Moving in**: import history, open tabs and Chromium passwords; folder sync; proxy; client
   certificates; download pause/resume; webmail for mailto; media keys.
6. **Big**: translation and the AI add-on on device, agents over MCP.

Phase 9 (shipping: desktop entry, default browser, packages, welcome) can happen between any of them.
