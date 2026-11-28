// The column, the strip above the page and the new tab, drawn from the
// state rippl sends (`rippl.update`). Everything it does goes back as a
// message (`send`); it decides nothing on its own.
"use strict";
(() => {
const $ = (id) => document.getElementById(id);
const send = (m) => window.ipc.postMessage(JSON.stringify(m));
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

const ICON = {
  back: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M10 3 5 8l5 5"/></svg>',
  forward: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m6 3 5 5-5 5"/></svg>',
  reload: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M13 8a5 5 0 1 1-1.5-3.6"/><path d="M13 2.5v3h-3"/></svg>',
  stop: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="m4 4 8 8M12 4l-8 8"/></svg>',
  info: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><circle cx="8" cy="8" r="5.8"/><path d="M8 7.2v4M8 4.9v.1"/></svg>',
  settings: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><path d="M2.5 4.5h5M10.5 4.5h3M2.5 11.5h2M7.5 11.5h6"/><circle cx="9" cy="4.5" r="1.5"/><circle cx="6" cy="11.5" r="1.5"/></svg>',
  star: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><path d="m8 2.2 1.8 3.7 4 .6-2.9 2.8.7 4L8 11.4l-3.6 1.9.7-4-2.9-2.8 4-.6z"/></svg>',
  starred: '<svg viewBox="0 0 16 16" fill="currentColor" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><path d="m8 2.2 1.8 3.7 4 .6-2.9 2.8.7 4L8 11.4l-3.6 1.9.7-4-2.9-2.8 4-.6z"/></svg>',
  close: '<svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="m4 4 8 8M12 4l-8 8"/></svg>',
  folder: '<svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"><path d="M2.5 4.5h4l1.2 1.5h5.8v6.5h-11z"/></svg>',
};

let state = { tabs: [], groups: [], spaces: [], keys: [] };
let offers = [], chosen = -1, typing = null, homeTyping = false;

$("back").innerHTML = ICON.back;
$("forward").innerHTML = ICON.forward;
$("reload").innerHTML = ICON.reload;
$("site-card").innerHTML = ICON.info;
$("settings").innerHTML = ICON.settings;

// ---- keys ---------------------------------------------------------------
// rippl's own shortcuts, for when the column has the keys (pages have the
// same listener, see page.js).
function keyName(e) {
  const k = e.key;
  const named = { Enter: "Return", ArrowLeft: "Left", ArrowRight: "Right", ArrowUp: "Up", ArrowDown: "Down",
    PageUp: "Page_Up", PageDown: "Page_Down", Backspace: "BackSpace", " ": "space", "+": "plus", "-": "minus",
    "=": "equal", ",": "comma", ".": "period", "/": "slash", "[": "bracketleft", "]": "bracketright" };
  if (named[k]) return named[k];
  if (k.length === 1) return k.toLowerCase();
  if (/^Digit\d$/.test(e.code) && (e.ctrlKey || e.altKey || e.metaKey)) return e.code.slice(5);
  return k;
}
function command(e) {
  const name = keyName(e);
  for (const k of state.keys || []) {
    const shiftOk = k.shift === e.shiftKey || k.key === "plus";
    const nameOk = k.key.toLowerCase() === name.toLowerCase() || (k.key === "ISO_Left_Tab" && name === "Tab" && e.shiftKey);
    if (nameOk && shiftOk && k.ctrl === e.ctrlKey && k.alt === e.altKey && k.meta === e.metaKey) return k.name;
  }
  return null;
}
addEventListener("keydown", (e) => {
  if (e.target === $("address") || e.target === $("home-address")) {
    if (["Down", "Up"].includes(keyName(e)) || e.key === "Escape" || e.key === "Enter") return;
  }
  const c = command(e);
  if (c) {
    e.preventDefault();
    e.stopPropagation();
    send({ t: "cmd", name: c, repeat: e.repeat });
  }
}, true);
addEventListener("keyup", (e) => {
  if (e.key === "Control" || e.key === "Meta") send({ t: "mod-up" });
});

// ---- the field ------------------------------------------------------------
function fieldInput(input, home) {
  input.addEventListener("focus", () => {
    if (!home) { input.value = state.field?.url || ""; }
    requestAnimationFrame(() => input.select());
    send({ t: "field-focus", on: true });
  });
  input.addEventListener("blur", () => {
    setTimeout(() => { hideOffers(); if (!home) showAddress(); }, 120);
    send({ t: "field-focus", on: false });
  });
  input.addEventListener("input", () => {
    typing = input.value; homeTyping = home;
    send({ t: "typed", text: input.value });
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!offers.length) return;
      chosen = (chosen + (e.key === "ArrowDown" ? 1 : -1) + offers.length + 1) % (offers.length + 1) - 1;
      if (chosen < -1) chosen = offers.length - 1;
      drawOffers();
      if (chosen >= 0) input.value = offers[chosen].fill ?? offers[chosen].url;
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const o = chosen >= 0 ? offers[chosen] : null;
      send({ t: "go", text: o ? (o.go ?? o.url) : input.value, private: false, background: e.altKey, new_tab: e.metaKey || e.ctrlKey });
      hideOffers();
      input.blur();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      if (offers.length) { hideOffers(); return; }
      input.value = home ? "" : state.field?.url || "";
      input.blur();
      send({ t: "escape" });
    }
  });
}
fieldInput($("address"), false);
fieldInput($("home-address"), true);

