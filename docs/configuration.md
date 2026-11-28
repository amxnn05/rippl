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

# uBlock Origin's filter lists as well as EasyList's (on, default)
ublock = on

# The column as frosted glass the compositor blurs
glass = off

# Shift+click a link: its page in a panel over this one (on, default)
peek = on

# What plays in another tab, at the foot of the column, with its pause (off, default)
player = off

# Space and arrow keys on a video with the browser's own controls (off, default)
video_keys = off

# Ctrl+Shift+Y: paste a video link, Return downloads it with yt-dlp (on, default).
# yt-dlp must be installed (pacman -S yt-dlp); ffmpeg too, to merge video and sound or make .mp3.
grab = on

# What Return downloads there: video (the best, as .mp4; default) or audio (as .mp3).
# Shift+Return downloads the other one.
grab_format = video

# Darken pages that have no dark look of their own, in the dark theme (off, default)
dusk = off

# Sites' icons on tabs (on, default); off: their letters
icons = on

# The new tab shows the sites of the bookmark folder "Home" (off, default)
home_grid = off

# Animations: fades, slides, the column's slide (full, default) or everything immediate (reduced).
# With the desktop's own "reduce animation" on (gtk-enable-animations off), rippl is reduced too.
motion = full

# Pages stretch a little past their top and bottom and spring back (on, default)
elastic = on

# Pages, their animations and smooth scrolling at the screen's own refresh rate
# (display, default: 144 on a 144 Hz screen, 120 on a 120 Hz one) or near 60 (60: less power)
frame_rate = display

# Videos start by themselves without sound (muted, default) or wait for a click (never).
# A site's card can let it play with sound.
autoplay = muted

# A click of the middle button on a page scrolls it towards the pointer (off, default)
autoscroll = off

# The web cache on disk is kept under this (512m, default; 128m, 256m, 1g, 2g).
# Checked a minute after launch and every 6 hours; the biggest sites go first.
cache = 512m

# Tabs left unseen this long close themselves, into rippl://archive
# (never, default; 12h, 1d, 7d, 30d)
close_after = never

# Ctrl+Tab shows the tabs as pictures, latest first (on, default); off: the next tab
tab_switcher = on

# A row of bookmarks above the page (off, default)
bookmarks_bar = off

# Tabs down the side (side, default) or across the top (top)
tabs = side

# The column with titles (full, default) or as icons only (icons)
column = full

# Behind the new tab's field: off (default), a painted wall (open-road, mountain-lake,
# northern-lights), solid or gradient (of home_colour), or image (home_image)
home_theme = off
home_colour = #4285f5
home_image = /home/you/Pictures/wall.jpg

# The AI add-on (off, default): ask about a page (Ctrl+Shift+A), translate it (Ctrl+Shift+L),
# with a model running on this computer, through its OpenAI-style API
ai = off
ai_endpoint = http://127.0.0.1:11434/v1
ai_model = llama3.2
translate_to = English

# Let a coding agent use rippl through `rippl --mcp` (off, default)
agents = off
agents_see_tabs = off

# Where pages and rippl's own requests go out: system (default), direct, or a proxy,
# e.g. socks5://127.0.0.1:9050 (Tor) or http://proxy.example:3128. Never a password.
proxy = system

# Email links open in a webmail: none (default: your mail app), gmail, outlook, fastmail, proton
webmail = none

# Sync bookmarks, pinned tabs, spaces and settings through a folder something else keeps the
# same on your devices (Syncthing, Nextcloud, a shared disk). Empty (default): off.
sync_folder = /home/you/Sync

# Your own keys for a command (rippl://settings lists them all); none for no keys
shortcut.new-tab = <Ctrl>t, <Ctrl>n

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
removes the site in front, at once: its open tabs get views without the filters (or with them
again), keeping their history. While a site is on the list, the address field shows a warning icon.

## Glass and Hyprland

With `glass = on` (off by default), rippl asks the compositor to blur behind its column (GTK 4.24's
`backdrop-filter`, sent as `ext-background-effect-v1`). Hyprland's own blur settings decide how it
looks. Omarchy also makes every window 95% opaque, which shows the desktop faintly through the page
too; a window rule giving `dev.rippl.Rippl` opacity 1 keeps the page solid while the column stays
frosted.

## Client certificates

Put each identity (certificate and key) in `~/.local/share/rippl/certificates/`: a `.pem` with
both, or a `.p12` / `.pfx` without a password. A site that asks gets a dialog to choose one or
none.

## Agents (MCP)

With `agents = on`, add rippl to a coding agent as an MCP server, e.g. Claude Code:
`claude mcp add rippl -- rippl --mcp`. It works in tabs of its own (✦), reads yours only with
`agents_see_tabs = on`, and never types into password fields.

