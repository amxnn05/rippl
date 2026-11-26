# Feature batch 6: Settings has it, the AI add-on, Translate page, agents over MCP

**Status:** done (2026-11-26). Plan: [features.md](../features.md).

## Settings, laid out as the

rippl://settings (Ctrl+, or the gear) is now the Settings tab:

- **The rail** down the left, the page's whole height (196 wide, a faint wash, a hairline beside
  it): "Settings", a **Find a setting** field (Ctrl+F goes to it on this page; Esc empties it),
  then the sections in the three blocks: General, Appearance, Themes, Tabs, Browsing,
  Spaces & Pins · Passwords, Privacy, AI, Keyboard & Mouse · About. The chosen one sits on a
  white card. While finding, each section shows how many of its settings match.
- **The section** on the right, at most 600 wide: its icon and title, then groups (a small
  uppercase heading) of cards of rows. Each row has its icon in a soft tile, its words and a line
  on what it does, and its control: a switch for on/off, a menu for choices, a box for text.
- Every setting has its place (`rippl_core::pages::place`); the page also holds what isn't a
  setting: Shield's exceptions, hidden elements, site permissions, Clear history (Privacy), saved
  passwords and certificates (Passwords), how spaces and pins work (Spaces & Pins), version,
  engine, profile and data folder (About).
- **Keyboard & Mouse** lists every command with its keys as key caps. **Change** records a new
  key: press it (Esc stops). **+** adds another; **Reset** gives a command its own keys back. While
  it listens, rippl's own shortcuts step aside (they would take Ctrl+T before the page saw it)
  and come back when the key is saved or Esc is pressed.
- The other rippl pages (History, Bookmarks, Archive, Passwords) keep their links; Settings
   has none above it.

Tested: recording Ctrl+J for New tab wrote `shortcut.new-tab = <Ctrl>j`; "proxy" in Find a setting
found Privacy › Connection › Proxy.

## The AI add-on (`ai`)

the , , , with a model running on this computer:
Ollama, llama.cpp's `llama-server`, LM Studio, through the OpenAI-style chat API they all speak.
Off until `ai = on`; nothing is sent before you ask, and then only the page you asked about, to
`ai_endpoint` (Ollama's address by default: `http://127.0.0.1:11434/v1`), model `ai_model`.

- **Ask about this page** (Ctrl+Shift+A): a panel beside the page. **Summarize this page**, or a
  question in its box; the answer comes word by word (server-sent events). The page's text goes
  with it, 12,000 characters at most.
- If no model answers, the panel says so and how to start one (`ollama run <model>`).

## Translate page (Ctrl+Shift+L, `translate_to`)

the , with the same local model instead of Apple's: a script in rippl's world
reads the page's words (text and the title, alt, placeholder and aria-label attributes;
on-screen first; code, fields, editable areas and `translate="no"` left alone), sends them in
slices of 40 as a JSON array, and puts each translation back in the same text node or attribute
as it comes: nothing added or removed, so pages that keep their nodes keep working. A slice whose
answer doesn't have exactly as many strings is left as it was. Ctrl+Shift+L again shows the
original. 16 languages offered; any the model knows can be typed in the config.

Tested with a stand-in model server: a summary and a question streamed into the panel naming the
page; the page went to its "translation" (uppercase, from the stand-in) and back.

No model is installed on this machine, and none was downloaded.

## Agents over MCP (`agents`, `rippl --mcp`)

the  and search-mcp. `agents = on` (off by default; Settings › About ›
Developer) opens rippl's socket for an agent's commands alone; `rippl --mcp` is the MCP server an
agent runs (stdio, one JSON-RPC message a line): `claude mcp add rippl -- rippl --mcp`.

Tools: `rippl_tabs`, `rippl_open`, `rippl_navigate`, `rippl_read`, `rippl_click`, `rippl_type`,
`rippl_screenshot`, `rippl_close`. Every rule is checked in rippl, not in the MCP process:

- It works in **tabs of its own**, opened behind yours, never selected for you, marked ✦ in the
  column, and only there may it go, click and type.
- **Your tabs** it may list and read only with `agents_see_tabs = on`; never click or type in
  them; never private tabs or rippl's own pages.
- Only http and https. No script of its own: clicking and typing are rippl's few lines in its own
  world. **Password and card fields** are refused.
- Clicking and typing wait for a page still loading, and for the element (5 s), as a person would:
  a script run in the first, blank document of a new tab was cut off when the page replaced it.
- Pictures only of a page on screen (WebKit draws only what is shown); a tab behind says so.
- A notice says an agent is at work, once a minute at most; `agents = off` closes the door.

Tested by driving `rippl --mcp` as an agent would: initialize, the 8 tools listed, a form opened,
typed in and submitted (three times out of three), the password box refused, the page read; your
tab refused for reading and clicking; `file://` refused; a PNG of the tab once on screen.

## Tests

96 (91 core, 5 app). New: the AI requests and answers (streamed pieces, whole answers,
translations that must match), key names as people write them, every setting having a place, and
the MCP server's messages.
