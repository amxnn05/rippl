# Testing and measuring

## Unit tests

```sh
cargo test                 # everything
cargo test -p rippl-core   # the logic only: no display needed
cargo clippy --all-targets # kept at zero warnings
```

`rippl-core` holds every decision that can be tested without a window: address parsing, search
engines, the tab model and sleep policy, the session format, and the Shield converter. Put new logic
there when you can.

## Never test with your real profile

Point the XDG folders at a scratch directory. rippl then can't read or overwrite your real
session, cookies or settings:

```sh
T=$(mktemp -d)
export XDG_DATA_HOME=$T/data XDG_CACHE_HOME=$T/cache XDG_CONFIG_HOME=$T/config
./target/release/rippl https://example.com
```

## The control socket

Start rippl with `RIPPL_CONTROL=1` and drive it with `scripts/ctl.sh`. This doesn't need keyboard
focus, so it works while you use the desktop:

```sh
scripts/ctl.sh list                 # index, id, Lazy/Awake/Asleep, front, url, title
scripts/ctl.sh open example.com     # opens in front, replies "ok <index>"
scripts/ctl.sh select 2
scripts/ctl.sh close 2
scripts/ctl.sh sleep                # sleep every tab that may sleep, now
scripts/ctl.sh shield               # list status, fetch age, allowed sites
scripts/ctl.sh shield update        # fetch the lists again now
scripts/ctl.sh shield toggle <host> # as Ctrl+Shift+B
scripts/ctl.sh type <text>          # as typed in the field: prints the field and its offers
scripts/ctl.sh enter                # Return in the field
scripts/ctl.sh history [n]          # recent history: visits, url, title
scripts/ctl.sh clear-history
scripts/ctl.sh bookmark             # toggle the front page; `bookmarks` lists them
scripts/ctl.sh find <text>          # find in the front page; `found` prints the count
scripts/ctl.sh permissions          # saved answers and questions waiting
scripts/ctl.sh permissions answer allow|deny
scripts/ctl.sh permissions forget [host]
scripts/ctl.sh download <url>       # `downloads` lists them
scripts/ctl.sh pin                  # pin/unpin the front tab
scripts/ctl.sh reader               # reading mode on/off
scripts/ctl.sh pick                 # start the hide picker
scripts/ctl.sh hide <selector>      # hide on the front site, as the picker would; `hidden` lists
scripts/ctl.sh unhide               # as Ctrl+Shift+U
scripts/ctl.sh sources              # other browsers' profiles found; `import <n>` imports one
scripts/ctl.sh js <code>            # run script in the front page, answer as JSON
scripts/ctl.sh passwords            # every saved account (origin, username); `accounts` for the front page
scripts/ctl.sh save                 # the save question shown, if any; `save save|later|never` answers it
scripts/ctl.sh fill <username>      # pick that account for the front page, as the list under a box does
scripts/ctl.sh passwords delete <origin> <username>
scripts/ctl.sh pending              # sign-ins sent and waiting to be confirmed
scripts/ctl.sh popover              # whether the account list is showing
```

**Passwords: test with `RIPPL_KEYRING=memory`.** That never touches your real keyring. To sign in
on a test page:
`scripts/ctl.sh js "document.querySelector('#u').value='bob'; document.querySelector('#p').value='pw-123x'; document.querySelector('form').requestSubmit(); 1"`.
If you test the real keyring, use an origin like `http://127.0.0.1:8765`, check with
`secret-tool search --all origin http://127.0.0.1:8765`, and delete the item afterwards.

`js` is how tests read a page, for example
`scripts/ctl.sh js 'getComputedStyle(document.querySelector("#x")).display'`.

To test the importer without touching real browsers, start rippl with `HOME` pointing to a
scratch folder that contains fake profiles (`.config/google-chrome/Default/Bookmarks`,
`.mozilla/firefox/x.default/places.sqlite`). The tests in `rippl_core::import` show the formats.

To keep downloads out of your real `~/Downloads`, put
`XDG_DOWNLOAD_DIR="/some/scratch/dir"` in `$XDG_CONFIG_HOME/user-dirs.dirs` of the scratch profile.

To test permission prompts, serve a page from `python3 -m http.server` that calls
`navigator.geolocation.getCurrentPosition(...)` and writes the result into `document.title`. Then
read it with `scripts/ctl.sh list`.

Why a socket? Sending keys with `wtype` is unreliable while someone else is using the same
desktop, because focus moves. Hyprland 0.56 also changed its `hyprctl dispatch` syntax (it is Lua
now), so old `sendshortcut` commands fail.

## Measuring memory

```sh
scripts/mem.sh            # rippl and every WebKit process it started
scripts/mem.sh firefox    # any other browser, to compare
```

It reports **PSS** (proportional set size): each shared page is split between the processes that
share it. PSS is the fair number for comparing browsers; RSS counts shared libraries several times.

Results vary by ±40 MB from run to run (GPU drivers, fonts, network), so compare several runs.

## A standard sleep/wake check

```sh
RIPPL_CONTROL=1 RIPPL_DEBUG=1 RIPPL_SLEEP_AFTER=8 ./target/release/rippl \
    https://example.com https://en.wikipedia.org/wiki/WebKit &
scripts/ctl.sh select 2     # tab 2 wakes: two web processes
# wait ~20 s: tab 1 sleeps, its process ends (pgrep WebKitWebProces)
scripts/ctl.sh select 1     # wakes from its saved state ("kept state = Some(…) bytes")
scripts/ctl.sh list
```

## Checking Shield

```sh
cargo run --release -p rippl-core --example shield_convert -- easylist.txt > out.json
```

This prints rule counts and conversion time. To compare Shield on and off, set `shield = off` in a
scratch config, load the same page, and compare `scripts/mem.sh` totals. See the numbers in
[phase 3](phases/phase-3-shield.md).
