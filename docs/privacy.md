# Privacy

rippl has no account, no server, no telemetry, no crash reporting and no analytics. Nothing about
you is collected, because there is nowhere for it to go.

## What leaves your machine

| What | When | To whom |
|---|---|---|
| The pages you open, and what they load | When you open them | Those sites, minus what Shield blocks |
| Your search | Only when you press Return. rippl sends no suggestions while you type | Your search engine (DuckDuckGo by default) |
| Files you download | When you download them | Saved in your Downloads folder; the request goes to the site |
| Two filter-list downloads | First launch, then when older than 4 days (checked every 6 hours) | `easylist.to`: a plain GET with user agent `rippl`, no cookies |
| Anything else | Never | — |

There is no update check, no "phone home", no favicon service and no safe-browsing lookup. If any
future change adds a request, this table must change in the same commit.

## Default search engine: DuckDuckGo

It's the default because it:

- needs no account and keeps no search history tied to you;
- builds no profile and sells no personal data, according to its published privacy policy;
- gives good results for everyday searching;
- works without JavaScript, so it stays fast in a lean browser.

rippl adds no tracking parameters to the search URL. It sends `?q=<your words>` and nothing else.

You can switch to any of these in `~/.config/rippl/config` (`search = <name>`):

| Name | Engine | Why you might pick it |
|---|---|---|
| `duckduckgo` | DuckDuckGo | Default. Mature, private by policy, good results |
| `brave` | Brave Search | Its own independent index (not Google's or Bing's), no profiling |
| `startpage` | Startpage | Google's results, fetched for you so Google doesn't see you; EU (GDPR) |
| `mojeek` | Mojeek | Its own index, no tracking, based in the UK; results are thinner |
| `qwant` | Qwant | EU-based (France), GDPR |
| `https://…{}` | Your own, e.g. a **SearXNG** you host | The strongest option: nobody but you sees your searches |

Google is not on the list. You can still type `google.com` and use it like any other site, but rippl
will never send your searches there on its own.

No engine's privacy can be checked from outside. These are the ones whose policies and business
models do not depend on profiling you. If you want certainty, host SearXNG yourself.

## Protections that are on

| Protection | What it does | Where |
|---|---|---|
| **Shield** | Blocks ads and trackers (EasyList + EasyPrivacy, ~116k rules) inside WebKit's network layer before any request is sent, and hides ad elements before the page draws | `shield.rs`, [phase 3](phases/phase-3-shield.md) |
| **No third-party cookies** | A site embedded in another site's page can't set or read cookies, the classic cross-site tracking cookie. Same as Safari | `engine.rs` `harden()` |
| **Intelligent Tracking Prevention** | WebKit classifies sites that track across sites and partitions or purges their storage | `engine.rs` `harden()` |
| **No cross-site pings** | `<a ping>` and beacons sent to another site are blocked by Shield. (WebKitGTK 2.54 ignores its own hyperlink-auditing setting, so Shield is what stops them) | `rippl_core::shield` |
| **Permissions asked, per site** | Camera, microphone, screen, location, notifications, clipboard reading, DRM and storage access are all asked for; nothing is granted silently. Answers are kept per site, never for private tabs | `permissions.rs` |
| **History stays local and optional** | Kept in `places.sqlite` on your machine; never recorded from private tabs; `history = off` stops it | `rippl_core::places` |
| **Address suggestions are local** | Offers come from your own history, bookmarks and tabs; nothing typed is sent anywhere before Return | `omnibox.rs` |
| **Safe download names** | A site can't choose a path, a hidden file, or overwrite an existing file | `rippl_core::downloads` |
| **Clipboard** | Pages can't read the clipboard on their own | `engine.rs` |
| **Private tabs** (`Ctrl+Shift+N`) | Each has its own throwaway cookie jar and storage, and is never written to the session | `engine.rs` `view(private)` |
| **Sandboxed pages** | WebKitGTK runs every web process in a `bwrap` sandbox | WebKit |
| **Local data only** | History, session, cookies and settings live in your home folder (see [architecture.md](architecture.md#files-on-disk)) | — |
| **Control socket off** | Only with `RIPPL_CONTROL=1`; mode `0600` in your private runtime folder | `control.rs` |

## Not done yet (known gaps)

- **Global Privacy Control / Do Not Track headers.** WebKitGTK has no UI-process API to add a
  header to every request. It needs a web-process extension (planned).
- **HTTPS-only mode.** Typed addresses default to `https://`, but links to `http://` are followed as
  they are (planned).
- **Fingerprinting.** WebKit has some built-in defences (ITP and others). rippl adds none of its own yet.
- **WebRTC.** WebKit uses mDNS to hide your local IP, but WebRTC itself is on, because video
  calls need it.
- **Extended filter syntax.** Shield skips the ~1,000 rules WebKit can't express (scriptlets, `$csp`,
  `$removeparam`, `:has-text`). Skipping a rule only means blocking less, never breaking a page.