## Profiles

`rippl --profile work` runs a separate browser: its own spaces, tabs, sign-ins, history, bookmarks
and session. Settings, passwords, downloads and the ad-block lists are shared. It can run beside the
default profile; its control socket is `rippl-work.sock` (`RIPPL_PROFILE=work scripts/ctl.sh …`).

## Environment

| Variable | Effect |
|---|---|
| `RIPPL_SLEEP_AFTER=<seconds>` | Overrides `sleep`, in seconds (for measuring) |
| `RIPPL_CLOSE_AFTER=<seconds>` | Overrides `close_after`, in seconds (for measuring) |
| `RIPPL_CACHE_CHECK=<seconds>` | When the first cache check runs (default 60), for testing `cache` |
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
| `Ctrl+Tab` (hold `Ctrl`, `Tab` again to walk on) | The tab switcher: tabs as pictures, latest first; let go of `Ctrl` to go there, `Esc` to stay. A quick `Ctrl+Tab` is the tab you were on before |
| `Ctrl+PgDn` / `Ctrl+PgUp` | Next / previous tab in the column |
| Middle-click a page (`autoscroll = on`) | Scroll towards the pointer; click, `Esc` or the wheel stops it |
| `Ctrl+1`–`Ctrl+8`, `Ctrl+9` | Go to that tab / the last tab |
| `Alt+←` / `Alt+→` | Back / forward |
| `Ctrl+R`, `F5` / `Ctrl+Shift+R` | Reload / reload without the cache |
| `Ctrl+Shift+B` | Shield off (or on) for this site |
| `Ctrl+Shift+Y` | Download a video: paste its link (the clipboard's, or the page's, is already there), `Return` for video, `Shift+Return` for audio only, `Esc` to close. `shortcut.grab` moves it |
| `Ctrl+D`, click the ☆ in the field | Bookmark the page, or remove the bookmark |
| Focus the field with nothing typed | Bookmarks, then recent pages |
| `Ctrl+H` | `rippl://history`: search, forget, clear |
| `Ctrl+J` | `rippl://downloads`, a page of its own: every download, open, show in folder, try again (also a click on *Downloads* at the column's foot) |
| `Ctrl+Shift+O` | `rippl://bookmarks`: remove, import from another browser |
| `Ctrl+,` | `rippl://settings`: permissions, Shield exceptions, hidden elements, configuration |
| `Ctrl+Alt+P` | `rippl://passwords`: saved accounts (delete), sites never offered saving |
| `Ctrl+Shift+P` | Pin or unpin the tab |
| `F9`, `Ctrl+Alt+R` | Reading mode on/off |
| `Ctrl+Shift+H` | Hide something on this site (click it; `Esc` cancels) |
| `Ctrl+Shift+U` | Show everything hidden on this site again |
| `Ctrl+Alt+S` | Split view: this tab beside the previous one, or take it out of its split. Splits are kept in the session |
| `Ctrl+Alt+G`, a tab's menu's *Group tabs from …* | This site's tabs as one row of tiles (click a tile: that tab); again on a grouped tab: loose again. `shortcut.group-site` moves it |
| Alt-click a tab, its menu's *Put beside this tab* | Add it to the split in front (up to four: 3 across, 2×2) |
| `Ctrl+Shift+T` | Reopen the last closed tab, with its history and scroll |
| Click the tab you're on, `Ctrl+I` | Site card: connection, copy, print, zoom, ad blocking, keep awake, pop-ups, permissions |
| Right-click a tab | Duplicate, Pin, Put beside this tab, Copy address, Reload, Close, Close other tabs |
| `Ctrl+Shift+D` | Duplicate the tab, with its history and scroll |
| `Ctrl+P` | Print |
| `Ctrl+Shift+S` | Capture the whole page: PNG in ~/Pictures/rippl, and on the clipboard |
| `Ctrl+Alt+A` | Save the page as a web archive (.mhtml) in Downloads |
| `Shift+click` a link | Peek at it; `Esc` puts it away, `Ctrl+Return` keeps it as a tab |
| `Alt+1`–`Alt+9`, two fingers sideways on the column | Go to that space / the next or previous |
| `Ctrl+Shift+A` | Ask about this page / summarize it (`ai = on`) |
| `Ctrl+Shift+L` | Translate the page, or show the original (`ai = on`) |
| `Ctrl+F` on Settings | Find a setting |
| `Ctrl+N` | New window (its own tabs; shares sign-ins, history, settings) |
| `Ctrl+Alt+T`, **+** at the column's foot | New space (its own tabs and sign-ins) |
| Right-click a space's button | Rename it, colour it (or a gradient), or delete it with its tabs and sign-ins |
| Right-click a pin | Put it in a pin folder, or take it out |
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
