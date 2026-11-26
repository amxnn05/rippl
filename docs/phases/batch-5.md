# Feature batch 5: moving in — import history, tabs and passwords, folder sync, proxy, client certificates, webmail, download pause/resume, media keys

**Status:** done (2026-11-26). Plan: [features.md](../features.md).

## Media keys

Nothing to build: WebKitGTK 2.54 has MPRIS built in (its MediaSession feature, on). A page that
plays registers `org.mpris.MediaPlayer2.dev.rippl.Rippl…` with the identity "rippl", the page's
title and the media's length, and the desktop's play/pause key reaches it. Measured with `busctl`:
`PlayPause` paused a playing video; once paused, WebKit takes rippl off the bus within seconds,
so the keys go back to Spotify or whatever played before: the problem the 
works around on the Mac doesn't happen here.

## Proxy (`proxy`)

the . `proxy = system` (default: the desktop's settings), `direct`, or
`http://host:port`, `socks5://host:port` (`socks4`, `https` too; a user name may come before an
`@`, never a password: the config is plain text). It goes on **every** network session (the
first space's, each space's, each private tab's, `Engine::set_proxy`) and on rippl's own
requests (filter lists, engine suggestions, resumed downloads, `engine::proxy_resolver`): a proxy
that only covered pages would let a favicon or a suggestion give the sites away. This machine
and `.local` names stay direct. Changing it applies at once, open pages included.

Tested with a logging proxy: a page went through it, `127.0.0.1` stayed direct, and `direct`
went round it.

## Client certificates

the . Linux has no keychain of identities, so rippl keeps them in
`~/.local/share/rippl/certificates/`: each file a certificate with its key (`.pem`, or `.p12` /
`.pfx` without a password). When a site asks (WebKit's `authenticate` signal,
`ClientCertificateRequested`), a dialog names the site and lists each identity ("Alice Tester —
issued by Test Authority, until 28 Nov 2026") and **Don't send one**. Nothing is sent unless
chosen. The answer holds for that site until rippl closes; a private tab asks for itself. A smart
card's PIN isn't offered. Other sign-in boxes (HTTP Basic) are WebKit's own.

Tested with a TLS server asking for a certificate (a throwaway CA): with the identity chosen the
server saw "Alice Tester"; the dialog shows as above.

## Email links in a webmail (`webmail`)

the . `webmail = none` (default: your mail app, through the desktop), `gmail`,
`outlook`, `fastmail`, `proton`. A `mailto:` link opens the compose page in a new tab with the
draft filled in. The mailto is read as RFC 6068 has it (`rippl_core::mail`): `+` stays a plus,
CRLF becomes a line break, the first subject and body count, at most 50 of each kind of address;
every value is percent-encoded down to the unreserved characters, so a subject like `1&2=3 #4`
can't start another field; a body is shortened, never dropped, to keep the address under 8,000
bytes. Tested: a link with a two-line body opened Fastmail's compose page filled in.

## Moving in: history, open tabs, passwords

rippl://bookmarks lists every other browser found, each with **Bookmarks and history**, **Open
tabs**, and, for the Chromium family, **Passwords**. Everything is read from copies; nothing in the
other browser changes, and it may be running (`rippl_core::import_more`).

- **History**: Chromium's `History`, Firefox's `places.sqlite`, the last 20,000 pages, only
  with `history = on`. Known pages keep the larger visit count and the later visit.
- **Open tabs**: Chromium's newest `Sessions/Session_*`, its SNSS command log replayed (tabs to
  windows, their order, each tab's selected entry, pins; closed tabs and windows left out);
  Firefox's `recovery.jsonlz4`, decompressed with a small LZ4 block decoder. They come asleep,
  pins pinned, into a new space "From Brave" (or the browser's name).
- **Passwords** (Chromium family): `Login Data`, each password AES-128-CBC under a PBKDF2-SHA1 key:
  `v11` from "<Browser> Safe Storage" in the system keyring (looked up by its `application`
  attribute: `chrome`, `chromium`, `brave`, `microsoft-edge`, `vivaldi`; the keyring may ask to
  unlock), or `v10` under Chromium's fixed key. Saved into your keyring as rippl's own accounts;
  the page asks before starting. Firefox's are encrypted with its own NSS key store: not done.

Tested against a fake Brave profile in a scratch home folder: 1 bookmark and 2 pages of history,
2 open tabs (one pinned) into "From Brave", one `v10` password into the test keyring. `v11` is
covered by a unit test (a password sealed and unsealed), not by a real keyring.

## Download pause and resume

WebKitGTK can't pause a download. rippl's pause button keeps what has come (a hard link to WebKit's
`<name>.wkdownload`, made before the download is stopped: nothing copied) as `<name>.part`, then
stops it. Resume asks the server for the rest with `Range: bytes=<have>-`, sending the tab's own
cookies (from its space's cookie store) and user agent, through the proxy setting, and appends it;
a server that ignores the range sends it all, which starts the part over. Pause works on a resumed
download too; × removes the part.

Tested with a slow server: paused at 1,376,256 bytes, the server then got `Range:
bytes=1376256-`, and the finished 4 MB file was identical to the original, byte for byte.

## Sync through a folder (`sync_folder`)

the  and : no account, no server. `sync_folder = <path>`: any
folder that Syncthing, Nextcloud, Dropbox or a shared disk keeps the same on your devices. Each
device writes `<folder>/rippl sync/<profile>/<device id>.json` (only it writes that file) and
reads the others', at start, every two minutes, and when the setting changes.

- **What syncs**: bookmarks (address, title, folder), pinned tabs (address, title, space, pin
  folder), spaces (name, colour; sync never deletes a space, which would take its tabs and
  sign-ins), and 22 settings about how you like rippl, and your shortcuts.
- **Never**: history, passwords, cookies and sign-ins, open tabs that aren't pinned, private
  tabs, the proxy, the sync folder.
- **The merge** (`rippl_core::sync`): items with stamps; what changed here is found against
  the last sync (the mirror); per item, the newest anywhere wins; a deletion travels as a
  tombstone, kept 60 days. A device's first sync gives way on conflicts, so a new device's
  untouched defaults don't overwrite what the others share.

Tested with two scratch profiles taking turns on one folder: a fresh device took the 12
bookmarks, a pin, a space and the dark theme; then a bookmark added and one deleted there were
added and deleted on the first device at its next start.

## Tests

91 (87 core, 4 app). New: the proxy setting, the mailto reader and compose addresses, LZ4 and
Firefox's session, a Chromium SNSS session replayed, Chromium passwords unsealed (v10, v11, wrong
key), history import, and the sync merge (two devices converging, newer wins, a new device gives
way, tombstones expire, files round trip).

## For testing

New control commands: `trust <pem> <host>`, `certificates`, `certificate <host> <n|none>`,
`download-pause <n>`, `sync`. Test servers used: a logging HTTP proxy, a TLS server asking for a
client certificate, a slow server honouring `Range`, all in the scratch folder.
