// Put into every page rippl shows (macOS and Windows). The page can see
// it, so it only reports what the page already shows (its icon, the keys
// pressed for rippl's own shortcuts) and does what rippl asks (find).
(() => {
  if (window.__rippl) return;
  const post = (m) => { try { window.ipc.postMessage(JSON.stringify(m)); } catch (_) {} };
  let keys = __RIPPL_KEYS__;
  const named = { Enter: "Return", ArrowLeft: "Left", ArrowRight: "Right", ArrowUp: "Up", ArrowDown: "Down",
    PageUp: "Page_Up", PageDown: "Page_Down", Backspace: "BackSpace", " ": "space", "+": "plus", "-": "minus",
    "=": "equal", ",": "comma", ".": "period", "/": "slash", "[": "bracketleft", "]": "bracketright" };
  const keyName = (e) => {
    if (named[e.key]) return named[e.key];
    if (/^Digit\d$/.test(e.code) && (e.ctrlKey || e.altKey || e.metaKey)) return e.code.slice(5);
    return e.key.length === 1 ? e.key.toLowerCase() : e.key;
  };
  addEventListener("keydown", (e) => {
    const name = keyName(e);
    // Keys typed into the page itself stay the page's, unless they hold a
    // modifier (Ctrl+T in a text box is still a new tab).
    const plain = !e.ctrlKey && !e.altKey && !e.metaKey;
    const t = e.target;
    if (plain && t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) && !/^F\d+$/.test(name) && name !== "Escape") return;
    for (const k of keys) {
      const shiftOk = k.shift === e.shiftKey || k.key === "plus";
      const nameOk = k.key.toLowerCase() === name.toLowerCase() || (k.key === "ISO_Left_Tab" && name === "Tab" && e.shiftKey);
      if (nameOk && shiftOk && k.ctrl === e.ctrlKey && k.alt === e.altKey && k.meta === e.metaKey) {
        if (k.name === "escape" && document.fullscreenElement) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        post({ t: "cmd", name: k.name, repeat: e.repeat });
        return;
      }
    }
  }, true);
  addEventListener("keyup", (e) => { if (e.key === "Control" || e.key === "Meta") post({ t: "mod-up" }); }, true);

  // The site's icon, as the page names it.
  const icon = () => {
    if (window.top !== window) return;
    const links = [...document.querySelectorAll('link[rel~="icon"], link[rel="shortcut icon"], link[rel="apple-touch-icon"]')];
    const best = links.sort((a, b) => (parseInt(b.sizes?.value) || 0) - (parseInt(a.sizes?.value) || 0))
      .find((l) => /^(https?:|data:image)/.test(l.href));
    const href = best ? best.href : (/^https?:$/.test(location.protocol) ? location.origin + "/favicon.ico" : "");
    if (href) post({ t: "icon", href });
  };
  addEventListener("DOMContentLoaded", icon);
  addEventListener("load", icon);

  // A link's address while the pointer is over it.
  let over = null;
  addEventListener("mouseover", (e) => {
    const a = e.target.closest && e.target.closest("a[href]");
    if (a === over) return;
    over = a;
    if (window.top === window) post({ t: "hover", url: a ? a.href : "" });
  }, true);

  // Shift+click: the link's page in a panel over this one (peek); middle
  // click and Ctrl/⌘+click: a new tab behind this one.
  addEventListener("click", (e) => {
    const a = e.target.closest && e.target.closest("a[href]");
    if (!a || !/^https?:/.test(a.href)) return;
    if (e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey && window.__ripplPeek) {
      e.preventDefault(); e.stopImmediatePropagation();
      post({ t: "peek", url: a.href });
    } else if (e.ctrlKey || e.metaKey) {
      e.preventDefault(); e.stopImmediatePropagation();
      post({ t: "open-behind", url: a.href });
    }
  }, true);
  addEventListener("auxclick", (e) => {
    const a = e.target.closest && e.target.closest("a[href]");
    if (e.button === 1 && a && /^https?:/.test(a.href)) {
      e.preventDefault(); e.stopImmediatePropagation();
      post({ t: "open-behind", url: a.href });
    }
  }, true);

  // Find in page.
  const count = (text) => {
    if (!text) return 0;
    const body = document.body ? document.body.innerText.toLowerCase() : "";
    const needle = text.toLowerCase();
    let n = 0, i = 0;
    while ((i = body.indexOf(needle, i)) !== -1 && n < 1000) { n++; i += needle.length; }
    return n;
  };
  window.__rippl = {
    keys(k) { keys = k; },
    find(text, back) {
      if (!text) { getSelection().removeAllRanges(); post({ t: "found", text, count: 0, hit: false }); return; }
      const hit = window.find(text, false, back, true, false, false, false);
      post({ t: "found", text, count: count(text), hit });
    },
    findClear() { getSelection().removeAllRanges(); },
    media() {
      const m = [...document.querySelectorAll("video, audio")].find((v) => !v.paused && !v.muted);
      return !!m;
    },
  };
  // What plays: rippl keeps a tab that plays awake, and shows it at the foot.
  const playing = () => {
    if (window.top !== window) return;
    const on = [...document.querySelectorAll("video, audio")].some((v) => !v.paused && !v.ended && !v.muted && v.currentTime > 0);
    post({ t: "audio", on, title: document.title });
  };
  addEventListener("play", playing, true);
  addEventListener("pause", playing, true);
  addEventListener("ended", playing, true);
  addEventListener("volumechange", playing, true);
})();
