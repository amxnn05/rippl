// Elastic edges (motion.rs): scrolled past its top or bottom, the page gives
// a little, the further the harder, and springs back when the wheel rests,
// as pages do on a Mac. Only the page's own scroll, never a box inside it
// that scrolls by itself; nothing while the wheel zooms (Ctrl) or goes
// sideways. In rippl's world: the page's scripts never see it.
(() => {
  if (window.__ripplElastic || window.top !== window) return;
  window.__ripplElastic = true;
  const root = document.documentElement;
  const MOST = 96;
  let pull = 0, rest = 0, back = 0;
  const scroller = () => document.scrollingElement || root;
  const atTop = () => scroller().scrollTop <= 0;
  const atBottom = () => { const s = scroller(); return Math.ceil(s.scrollTop + innerHeight) >= s.scrollHeight - 1; };
  const inner = (el) => {
    for (; el && el !== document.body && el !== root; el = el.parentElement) {
      const s = getComputedStyle(el);
      if (/(auto|scroll|overlay)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 1) return true;
    }
    return false;
  };
  const place = (y, spring) => {
    root.style.transition = spring ? 'transform 460ms cubic-bezier(0.2, 1.35, 0.4, 1)' : 'none';
    root.style.transform = y ? 'translateY(' + y.toFixed(1) + 'px)' : 'translateY(0)';
  };
  const release = () => {
    if (!pull) return;
    pull = 0;
    place(0, true);
    clearTimeout(back);
    // Once it has settled, the page has no transform of rippl's left on it.
    back = setTimeout(() => { if (!pull) { root.style.transition = ''; root.style.transform = ''; } }, 480);
  };
  addEventListener('wheel', (e) => {
    if (e.ctrlKey || e.defaultPrevented || Math.abs(e.deltaX) > Math.abs(e.deltaY) || inner(e.target)) return;
    const dy = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaMode === 2 ? e.deltaY * innerHeight : e.deltaY;
    if ((dy < 0 && atTop()) || (dy > 0 && atBottom() && !atTop())) {
      // Each step gives less as it goes: a rubber band, not a slide.
      pull -= dy * 0.32 * (1 - Math.abs(pull) / MOST);
      pull = Math.max(-MOST, Math.min(MOST, pull));
      place(pull, false);
      clearTimeout(rest);
      rest = setTimeout(release, 110);
    } else if (pull) {
      release();
    }
  }, { passive: true, capture: true });
  addEventListener('keydown', release, true);
  addEventListener('mousedown', release, true);
})();
