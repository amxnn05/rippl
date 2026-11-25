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
```

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
| `RIPPL_CONTROL=1` | Opens the control socket ([testing.md](testing.md)) |
| `RIPPL_DEBUG=1` | Logs tab life and Shield activity to stderr |
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
| `Ctrl+H`, `Ctrl+Shift+O` | Field with nothing typed: bookmarks, then recent pages |
| `↓` `↑` `Return` in the field | Move through what the field offers, open it |
| `Ctrl+F` | Find in page (`Return`/`Shift+Return` next/previous, `Esc` closes) |
| `Ctrl+G` / `Ctrl+Shift+G`, `F3` / `Shift+F3` | Next / previous match |
| `Ctrl+S` | Hide or show the tab column |
| `Ctrl+Shift+I`, `F12` | Web Inspector |
| `Ctrl++` `Ctrl+-` `Ctrl+0` | Zoom |