function showAddress() {
  const a = $("address");
  if (document.activeElement !== a) a.value = state.field?.pretty || "";
}
function hideOffers() {
  offers = []; chosen = -1;
  $("offers").hidden = true; $("home-offers").hidden = true;
}
function drawOffers() {
  const box = homeTyping ? $("home-offers") : $("offers");
  (homeTyping ? $("offers") : $("home-offers")).hidden = true;
  if (!offers.length) { box.hidden = true; return; }
  box.innerHTML = offers.map((o, i) =>
    `<div class="offer${i === chosen ? " chosen" : ""}" data-i="${i}"><span class="t">${esc(o.title || o.url)}</span>${o.title && o.url ? `<span class="u">${esc(o.pretty || o.url)}</span>` : ""}</div>`).join("");
  box.hidden = false;
}
for (const box of [$("offers"), $("home-offers")]) {
  box.addEventListener("mousedown", (e) => {
    const el = e.target.closest(".offer");
    if (!el) return;
    e.preventDefault();
    const o = offers[+el.dataset.i];
    send({ t: "go", text: o.go ?? o.url, new_tab: e.metaKey || e.ctrlKey });
    hideOffers();
    document.activeElement?.blur();
  });
}

// ---- buttons --------------------------------------------------------------
$("back").onclick = () => send({ t: "cmd", name: "back" });
$("forward").onclick = () => send({ t: "cmd", name: "forward" });
$("reload").onclick = () => send({ t: "cmd", name: state.field?.loading ? "stop" : "reload" });
$("site-card").onclick = (e) => send({ t: "site-card", x: e.clientX, y: e.clientY });
$("settings").onclick = () => send({ t: "cmd", name: "settings" });
$("new-tab").onclick = () => send({ t: "cmd", name: "new-tab" });
$("star").onclick = () => send({ t: "cmd", name: "bookmark" });
$("downloads").querySelector(".heading").onclick = () => send({ t: "cmd", name: "downloads" });
$("find-text").addEventListener("input", () => send({ t: "find", text: $("find-text").value, dir: 0 }));
$("find-text").addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); send({ t: "find", text: $("find-text").value, dir: e.shiftKey ? -1 : 1 }); }
  if (e.key === "Escape") { e.preventDefault(); send({ t: "find-close" }); }
});
$("find-next").onclick = () => send({ t: "find", text: $("find-text").value, dir: 1 });
$("find-prev").onclick = () => send({ t: "find", text: $("find-text").value, dir: -1 });
$("find-close").onclick = () => send({ t: "find-close" });

// Tabs: a click selects, a middle click closes, a right click asks rippl
// for the tab's menu (a native one).
function tabEvents(container) {
  container.addEventListener("mousedown", (e) => {
    const el = e.target.closest("[data-id]");
    if (!el || e.button !== 0) return;
    if (e.target.closest(".close")) return;
    send({ t: "select", id: +el.dataset.id, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey || e.ctrlKey });
  });
  container.addEventListener("click", (e) => {
    const close = e.target.closest(".close");
    if (close) {
      const el = close.closest("[data-id]");
      send({ t: "close", id: +el.dataset.id });
      return;
    }
    const head = e.target.closest("[data-group]");
    if (head) send({ t: "fold", group: +head.dataset.group });
  });
  container.addEventListener("auxclick", (e) => {
    const el = e.target.closest("[data-id]");
    if (el && e.button === 1) { e.preventDefault(); send({ t: "close", id: +el.dataset.id }); }
  });
  container.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    const el = e.target.closest("[data-id]");
    const head = e.target.closest("[data-group]");
    if (el) send({ t: "tab-menu", id: +el.dataset.id, x: e.clientX, y: e.clientY });
    else if (head) send({ t: "group-menu", group: +head.dataset.group, x: e.clientX, y: e.clientY });
  });
}
tabEvents($("tabs"));
tabEvents($("pins"));
tabEvents($("top-tabs"));

