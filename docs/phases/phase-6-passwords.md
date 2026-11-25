# Phase 6: Passwords

**Status:** done (2026-11-25)

## Goal

Save sign-ins in the system keyring and offer them back, without ever:

- filling anything on its own,
- offering an account to the wrong site,
- saving a password that didn't work,
- keeping a secret in rippl's own files.

## Design

### Where passwords live (`keyring.rs`)

- In the **system keyring** through the Secret Service, via **libsecret**. On this machine that's
  GNOME Keyring; KeePassXC and KWallet's Secret Service mode work the same way. rippl writes no
  secret to disk itself.
- One item per account: schema `dev.rippl.Password`, attributes `origin` and `username`, label
  "rippl: bob on example.com". Keyring apps (Seahorse, "Passwords and Keys") show and manage them.
- Listing accounts reads only attributes, so the keyring isn't asked to unlock just because you
  clicked a box. A secret is read only to fill a box you picked, or to compare before asking to
  save.
- Why **libsecret** and not `oo7`: libsecret's async calls run on GLib's main loop, which rippl
  already runs. `oo7` would bring a second async runtime (tokio or async-std) and its own crypto
  into the browser. libsecret is also already on any desktop that has a keyring.
- `RIPPL_KEYRING=memory` swaps in an in-memory keyring for tests.

### The rules (`rippl_core::passwords`, 5 tests)

- An account belongs to an **exact origin**: scheme, host and port, with default ports dropped.
  `https://example.com` and `https://login.example.com` don't share; `http://` never sees an
  `https://` account.
- Saving only on **secure pages**: `https`, or `http` on this machine (`localhost`, `127.0.0.1`,
  `[::1]`) for development.
- A password box holding 4–8 digits is a one-time code, not a password, and is never offered for
  saving.
- Save, update or say nothing: nothing if the same password is already saved or the site is on the
  never list; *Update* if the username is saved with another password.

### Noticing a sign-in (`signin.rs`)

- A script in rippl's **isolated world**, injected into the top frame only, watches forms. When a
  form with a filled password box is sent (submit, Enter in the password box, or a button click), it
  reports the username box's value and the password **once**.
- The username box is one marked `autocomplete=username|email`, otherwise the last text-like box
  before the password box.
- The report goes to a message handler that exists only in that world, so a page can't fake one.
  rippl keeps it in memory, and it is **wiped when dropped** (`zeroize`).
- The sign-in counts as having **worked** when:
  - the tab loads a page with no password box (rippl asks the isolated world), or
  - on a single-page app, the password box disappears.
- If the page comes back with its password box (a wrong password), the capture is dropped and
  nothing is asked. After two minutes the offer lapses.
- Nothing is captured in a **private tab**, on an insecure page, or on a site in the never list.

### Asking

A bar over the page: *"Save the password for bob@mail.com on example.com in your keyring?"* with
**Never for this site**, **Not now**, **Save** (or **Update**). It shows only while its tab is in
front. "Never" is kept in `places.sqlite` (`never_save`) and can be undone on `rippl://passwords`.

### Filling: never on its own

- Clicking into a page's username or password box makes the script report the box's position.
  rippl lists the saved accounts for **that exact origin** (attributes only) and, if there are any,
  shows them in a small list under the box.
- Picking one:
  1. reads that password from the keyring;
  2. **checks again** that the page in front is still on that origin, since it may have navigated
     while the keyring answered;
  3. only then calls the fill function in the isolated world. That function uses the native value
     setter and sends `input`/`change` events, so frameworks (React and others) notice.
- Top frame only: a sign-in form inside an iframe isn't filled.
- Every copy of the password rippl makes for this (the quoted script argument and the script
  itself) is wiped when dropped.

### `rippl://passwords` (`Ctrl+Alt+P`)

It lists saved accounts by site with **Delete**, and the never list with **Offer again**. It never
shows a password; your keyring app does that. Its buttons need the per-launch token, like every
`rippl://` page.

## Tested

All with `RIPPL_KEYRING=memory` against local test pages, except the last row, which used the real
GNOME Keyring:

| Check | Result |
|---|---|
| Sign in, the next page has no password box | asked **Save**; saved |
| Same sign-in again | nothing asked |
| Same user, new password | asked **Update** |
| Wrong password (the page comes back with its form) | **nothing asked**, capture dropped |
| Single-page app (form removed, no page load) | asked **Save** |
| A 6-digit code typed in a password box | nothing asked |
| Click the username box | account list shown; picking fills username and password |
| Same page on another origin (`127.0.0.1` vs `localhost`) | **no accounts**; a forced fill puts nothing in the box |
| `rippl://passwords` | account and never list shown; the password is **not** in the page |
| **Real GNOME Keyring** | saved as "rippl: rippl-test-user on 127.0.0.1:8765"; `secret-tool lookup` returned the exact password; deleted through rippl; 0 items left |
| Memory, one page open | rippl ~122 MB, total ~346 MB (unchanged) |
| Binary | 1.1 MB |
| Tests | 55, clippy clean |

## Known limits

- Accounts aren't shared between subdomains (`example.com` vs `login.example.com`). This is
  stricter than other browsers, on purpose; it may be relaxed later for sites whose sign-in page
  lives on another subdomain.
- Sign-in forms inside iframes aren't filled or saved.
- Multi-step sign-ins (username on one page, password on the next) save the password with whatever
  username is on the password page, which may be empty.
- No import of passwords from other browsers yet. Chrome's are encrypted with a key in the
  keyring; Firefox's need its NSS key.
- No passkeys or WebAuthn UI beyond what WebKitGTK provides.
