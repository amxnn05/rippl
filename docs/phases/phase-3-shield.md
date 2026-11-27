# Phase 3: Shield and privacy

**Status:** done (2026-11-25)

## Goal

Block ads and trackers before they load, so pages are lighter, faster and can't follow you. Close
the privacy gaps left by the defaults, and choose a search engine that doesn't profile you.

## What was built

### Shield

- **Converter** (`rippl_core::shield::convert`, pure Rust, 7 tests). It turns Adblock Plus /
  EasyList syntax into WebKit content-blocker JSON, the format Safari uses:
  - network rules: `||host^`, `|https://…`, `*` and `^` wildcards
  - options: `third-party`/`~third-party`, `script`, `image`, `stylesheet`, `font`, `media`,
    `xmlhttprequest`, `websocket`, `ping`, `other`, `popup`, `subdocument`, `domain=`, `match-case`
  - exceptions: `@@…` become `ignore-previous-rules`; `@@||site^$document` turns blocking off on a site
  - element hiding: `##sel` (in batches of 100 selectors), `site##sel` (grouped by domain set), and
    `site#@#sel` exceptions (the generic selector gets `unless-domain`)
  - skipped: regex rules, scriptlets (`##+js`), `$csp`, `$redirect`, `$removeparam` and similar,
    extended CSS (`:has-text`, `:-abp-has`, …), and rules mixing included and excluded domains
  - rules are ordered blocks, then hides, then exceptions, because an exception only cancels the
    rules before it
  - always added: block third-party pings
- **Lists**: EasyList (ads) and EasyPrivacy (trackers), each compiled into **its own filter**. That
  keeps each one under WebKit's per-list rule limit, and an update to one doesn't recompile the
  other.
- **Seed list**: about 25 of the largest ad and tracking networks, compiled in milliseconds on first
  launch, so blocking starts before the full lists have downloaded.
- **Launch**: compiled filters load from WebKit's store in **5 ms**. Pages wait for them, up to 1 s,
  so even the first requests of the restored tab are blocked (the same reasoning as the
  `Shield.whenReady`).
- **Updates**: a list is fetched again when it is older than 4 days (EasyList's own "Expires"),
  checked at launch and every 6 hours. The old filter keeps blocking until the new one is compiled,
  then it is swapped for every tab at once. A page that isn't a filter list (a captive portal) is
  rejected.
- **Per site**: `Ctrl+Shift+B` turns Shield off or on for the site in front. The site is added as an
  `ignore-previous-rules` exception for that domain, the filters are recompiled and the page reloads.
  A warning icon in the field shows that Shield is off here.

### Privacy defaults (`engine.rs`)

- Third-party cookies blocked (`CookieAcceptPolicy::NoThirdParty`), for private tabs too.
- Intelligent Tracking Prevention on, for private tabs too.
- Hyperlink auditing (`<a ping>`) off. *Correction (Phase 4): WebKitGTK 2.54 ignores this setting, so Shield's third-party ping rule is what actually blocks pings.*
- Pages can't read the clipboard on their own.
- **Cookies now persist** (`cookies.sqlite`). This was a Phase 1 bug: sign-ins were lost at every
  restart.

### Search engine and config

- `~/.config/rippl/config` (`config.rs`): `search`, `shield`, `sleep`. See
  [configuration.md](../configuration.md).
- DuckDuckGo by default; Brave Search, Startpage, Mojeek, Qwant, or your own SearXNG. The reasoning
  is in [privacy.md](../privacy.md).

## Measurements

Conversion and compile, this machine:

| List | Rules | Skipped | Convert | WebKit compile | Compiled size |
|---|---|---|---|---|---|
| EasyList | 59,113 | 913 | 60 ms | 3.2 s | 26 MB |
| EasyPrivacy | 56,726 | 110 | 33 ms | 2.6 s | 22 MB |
| seed | 26 | 0 | <1 ms | ms | 9 KB |

The same page, fresh cache, 25 s after opening (PSS of all processes):

| Site | Shield on | Shield off | Saved | Cookies stored on / off |
|---|---|---|---|---|
| cnn.com | 726 MB | 908 MB | **20%** | 18 / 49 |
| speedtest.net | 561 MB | 1068 MB | **47%** | 6 / 17 |

The HTTP cache written was also about half as large with Shield on (22 vs 30 MB, 6.4 vs 13 MB).

## Decisions

| Decision | Why |
|---|---|
| WebKit content filters, not a JavaScript blocker | They are enforced in WebKit's network process and style engine: no script runs in every page, and nothing is fetched to be thrown away |
| Our own converter instead of a downloaded pre-converted list | No third party in the loop, and we control what's skipped. Conversion takes 60 ms |
| Skip what WebKit can't express | A skipped rule only blocks less; a wrongly widened rule breaks sites |
| Lists fetched with libsoup in a session of their own | No cookies, no shared cache, generic user agent; the only request rippl makes on its own |
| Third-party pings blocked by default | Pings to another site are reports to a tracker |

## Known limits

- ~~Turning Shield off for a site takes about 6 s~~: instant since [phase 11](phase-11-polish.md),
  with a second content manager without filters for those sites.
- `^` at the very end of a URL can't be matched (WebKit's regex has no `|`), so a few rules match
  slightly less than written.
- ~~The HTTP cache folder is not yet capped~~: `cache` since [phase 11](phase-11-polish.md).
- Compiling happens in the UI process. Its heap grows during the compile and is trimmed afterwards.