// Drag a tab onto another to move it there.
let dragged = null;
$("tabs").addEventListener("dragstart", (e) => { dragged = e.target.closest("[data-id]")?.dataset.id; });
$("tabs").addEventListener("dragover", (e) => { if (dragged) e.preventDefault(); });
$("tabs").addEventListener("drop", (e) => {
  e.preventDefault();
  const onto = e.target.closest("[data-id]")?.dataset.id;
  if (dragged && onto && onto !== dragged) send({ t: "move", id: +dragged, onto: +onto });
  dragged = null;
});

$("spaces").addEventListener("click", (e) => {
  const el = e.target.closest("[data-space]");
  if (!el) return;
  if (el.dataset.space === "new") send({ t: "cmd", name: "new-space" });
  else send({ t: "space", id: +el.dataset.space });
});
$("spaces").addEventListener("contextmenu", (e) => {
  e.preventDefault();
  const el = e.target.closest("[data-space]");
  if (el && el.dataset.space !== "new") send({ t: "space-menu", id: +el.dataset.space, x: e.clientX, y: e.clientY });
});
$("download-list").addEventListener("click", (e) => {
  const el = e.target.closest("[data-download]");
  if (!el) return;
  send({ t: e.target.closest(".close") ? "download-forget" : "download-open", id: +el.dataset.download });
});
$("home-grid").addEventListener("click", (e) => {
  const el = e.target.closest("[data-url]");
  if (el) send({ t: "go", text: el.dataset.url });
});
$("bookmarks-bar").addEventListener("click", (e) => {
  const el = e.target.closest("[data-url]");
  if (el) send({ t: "go", text: el.dataset.url, new_tab: e.metaKey || e.ctrlKey });
});
$("bar").addEventListener("click", (e) => {
  const el = e.target.closest("[data-answer]");
  if (el) send({ t: "bar", answer: el.dataset.answer });
});
$("player").addEventListener("click", (e) => {
  const el = e.target.closest("[data-act]");
  send({ t: "player", act: el ? el.dataset.act : "show" });
});

// The column's width: drag its edge.
$("column-edge").addEventListener("mousedown", (e) => {
  e.preventDefault();
  const move = (m) => {
    const w = Math.max(160, Math.min(420, m.clientX));
    document.documentElement.style.setProperty("--column", w + "px");
    reportPage();
  };
  const up = () => {
    removeEventListener("mousemove", move);
    removeEventListener("mouseup", up);
    send({ t: "column-width", w: parseInt(getComputedStyle(document.documentElement).getPropertyValue("--column")) });
  };
  addEventListener("mousemove", move);
  addEventListener("mouseup", up);
});
// The window's title bar is the top of the column (macOS).
$("drag-top").addEventListener("mousedown", (e) => { if (e.button === 0) send({ t: "drag-window" }); });
$("drag-top").addEventListener("dblclick", () => send({ t: "zoom-window" }));

// ---- where the page goes -----------------------------------------------
// The engine's view sits exactly on the card; rippl is told where it is.
let lastRect = "";
function reportPage() {
  const r = $("page").getBoundingClientRect();
  const rect = { x: r.left, y: r.top, w: r.width, h: r.height };
  const key = JSON.stringify(rect);
  if (key !== lastRect) { lastRect = key; send({ t: "page-rect", ...rect }); }
}
new ResizeObserver(reportPage).observe($("page"));
addEventListener("resize", reportPage);

