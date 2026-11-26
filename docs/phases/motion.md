# Motion: springs, fades, elastic edges

**Status:** done (2026-11-26).

the Motion, in GTK's CSS and a little script (motion.rs, elastic.js):

| the | rippl's | Used for |
|---|---|---|
| `glide`: a spring, 0.34 s, damping 0.82 | `cubic-bezier(0.2, 1.3, 0.4, 1)`, 240–340 ms (a little overshoot) | what comes onto the page: popovers, peek, the Ctrl+Tab cards |
| `settle`: a spring, 0.30 s, damping 0.86 | `cubic-bezier(0.25, 1.15, 0.4, 1)`, 300 ms | what slides into place: bars, rows, the AI panel, the new tab's field |
| `quick`: ease-out 0.14 s | `ease-out 140ms` transitions | hover, colours, the chosen row |
| `give`: 0.97 | `transform: scale(0.97)` while pressed | buttons, tabs, pins, rail rows, cards |
| `fade`: 0.2 s | `rippl-fade 200ms` | dims, the link status |

What moves:

- **Popovers and menus** (the site card, tab menu, accounts, pin folders, spaces, the bookmarks
  bar's menus) pop in.
- **Bars** (find, a page's question, save password, blocked pop-up) drop in; a **notice** rises,
  then sinks and fades as it goes.
- **Peek** and the **Ctrl+Tab cards** scale up over a fading dim; the chosen card grows a little;
  the **AI panel** slides in from the right.
- A **new tab** slides into the column (or the strip on top); one closed with the mouse (×, middle
  click) folds away first. Ctrl+W stays immediate: the keyboard shouldn't wait.
- Going from tab to tab **cross-fades** (120 ms); **Ctrl+S** slides the column away and back.
- A **loading tab** shows a thin ring turning where its × comes (loader.rs), the site's icon
  staying on the left; under the pointer the ring fades and the × is there to close it. In the
  column and the strip on top; on pins and in the icon-only column, which have no ×, the ring
  takes the icon's place; a faint track, a third of a turn in the tab's own ink, one turn per
  0.8 s off the frame clock, so every loading tab turns together and a redrawn column doesn't
  make it jump. It goes with the loading, and costs frames only then.
- The **new tab's field** rises into place; its offers pop.
- **Elastic edges**: scrolled past its top or bottom, a page gives (a rubber band: each step less,
  96 px at most) and springs back when the wheel rests; never a box inside the page that scrolls
  by itself, nor while zooming. In rippl's own script world; the page's transform is cleared once
  it settles. `elastic = off` turns it off alone.
- rippl's own pages: Settings' sections slide in, rows answer the pointer, buttons give; the
  welcome page's parts rise one after another. They follow `prefers-reduced-motion`.

How: GTK starts a CSS animation when a widget's style changes, not when a widget shows again. So
what comes and goes carries the class `enter` while it is mapped (`motion::enters`): each showing
is a style change, and its animation plays every time. (The first try, animations on the plain
classes, played once and never again: caught by capturing frames of peek opening.) New tab rows
are new widgets; a row is marked `fresh` only the first time its tab is drawn, so redrawing the
column doesn't replay anything.

`motion = reduced` makes everything immediate: GTK's own animations off
(`gtk-enable-animations`), no cross-fade, no slide, no elastic edges. Nothing animates while
nothing moves: these cost frames only while they play.

Tested: elastic edges pulled a page 45 px past its top and released it, and did nothing with
`motion = reduced`; peek's panel caught mid-fade in a frame capture; no CSS warnings from GTK.
