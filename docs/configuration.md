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

# system (default), light or dark: the window, and pages that offer both
theme = system

# Upgrade every http request to https before it leaves (on, default)
https_only = on

# Send Global Privacy Control, "do not sell or share my data" (on, default)
gpc = on
```

All of these can also be changed on `rippl://settings` (`Ctrl+,`). Changes are checked, written
into this file (your comments and other lines stay), and applied right away, except ad blocking,
which applies after a restart.

See [privacy.md](privacy.md) for how to choose a search engine.

## `~/.config/rippl/shield-allow`

Sites Shield is turned off for, one host per line (subdomains included). `Ctrl+Shift+B` adds or
removes the site in front, recompiles the filters (about 6 s) and reloads the page. While a site
is on the list, the address field shows a warning icon.

## Environment

| Variable | Effect |
|---|---|
| `RIPPL_SLEEP_AFTER=<seconds>` | Overrides `sleep`, in seconds (for measuring) |
| `RIPPL_LEAN=1` | WebKit's smaller cache model: less RAM, slower Back |
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
| Alt-click a tab | Put it beside the tab in front |
| `Ctrl+Alt+←` / `Ctrl+Alt+→` | Left / right pane |
| `↓` `↑` `Return` in the field | Move through what the field offers, open it |
| `Ctrl+F` | Find in page (`Return`/`Shift+Return` next/previous, `Esc` closes) |
| `Ctrl+G` / `Ctrl+Shift+G`, `F3` / `Shift+F3` | Next / previous match |
| `Ctrl+S` | Hide or show the tab column |
| `Ctrl+Shift+I`, `F12` | Web Inspector |
| `Ctrl++` `Ctrl+-` `Ctrl+0` | Zoom |
