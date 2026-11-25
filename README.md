# rippl

A small, fast, private web browser for Linux, built on WebKit (WebKitGTK 6.0 + GTK4) and written in Rust.

A row of tabs, one field, and the page. No account, no telemetry, no server. Ads and trackers are blocked
inside WebKit before they load, and tabs you aren't using give their memory back.

## Why it uses less memory

Most of a browser's memory is its web processes, one per open page. rippl keeps as few of them alive as
it can:

- **Lazy restore.** Tabs from the last session cost nothing until you click them.
- **Tabs sleep.** A tab unseen for 30 minutes ends its web process and keeps its history and scroll
  position, so it comes back where you left it.
- **Memory pressure.** rippl reads Linux PSI. When memory is tight, idle tabs sleep sooner, and at once
  when it is critical.
- **Shield.** EasyList and EasyPrivacy (~116k rules) are compiled into WebKit's own content filters.
  On an ad-heavy page that saved **20–47% of memory**, and the page loads less.
- **Per-process limits** and heap trimming.

## Everyday things

The field offers your open tabs, bookmarks and history as you type, ranked by how you use them, and
completes addresses you've visited (`you` → `youtube.com`). Downloads go to `~/Downloads`, never
overwriting a file, and are listed at the foot of the column. Find in page, permission prompts that
remember your answer per site, and light/dark mode.

## Private by default

DuckDuckGo search (or Brave, Startpage, Mojeek, Qwant, or your own SearXNG). Third-party cookies are
blocked, WebKit's tracking prevention is on, and cross-site pings are blocked. Nothing leaves your
machine except the pages you open and the filter-list updates. Details: [docs/privacy.md](docs/privacy.md).

## Build

```sh
sudo pacman -S --needed webkitgtk-6.0 gtk4      # Debian/Ubuntu: libwebkitgtk-6.0-dev libgtk-4-dev
cargo build --release
./target/release/rippl [url…]
```

`Ctrl+L` address · `Ctrl+T` new tab · `Ctrl+W` close · `Ctrl+Shift+N` private tab · `Ctrl+Tab` next ·
`Ctrl+1`–`9` jump · `Alt+←/→` back/forward · `Ctrl+F` find · `Ctrl+D` bookmark · `Ctrl+H` history ·
`Ctrl+Shift+B` Shield off for this site · `Ctrl+S` hide tabs.
All keys and settings: [docs/configuration.md](docs/configuration.md).

## Documentation

Everything is in [docs/](docs/README.md): [architecture](docs/architecture.md), [privacy](docs/privacy.md),
[configuration](docs/configuration.md), [testing](docs/testing.md), the [roadmap](docs/roadmap.md), and
one page per finished phase in [docs/phases/](docs/phases/).

Inspired by another browser, a WebKit browser for the Mac.
