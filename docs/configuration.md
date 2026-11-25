# Configuration

## `~/.config/rippl/config`

Optional. `key = value`, one per line, `#` for comments. If the file or a key is missing, the
default is used. Read once at launch.

```ini
# duckduckgo (default), brave, startpage, mojeek, qwant,
# or an https address with {} where the words go, e.g. your own SearXNG:
#   search = https://searx.example.org/search?q={}
search = duckduckgo

# Ad and tracker blocking: on (default) or off
shield = on

# Minutes a tab sits unseen before it sleeps and gives its memory back (default 30)
sleep = 30

# Keep history (on, default) or not (off). Bookmarks are kept either way.
history = on

# system (default), light or dark: the window, and pages that offer both.
# system follows the desktop's own setting, live.
theme = system

# Upgrade every http request to https before it leaves (on, default)
https_only = on

# Send Global Privacy Control, "do not sell or share my data" (on, default)
gpc = on

# Suggestions from the search engine while you type (off, default).
# On, each keystroke is sent to the engine (DuckDuckGo and Brave offer them).
suggestions = off

# Your own site keywords: "npm left-pad" searches npm. One %s, https.
keyword.npm = https://www.npmjs.com/search?q=%s
```

All of these can also be changed on `rippl://settings` (`Ctrl+,`). Changes are checked, written
into this file (your comments and other lines stay), and applied right away, except ad blocking,
which applies after a restart.

See [privacy.md](privacy.md) for how to choose a search engine.

## `~/.config/rippl/shield-allow`

Sites Shield is turned off for, one host per line (subdomains included). `Ctrl+Shift+B` adds or
removes the site in front, recompiles the filters (about 6 s) and reloads the page. While a site
is on the list, the address field shows a warning icon.

## Profiles

`rippl --profile work` runs a separate browser: its own spaces, tabs, sign-ins, history, bookmarks
and session. Settings, passwords, downloads and the ad-block lists are shared. It can run beside the
default profile; its control socket is `rippl-work.sock` (`RIPPL_PROFILE=work scripts/ctl.sh …`).

## Environment

| Variable | Effect |
|---|---|
| `RIPPL_SLEEP_AFTER=<seconds>` | Overrides `sleep`, in seconds (for measuring) |
| `RIPPL_LEAN=1` | WebKit's smaller cache model: less RAM, slower Back |
| `RIPPL_NO_SHARE=1` | Every tab in a page process of its own, even on the same site |
| `RIPPL_NO_GPU=1` | Pages drawn without the GPU: 167 → 95 MB per light page, but about twice the CPU while scrolling ([performance.md](performance.md)) |
| `RIPPL_CONTROL=1` | Opens the control socket ([testing.md](testing.md)) |
| `RIPPL_DEBUG=1` | Logs tab life and Shield activity to stderr |
| `RIPPL_EXTENSION=/path/librippl_web_extension.so` | Use this web process extension (otherwise beside the binary, then `/usr/lib/rippl`) |
| `RIPPL_KEYRING=memory` | Keep passwords in memory instead of the system keyring (tests) |
| `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `XDG_CONFIG_HOME` | Where everything is kept; point them at a temporary folder to test without touching your real profile |

## Keys

| Key | Action |
|---|---|
| `Ctrl+L`, `F6`, `Alt+D` | Focus the address field |
| `Esc` in the field | Put the page's address back and return to the page |
| `Ctrl+T` / `Ctrl+Shift+N` | New tab / new private tab |
| `Ctrl+W`, `Ctrl+F4`, middle-click a tab | Close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` (`Ctrl+PgDn` / `PgUp`) | Next / previous tab |
| `Ctrl+1`–`Ctrl+8`, `Ctrl+9` | Go to that tab / the last tab |
| `Alt+←` / `Alt+→` | Back / forward |
| `Ctrl+R`, `F5` / `Ctrl+Shift+R` | Reload / reload without the cache |
| `Ctrl+Shift+B` | Shield off (or on) for this site |
| `Ctrl+D`, click the ☆ in the field | Bookmark the page, or remove the bookmark |
| Focus the field with nothing typed | Bookmarks, then recent pages |
| `Ctrl+H` | `rippl://history`: search, forget, clear |
| `Ctrl+Shift+O` | `rippl://bookmarks`: remove, import from another browser |
| `Ctrl+,` | `rippl://settings`: permissions, Shield exceptions, hidden elements, configuration |
| `Ctrl+Alt+P` | `rippl://passwords`: saved accounts (delete), sites never offered saving |
| `Ctrl+Shift+P` | Pin or unpin the tab |
| `F9`, `Ctrl+Alt+R` | Reading mode on/off |
| `Ctrl+Shift+H` | Hide something on this site (click it; `Esc` cancels) |
| `Ctrl+Shift+U` | Show everything hidden on this site again |
| `Ctrl+Alt+S` | Split view: this tab beside the previous one, or separate them |
| `Ctrl+Shift+T` | Reopen the last closed tab, with its history and scroll |
| Click the tab you're on, `Ctrl+I` | Site card: connection, copy, print, zoom, ad blocking, keep awake, pop-ups, permissions |
| Right-click a tab | Duplicate, Pin, Put beside this tab, Copy address, Reload, Close, Close other tabs |
| `Ctrl+Shift+D` | Duplicate the tab, with its history and scroll |
| `Ctrl+P` | Print |
| `Alt+1`–`Alt+9` | Go to that space |
| `Ctrl+Alt+T`, **+** at the column's foot | New space (its own tabs and sign-ins) |
| Right-click a space's button | Rename it, or delete it with its tabs and sign-ins |
| Click / right-click a group's heading | Fold or unfold / rename or ungroup |
| `Ctrl+Shift+C` | Copy the address, without tracking parameters |
| `Ctrl+Shift+V` | Paste and go: open what's on the clipboard |
| `Ctrl+K` | Switch to a tab: the field, empty; type to find open tabs |
| `w` `yt` `gh` `mdn` `osm` `r` + words in the field | Search that site: Wikipedia, YouTube, GitHub, MDN, OpenStreetMap, Reddit |
| Alt-click a tab | Put it beside the tab in front |
| `Ctrl+Alt+←` / `Ctrl+Alt+→` | Left / right pane |
| `↓` `↑` `Return` in the field | Move through what the field offers, open it |
| `Ctrl+F` | Find in page (`Return`/`Shift+Return` next/previous, `Esc` closes) |
| `Ctrl+G` / `Ctrl+Shift+G`, `F3` / `Shift+F3` | Next / previous match |
| `Ctrl+S` | Hide or show the tab column |
| `Ctrl+Shift+I`, `F12` | Web Inspector |
| `Ctrl++` `Ctrl+-` `Ctrl+0` | Zoom |
