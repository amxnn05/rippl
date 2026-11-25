//! Reading mode (F9 or Ctrl+Alt+R): the article on its own, in a calm
//! layout, and back again with the same key.
//!
//! The extractor runs in rippl's isolated script world ("rippl"), which
//! the page's own scripts can't see or touch. It finds the article and
//! rebuilds it element by element into a fresh tree, keeping only plain
//! tags (paragraphs, headings, lists, quotes, code, tables, images,
//! links), only `href`/`src`/`alt`/`title` attributes, and only http(s)
//! links and images. The reader page then forbids every script with its
//! Content-Security-Policy (`rippl_core::pages::reader`).
//!
//! It is shown with `load_alternate_html` under the page's own address,
//! so the field still says where you are; leaving reloads the page.

use rippl_core::tabs::TabId;
use webkit6::prelude::*;
use webkit6::{gio, WebView};

use crate::browser::Browser;

/// Returns JSON `{title, byline, site, html}`, or `null` when the page
/// doesn't look like an article.
const EXTRACT: &str = r#"
(() => {
  const SKIP = /comment|share|social|related|promo|sidebar|footer|newsletter|subscribe|advert|banner|cookie|popup|modal|nav|menu|breadcrumb|recommend|sponsor|infobox|navbox|metadata|editsection|toc|hatnote|catlinks|ambox|noprint|mw-jump|reflist|references|shortdescription/i;
  const KEEP = new Set(["P","H1","H2","H3","H4","H5","H6","UL","OL","LI","BLOCKQUOTE","PRE","CODE","EM","STRONG","B","I","A","IMG","FIGURE","FIGCAPTION","BR","HR","TABLE","THEAD","TBODY","TR","TD","TH","SUP","SUB","DL","DT","DD","SPAN","DIV","SECTION","ARTICLE","PICTURE","MAIN"]);
  const DROP = new Set(["SCRIPT","STYLE","NOSCRIPT","IFRAME","FORM","BUTTON","INPUT","SELECT","TEXTAREA","NAV","ASIDE","FOOTER","HEADER","SVG","CANVAS","VIDEO","AUDIO","OBJECT","EMBED","TEMPLATE","DIALOG"]);
  const text = el => (el.innerText || el.textContent || "").trim();
  const safeUrl = u => { try { const x = new URL(u, document.baseURI); return /^https?:$/.test(x.protocol) ? x.href : null; } catch (e) { return null; } };

  // The article: an <article> with enough text, or the element whose
  // direct paragraphs add up to the most text.
  let best = null;
  for (const a of document.querySelectorAll("article, [itemprop=articleBody], main")) {
    if (text(a).length > 600 && (!best || text(a).length > text(best).length)) best = a;
  }
  if (!best) {
    const score = new Map();
    for (const p of document.querySelectorAll("p, pre, blockquote")) {
      const len = text(p).length;
      if (len < 40) continue;
      const add = 1 + Math.min(len / 100, 3);
      const parent = p.parentElement, grand = parent && parent.parentElement;
      if (parent) score.set(parent, (score.get(parent) || 0) + add);
      if (grand) score.set(grand, (score.get(grand) || 0) + add / 2);
    }
    let top = 0;
    for (const [el, s] of score) if (s > top) { top = s; best = el; }
  }
  if (!best || text(best).length < 400) return null;

  // Rebuilt, not copied: nothing of the page's markup comes along that
  // isn't on the lists above.
  const out = document.implementation.createHTMLDocument("");
  const rebuild = (node) => {
    if (node.nodeType === 3) return out.createTextNode(node.nodeValue);
    if (node.nodeType !== 1) return null;
    const tag = node.tagName;
    if (DROP.has(tag)) return null;
    const label = (node.id || "") + " " + (typeof node.className === "string" ? node.className : "");
    if (node !== best && SKIP.test(label) && text(node).length < 2000) return null;
    if (node.getAttribute("aria-hidden") === "true" || node.hidden) return null;
    const keep = KEEP.has(tag);
    const el = keep ? out.createElement(tag === "MAIN" || tag === "ARTICLE" || tag === "SECTION" ? "div" : tag) : out.createDocumentFragment();
    if (tag === "A") { const h = safeUrl(node.getAttribute("href") || ""); if (h) el.setAttribute("href", h); }
    if (tag === "IMG") {
      const src = safeUrl(node.currentSrc || node.getAttribute("src") || node.getAttribute("data-src") || "");
      if (!src) return null;
      el.setAttribute("src", src);
      if (node.alt) el.setAttribute("alt", node.alt);
      return el;
    }
    if (node.title && keep) el.setAttribute("title", node.title);
    for (const child of node.childNodes) { const c = rebuild(child); if (c) el.appendChild(c); }
    return el;
  };
  const body = out.createElement("div");
  const built = rebuild(best);
  if (built) body.appendChild(built);
  // The page's own title heading would repeat the one reader mode shows.
  const h1 = body.querySelector("h1");
  const meta = n => { const m = document.querySelector(`meta[property="${n}"], meta[name="${n}"]`); return m ? m.content : ""; };
  const title = meta("og:title") || (h1 && text(h1)) || document.title;
  if (h1 && text(h1) === title) h1.remove();
  return JSON.stringify({
    title, byline: meta("author") || meta("article:author") || "",
    site: meta("og:site_name") || location.hostname.replace(/^www\./, ""),
    html: body.innerHTML,
  });
})()
"#;

impl Browser {
    /// F9: reading mode on for the page in front, or off again.
    pub(crate) fn toggle_reader(&self) {
        let Some(id) = self.front() else { return };
        let Some(view) = self.front_view() else { return };
        if self.reading.borrow_mut().remove(&id).is_some() {
            view.reload();
            return;
        }
        let url = view.uri().map(|u| u.to_string()).unwrap_or_default();
        if !url.starts_with("http") {
            return;
        }
        let weak = self.weak();
        view.evaluate_javascript(EXTRACT, Some("rippl"), None, None::<&gio::Cancellable>, move |result| {
            let Some(b) = weak.upgrade() else { return };
            let json = match result {
                Ok(v) if v.is_string() => v.to_str().to_string(),
                Ok(_) => {
                    b.notice("This page doesn't look like an article");
                    return;
                }
                Err(e) => {
                    eprintln!("rippl: reading mode: {e}");
                    b.notice("Reading mode isn't available on this page");
                    return;
                }
            };
            b.show_reader(id, &url, &json);
        });
    }

    fn show_reader(&self, id: TabId, url: &str, json: &str) {
        let Ok(article) = serde_json::from_str::<serde_json::Value>(json) else { return };
        let field = |k: &str| article.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let html = rippl_core::pages::reader(&field("title"), &field("byline"), &field("site"), url, &field("html"));
        let Some(view) = self.views.borrow().get(&id).cloned() else { return };
        // Still the same page? (It may have navigated while extracting.)
        if view.uri().as_deref() != Some(url) {
            return;
        }
        self.reading.borrow_mut().insert(id, url.to_string());
        view.load_alternate_html(&html, url, Some(url));
    }

    /// Reading mode ends when the tab goes somewhere else.
    pub(crate) fn reader_followed(&self, id: TabId, view: &WebView) {
        let now = view.uri().map(|u| u.to_string());
        let mut reading = self.reading.borrow_mut();
        if reading.get(&id).is_some_and(|u| Some(u) != now.as_ref()) {
            reading.remove(&id);
        }
    }
}