// ---- drawing ------------------------------------------------------------
function mark(t) {
  if (t.icon && state.icons) return `<span class="mark has-icon"><img src="${esc(t.icon)}" alt="" onerror="this.parentNode.classList.remove('has-icon');this.replaceWith(document.createTextNode('${esc(t.letter)}'))"></span>`;
  return `<span class="mark">${esc(t.letter)}</span>`;
}
function tabRow(t) {
  const cls = ["tab", t.front && "front", t.asleep && "asleep", t.loading && "loading", t.private && "private",
    t.group != null && "in-group", t.split && "split"].filter(Boolean).join(" ");
  return `<div class="${cls}" data-id="${t.id}" draggable="true" title="${esc(t.title)}\n${esc(t.pretty)}">${mark(t)}<span class="title">${esc(t.title || t.pretty || "New tab")}</span><span class="ring"></span><button class="close" title="Close tab">${ICON.close}</button></div>`;
}
let seen = new Set();
function drawTabs() {
  const tabs = state.tabs || [];
  // Pins.
  $("pins").innerHTML = (state.pin_folders || []).map((f) => `<button class="pin pin-folder" title="${esc(f)}">${ICON.folder}</button>`).join("") +
    tabs.filter((t) => t.pinned).map((t) => {
      const cls = ["pin", t.front && "front", t.asleep && "asleep"].filter(Boolean).join(" ");
      const inner = t.icon && state.icons ? `<img src="${esc(t.icon)}" alt="" onerror="this.replaceWith(document.createTextNode('${esc(t.letter)}'))">` : esc(t.letter);
      return `<button class="${cls}" data-id="${t.id}" title="${esc(t.title)}">${inner}${t.unread ? `<span class="unread">${esc(t.unread)}</span>` : ""}</button>`;
    }).join("");
  // Rows, in groups.
  const groups = new Map((state.groups || []).map((g) => [g.id, g]));
  let html = "", lastGroup = null;
  const loose = tabs.filter((t) => !t.pinned);
  for (let i = 0; i < loose.length; i++) {
    const t = loose[i];
    if (t.group != null && t.group !== lastGroup) {
      const g = groups.get(t.group);
      if (g && g.site) {
        // One site's tabs as one row of tiles.
        const members = loose.filter((x) => x.group === t.group);
        html += `<div class="stack${members.some((m) => m.front) ? " front" : ""}">` + members.map((m) =>
          `<span class="tile${m.front ? " front" : ""}${m.asleep ? " asleep" : ""}" data-id="${m.id}" title="${esc(m.title)}">${mark(m)}</span>`).join("") + "</div>";
        while (i + 1 < loose.length && loose[i + 1].group === t.group) i++;
        lastGroup = t.group;
        continue;
      }
      if (g) html += `<div class="group-head${g.collapsed ? " collapsed" : ""}" data-group="${g.id}"><span class="fold">▾</span><span class="name">${esc(g.name)}</span></div>`;
    }
    lastGroup = t.group;
    if (t.group != null && groups.get(t.group)?.collapsed && !t.front) continue;
    html += tabRow(t);
  }
  const box = $("tabs");
  box.innerHTML = html;
  // New rows come in; nothing else moves.
  const now = new Set(loose.map((t) => t.id));
  if (seen.size) for (const el of box.querySelectorAll(".tab")) if (!seen.has(+el.dataset.id)) el.classList.add("enter");
  seen = now;
  // Tabs across the top.
  $("top-tabs").innerHTML = state.top_tabs ? tabs.map((t) =>
    `<div class="top-tab${t.front ? " front" : ""}${t.asleep ? " asleep" : ""}" data-id="${t.id}">${mark(t)}<span class="title">${esc(t.title || t.pretty || "New tab")}</span><button class="close">${ICON.close}</button></div>`).join("") : "";
  box.querySelector(".tab.front")?.scrollIntoView({ block: "nearest" });
}
function drawSpaces() {
  const spaces = state.spaces || [];
  $("spaces").innerHTML = (spaces.length > 1 ? spaces.map((s, i) =>
    `<button class="space${s.here ? " here" : ""}" data-space="${s.id}" title="${esc(s.name)} (Alt+${i + 1})" style="${s.colour ? `color:${esc(s.colour)}` : ""}">${esc((s.name || "?").slice(0, 1).toUpperCase())}</button>`).join("") : "") +
    `<button class="space add" data-space="new" title="New space">+</button>`;
}
function drawDownloads() {
  const list = state.downloads || [];
  $("downloads").hidden = !list.length;
  $("download-list").innerHTML = list.map((d) =>
    `<div class="download" data-download="${d.id}" title="${esc(d.name)}"><div class="what"><span class="name">${esc(d.name)}</span><span class="status">${esc(d.status)}</span>${d.done || d.failed ? "" : `<div class="bar"><div style="width:${Math.round((d.fraction || 0) * 100)}%"></div></div>`}</div><button class="close" title="Remove from the list">${ICON.close}</button></div>`).join("");
}
function drawField() {
  const f = state.field || {};
  showAddress();
  $("back").disabled = !f.can_back;
  $("forward").disabled = !f.can_forward;
  $("reload").innerHTML = f.loading ? ICON.stop : ICON.reload;
  $("reload").title = f.loading ? "Stop" : "Reload";
  $("star").innerHTML = f.starred ? ICON.starred : ICON.star;
  $("star").classList.toggle("on", !!f.starred);
  $("star").hidden = !f.url;
  $("shield-off").hidden = !f.shield_off;
  $("site-card").hidden = !f.url;
  progress(f.loading ? Math.max(f.progress || 0, 0.1) : 0);
}
let progressTimer = 0;
function progress(p) {
  const line = $("progress-line"), box = $("progress");
  clearTimeout(progressTimer);
  if (p > 0 && p < 1) { box.classList.add("on"); line.style.width = (p * 100) + "%"; }
  else if (box.classList.contains("on")) {
    line.style.width = "100%";
    progressTimer = setTimeout(() => { box.classList.remove("on"); setTimeout(() => (line.style.width = "0"), 240); }, 300);
  }
}
function drawHome() {
  const home = !!state.home;
  $("home").hidden = !home;
  const h = state.home_look || {};
  $("home-wall").style.background = h.background || "";
  $("home-grid").innerHTML = (state.home_grid || []).map((p) =>
    `<button class="tile home-tile" data-url="${esc(p.url)}" title="${esc(p.url)}"><span class="mark">${esc(p.letter)}</span>${esc(p.title)}</button>`).join("");
}
function drawBars() {
  const bb = state.bookmarks_bar;
  $("bookmarks-bar").hidden = !bb;
  if (bb) $("bookmarks-bar").innerHTML = bb.length ? bb.map((b) => `<button class="bar-item" data-url="${esc(b.url)}" title="${esc(b.url)}">${esc(b.title || b.url)}</button>`).join("")
    : `<span class="bar-hint">Bookmarks you add (Ctrl+D) show here</span>`;
  const bar = state.bar;
  $("bar").hidden = !bar;
  if (bar) $("bar").innerHTML = `<span class="text">${esc(bar.text)}</span>` + bar.answers.map((a, i) =>
    `<button class="button${i === bar.answers.length - 1 ? " primary" : ""}" data-answer="${esc(a.id)}">${esc(a.label)}</button>`).join("");
  const n = $("notice");
  if (state.notice) { n.textContent = state.notice; n.hidden = false; n.classList.remove("leaving"); }
  else if (!n.hidden && !n.classList.contains("leaving")) { n.classList.add("leaving"); setTimeout(() => { n.hidden = true; n.classList.remove("leaving"); }, 150); }
  const p = state.player;
  $("player").hidden = !p;
  if (p) $("player").innerHTML = `<span class="title">${esc(p.title)}</span><button class="knob" data-act="pause">${p.paused ? "▶" : "❚❚"}</button>`;
}
function drawRoot() {
  const r = document.documentElement.classList;
  r.toggle("dark", state.theme === "dark");
  r.toggle("macos", state.platform === "macos");
  r.toggle("windows", state.platform === "windows");
  r.toggle("top-tabs", !!state.top_tabs);
  r.toggle("no-column", !!state.top_tabs || !!state.column_hidden);
  r.toggle("icon-column", !!state.icon_column);
  r.toggle("reduced", !state.motion);
  $("top-strip").hidden = !state.top_tabs;
  if (state.column_width && !state.icon_column) document.documentElement.style.setProperty("--column", state.column_width + "px");
  else document.documentElement.style.removeProperty("--column");
}

window.rippl = {
  update(s) {
    state = s;
    drawRoot(); drawTabs(); drawSpaces(); drawDownloads(); drawField(); drawHome(); drawBars();
    const f = $("find");
    f.hidden = !s.find;
    if (s.find) $("find-count").textContent = s.find.count || "";
    requestAnimationFrame(reportPage);
  },
  offers(list, text) {
    // Only for what is still typed.
    const input = homeTyping ? $("home-address") : $("address");
    if (text !== input.value) return;
    offers = list; chosen = -1;
    drawOffers();
  },
  progress,
  focusField(text) {
    const input = state.home ? $("home-address") : $("address");
    if (text != null) { input.focus(); input.value = text; send({ t: "typed", text }); return; }
    input.focus();
    input.select();
  },
  find(open) {
    $("find").hidden = !open;
    if (open) { $("find-text").focus(); $("find-text").select(); }
  },
  findCount(text) { $("find-count").textContent = text; },
};
send({ t: "ready" });
})();
