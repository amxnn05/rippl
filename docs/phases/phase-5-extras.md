# Phase 5: Extras

**Status:** done (2026-11-25)

## Goal

The things that make a minimal browser pleasant to live in: pinned tabs, reading mode, getting rid
of clutter for good, pages for your history, bookmarks and settings, and bringing your bookmarks
over from the browser you're leaving.

**Moved out of this phase:** passwords now have a phase of their own (Phase 6). Detecting sign-in
forms, storing secrets in the system keyring and filling them safely is the most
security-sensitive feature a browser has, and it gets focused work rather than a slot in a list.
Split view moved to Phase 7.

## What was built

### Pinned tabs (`Ctrl+Shift+P`)

- Pinned tabs sit together at the top of the column, in the order they were pinned, with a pin
  icon, bold titles and a line under the group (`Tabs::set_pinned`).
- A tab opened from a pinned tab goes below the pins, never inside the group (`Tabs::open`, tested).
- Pins never sleep (Phase 2) and come back pinned after a restart (session file).

### Reading mode (`F9`, `Ctrl+Alt+R`)

- An extractor runs in rippl's **isolated script world** (`"rippl"`), so the page's own scripts
  can't see or change it. It finds the article: an `<article>`/`main`/`articleBody` with enough
  text, or else the element whose paragraphs add up to the most text.
- It **rebuilds** the article into a fresh document rather than copying it:
  - only plain tags are kept (paragraphs, headings, lists, quotes, code, tables, figures, images,
    links);
  - only `href`, `src`, `alt` and `title` attributes;
  - only `http(s)` URLs;
  - comments, share bars, related links, newsletter boxes, infoboxes, navigation boxes, edit links
    and reference lists are dropped.
- The reader page (`rippl_core::pages::reader`) has `default-src 'none'`, so **no script can run in
  it** even if something slipped through. A serif column 680 px wide, light or dark with the
  system, starting at the top.
- It is shown with `load_alternate_html` under the page's own address. `F9` again reloads the
  page. Following a link leaves reading mode. A reading-mode view isn't recorded as another visit.

### Hide anything, for good (`Ctrl+Shift+H`, `Ctrl+Shift+U`)

The same idea Curtain:

- `Ctrl+Shift+H` starts a picker (in the isolated world) that outlines what is under the pointer.
  A click hides **what is under the click** and reports a CSS selector that matches exactly that
  element: a stable `id` if there is one, otherwise classes and `:nth-of-type` steps up to the
  nearest id, checked to be unique. `Esc` cancels.
- The report comes back through a script message handler registered **only in rippl's world**, so
  a page can't send fake reports.
- Selectors are kept per site in `places.sqlite` (`hidden` table). Each site gets one user style
  sheet, limited to that site and its subdomains, with one `display: none !important` rule per
  selector, so one bad selector only loses itself. WebKit applies these as the page loads, so
  nothing flashes.
- `Ctrl+Shift+U` shows everything on the site again; `rippl://settings` lists and clears them too.
  Nothing is kept from a private tab.

### rippl's own pages: `rippl://history`, `rippl://bookmarks`, `rippl://settings`

- `Ctrl+H` opens History (search, forget one page, clear everything). `Ctrl+Shift+O` opens
  Bookmarks (remove one, import from another browser). `Ctrl+,` opens Settings: site permissions
  (forget), sites Shield is off for (turn it back on), sites with hidden elements (show again), and
  the current configuration. If one of these pages is already open in a tab, that tab is reused.
- HTML is built in `rippl_core::pages`, with **everything from outside escaped** (tested with
  hostile titles and URLs).
- **Security:**
  - The `rippl` scheme is registered *local*, *display-isolated* and *secure*. A website can't
    link to it, load it, frame it or read it.
  - Buttons are navigations to `rippl://<page>?do=…&arg=…&token=…`. The token is 128 random bits
    from `/dev/urandom`, new at every launch, and only appears inside these pages. Without it,
    nothing happens and rippl logs the refusal.
  - Each page's Content-Security-Policy allows only its own inline script and style.

### Bring bookmarks over (`rippl_core::import`)

- rippl finds the profiles of Chrome, Chromium, Brave, Edge and Vivaldi (their `Bookmarks` JSON
  file), and of Firefox, LibreWolf and Zen (`places.sqlite`, in `~/.mozilla/firefox` and
  `~/.config/mozilla/firefox`).
- Firefox locks its database while running, so it's **copied** (with its `-wal`) to rippl's cache,
  read there, and the copy is deleted. The other browser is never written to.
- Duplicates and non-web addresses (`javascript:`, Firefox `place:` queries) are skipped. Folders are
  flattened for now.

### Smaller things

- A **notice** bubble over the bottom of the page for short messages ("Hidden on example.com ·
  Ctrl+Shift+U shows it again"), gone after 3 s.
- Control socket: `js <code>` runs script in the front page and answers with JSON, which is how the
  tests below read pages. It can read any open page, which is one more reason the socket is off
  unless `RIPPL_CONTROL=1`.

## Tested (control socket, scratch profile, a fake `HOME` with fake Chrome and Firefox profiles)

| Check | Result |
|---|---|
| Pin the third tab | moved to the top, pin icon, separator |
| `hide #cookie-banner` | `display: none`, still after reload |
| Picker, click on the promo box | hid `div.promo.box:nth-of-type(2)`, not its neighbour; still hidden after reload |
| `Ctrl+Shift+U` | shown again; nothing kept |
| Reading mode on Wikipedia, scrolled to 3000 px | article title, **0 scripts**, ~10,500 words, starts at the top, opens with the real first paragraph, infobox gone |
| `F9` again | full page back |
| `rippl://history?do=clear&token=0000` (wrong token) | **refused**, history untouched (8 rows) |
| A website setting `location.href = "rippl://history?do=clear"` | navigation blocked, still on the website |
| A website framing `rippl://settings` and reading it | blocked |
| The page's own Forget button | 8 → 7 rows, address cleaned back to `rippl://history` |
| Import from Chrome (3 bookmarks, one in a folder) | 3 new |
| Import from Firefox (2, one a duplicate) | 1 new |
| Memory, one page open | rippl ~121 MB, total ~344 MB (unchanged) |
| Binary | 1.0 MB (up from 0.86: `serde_json` for import and page scripts) |
| Tests | 48 (45 core, 3 app), clippy clean |

## Bugs found and fixed

- **The picker hid the wrong element.** It hid what was last *hovered*, and the pointer can move
  between hovering and clicking; in the test, a real mouse over the window made it hide a
  paragraph instead of the promo box. It now hides what is under the click itself.
- **Reading mode opened mid-page and started with Wikipedia's infobox.** The view kept the page's
  scroll position, and the extractor kept infoboxes and navigation boxes. It now scrolls to the top
  and drops those.
- **Build:** the picker's script contains `"#`, which ends a Rust `r#"…"#` string; it uses
  `r##"…"##`.

## Known limits

- The extractor is a compact heuristic, not Mozilla's Readability. Unusual layouts may pick the
  wrong block, or a short page may be taken for an article.
- Selectors with `:nth-of-type` can stop matching if a site reorders its page; hide the element
  again if it comes back.
- Imported bookmarks lose their folders (bookmarks are a flat list).
- Settings shows the configuration but doesn't edit it yet; edit the file and restart.
