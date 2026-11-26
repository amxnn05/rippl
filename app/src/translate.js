// Translate page, in rippl's world: the page's
// words, read and put back in the same text nodes and attributes, so no
// element is added or removed and Show original puts back what was there.
(() => {
  if (window.__ripplTranslate) return;
  const skip = 'script, style, noscript, code, pre, kbd, samp, textarea, input, select, [contenteditable=""], [contenteditable="true"], [translate="no"], .notranslate, svg, math';
  const ATTRS = ['title', 'alt', 'placeholder', 'aria-label'];
  let found = [];   // { node, attr?, original }
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    return r.bottom > -200 && r.top < innerHeight + 2000;
  };
  window.__ripplTranslate = {
    // The strings to translate, on-screen ones first.
    collect() {
      found = [];
      const near = [], far = [];
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
        acceptNode(n) {
          if (!n.nodeValue || !/\p{L}/u.test(n.nodeValue)) return NodeFilter.FILTER_REJECT;
          const p = n.parentElement;
          if (!p || p.closest(skip)) return NodeFilter.FILTER_REJECT;
          return NodeFilter.FILTER_ACCEPT;
        }
      });
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        (visible(n.parentElement) ? near : far).push({ node: n, original: n.nodeValue });
      }
      for (const el of document.querySelectorAll(ATTRS.map(a => '[' + a + ']').join(','))) {
        if (el.closest(skip)) continue;
        for (const a of ATTRS) {
          const v = el.getAttribute(a);
          if (v && /\p{L}/u.test(v)) far.push({ node: el, attr: a, original: v });
        }
      }
      found = near.concat(far).slice(0, 3000);
      return JSON.stringify(found.map(f => f.original));
    },
    // Translations for found[start..].
    put(start, list) {
      list.forEach((t, i) => {
        const f = found[start + i];
        if (!f || typeof t !== 'string') return;
        if (f.attr) f.node.setAttribute(f.attr, t); else f.node.nodeValue = t;
      });
      document.documentElement.dataset.ripplTranslated = '1';
      return true;
    },
    original() {
      for (const f of found) {
        if (f.attr) f.node.setAttribute(f.attr, f.original); else f.node.nodeValue = f.original;
      }
      delete document.documentElement.dataset.ripplTranslated;
      return true;
    },
  };
})();
