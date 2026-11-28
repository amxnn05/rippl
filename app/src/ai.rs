//! The AI add-on and Translate page,
//! with a model running on this computer: Ollama, llama.cpp's
//! `llama-server`, LM Studio, anything with the OpenAI-style chat API.
//! Off until `ai = on`; nothing is sent before you ask, and then only the
//! page you asked about, to the address you set (`ai_endpoint`, this
//! computer's by default).
//!
//! - **Ask about this page** (Ctrl+Shift+A): a panel beside the page.
//!   Summarize, or type a question; the answer comes word by word.
//! - **Translate page** (Ctrl+Shift+L): the page's words, on-screen first,
//!   sent in slices of 40 and put back in the same text nodes and
//!   attributes as each comes; Ctrl+Shift+L again shows the original. Code,
//!   fields and what the page marks `translate="no"` stay as they are.

use std::cell::{Cell, RefCell};

use rippl_core::ai;
use webkit6::prelude::*;
use webkit6::{gio, glib, gtk, soup};

use crate::browser::Browser;

const TRANSLATE: &str = include_str!("translate.js");
/// Strings per translation request.
const SLICE: usize = 40;

/// The panel and what it is doing.
pub(crate) struct Panel {
    pub panel: gtk::Box,
    answer: gtk::TextView,
    ask: gtk::Entry,
    summarize: gtk::Button,
    close: gtk::Button,
    /// The answer coming in, to stop when another question starts.
    asking: RefCell<Option<gio::Cancellable>>,
    /// Pages being translated, by the tab's view: a new run cancels.
    pub translating: Cell<u32>,
}

impl Panel {
    pub(crate) fn new() -> Panel {
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.add_css_class("ai-panel");
        panel.set_size_request(340, -1);
        panel.set_visible(false);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = gtk::Label::builder().label("Ask about this page").xalign(0.0).hexpand(true).build();
        title.add_css_class("ai-title");
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.set_tooltip_text(Some("Close (Ctrl+Shift+A)"));
        head.append(&title);
        head.append(&close);
        let answer = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(4)
            .right_margin(4)
            .build();
        answer.add_css_class("ai-answer");
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&answer)
            .build();
        let summarize = gtk::Button::with_label("Summarize this page");
        summarize.add_css_class("ai-button");
        let ask = gtk::Entry::builder().placeholder_text("Ask about this page…").build();
        ask.add_css_class("ai-ask");
        panel.append(&head);
        panel.append(&summarize);
        panel.append(&scroll);
        panel.append(&ask);
        Panel {
            panel,
            answer,
            ask,
            summarize,
            close,
            asking: RefCell::new(None),
            translating: Cell::new(0),
        }
    }

    /// What the panel says now (for the control socket).
    pub(crate) fn answer_text(&self) -> String {
        let b = self.answer.buffer();
        b.text(&b.start_iter(), &b.end_iter(), false).to_string()
    }

    fn say(&self, text: &str) {
        self.answer.buffer().set_text(text);
    }

    fn add(&self, text: &str) {
        let buffer = self.answer.buffer();
        buffer.insert(&mut buffer.end_iter(), text);
    }
}

/// A POST of `body` to the model's chat API.
fn request(endpoint: &str, body: &str) -> Result<(soup::Session, soup::Message), String> {
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let session = soup::Session::new();
    session.set_timeout(300);
    if let Some(r) = crate::engine::proxy_resolver() {
        session.set_proxy_resolver(Some(&r));
    }
    let message = soup::Message::new("POST", &url).map_err(|e| format!("{url}: {e}"))?;
    message.set_request_body_from_bytes(Some("application/json"), Some(&glib::Bytes::from_owned(body.as_bytes().to_vec())));
    Ok((session, message))
}

impl Browser {
    pub(crate) fn wire_ai(&self) {
        let weak = self.weak();
        self.ai.close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.toggle_ai_panel();
            }
        });
        let weak = self.weak();
        self.ai.summarize.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.ask_page("");
            }
        });
        let weak = self.weak();
        self.ai.ask.connect_activate(move |e| {
            if let Some(b) = weak.upgrade() {
                let q = e.text().to_string();
                if !q.trim().is_empty() {
                    b.ask_page(&q);
                }
            }
        });
    }

    /// Whether the add-on is on; says how to turn it on if not.
    fn ai_ready(&self) -> bool {
        if self.config.borrow().ai {
            return true;
        }
        self.notice("The AI add-on is off: Settings › AI, with a model running on this computer");
        false
    }

    /// Ctrl+Shift+A: the panel, open or closed.
    pub(crate) fn toggle_ai_panel(&self) {
        if !self.ai.panel.is_visible() && !self.ai_ready() {
            return;
        }
        let show = !self.ai.panel.is_visible() || self.ai.panel.has_css_class("leaving");
        if show {
            Browser::arrive(&self.ai.panel);
        } else {
            self.leave(&self.ai.panel);
        }
        if show {
            if self.ai.answer.buffer().char_count() == 0 {
                let c = self.config.borrow();
                self.ai.say(&format!("Answers come from {} at {}, on what this page says.", c.ai_model, c.ai_endpoint));
            }
            self.ai.ask.grab_focus();
        } else if let Some(c) = self.ai.asking.borrow_mut().take() {
            c.cancel();
        }
    }

    /// Asks about the page in front (`question` empty: a summary).
    pub(crate) fn ask_page(&self, question: &str) {
        if !self.ai_ready() {
            return;
        }
        let Some(view) = self.front_view() else { return };
        if let Some(c) = self.ai.asking.borrow_mut().take() {
            c.cancel();
        }
        let cancel = gio::Cancellable::new();
        *self.ai.asking.borrow_mut() = Some(cancel.clone());
        Browser::arrive(&self.ai.panel);
        self.ai.say(if question.is_empty() { "Reading the page…" } else { "Thinking…" });
        let question = question.to_string();
        let weak = self.weak();
        view.evaluate_javascript(
            "document.body ? document.body.innerText : ''",
            Some("rippl"),
            None,
            None::<&gio::Cancellable>,
            move |r| {
                let Some(b) = weak.upgrade() else { return };
                let text = r.map(|v| v.to_str().to_string()).unwrap_or_default();
                let (title, url) = b.front_view().map_or((String::new(), String::new()), |v| {
                    (v.title().map(|t| t.to_string()).unwrap_or_default(), v.uri().map(|u| u.to_string()).unwrap_or_default())
                });
                let (endpoint, model) = {
                    let c = b.config.borrow();
                    (c.ai_endpoint.clone(), c.ai_model.clone())
                };
                let body = ai::chat_request(&model, &title, &url, &text, &question, true);
                glib::MainContext::default().spawn_local(async move {
                    let result = stream_answer(&endpoint, &body, &cancel, |piece, first| {
                        if first {
                            b.ai.say("");
                        }
                        b.ai.add(piece);
                    })
                    .await;
                    if let Err(e) = result
                        && !cancel.is_cancelled()
                    {
                        b.ai.say(&format!("No answer from {endpoint}: {e}\n\nIs a model running there? With Ollama: ollama run {model}"));
                    }
                });
            },
        );
    }

    /// Ctrl+Shift+L: the page in front translated, or back to its original.
    pub(crate) fn translate_page(&self) {
        if !self.ai_ready() {
            return;
        }
        let Some(view) = self.front_view() else { return };
        let run = self.ai.translating.get().wrapping_add(1);
        self.ai.translating.set(run);
        let weak = self.weak();
        let script = format!(
            "{TRANSLATE}; document.documentElement.dataset.ripplTranslated ? (window.__ripplTranslate.original(), '\"shown\"') : window.__ripplTranslate.collect()"
        );
        let v = view.clone();
        view.evaluate_javascript(&script, Some("rippl"), None, None::<&gio::Cancellable>, move |r| {
            let Some(b) = weak.upgrade() else { return };
            let collected = r.map(|v| v.to_str().to_string()).unwrap_or_default();
            if collected == "\"shown\"" {
                b.notice("The original page");
                return;
            }
            let strings: Vec<String> = serde_json::from_str(&collected).unwrap_or_default();
            if strings.is_empty() {
                b.notice("Nothing to translate here");
                return;
            }
            let (endpoint, model, language) = {
                let c = b.config.borrow();
                (c.ai_endpoint.clone(), c.ai_model.clone(), c.translate_to.clone())
            };
            b.notice(&format!("Translating into {language}… (Ctrl+Shift+L shows the original)"));
            glib::MainContext::default().spawn_local(async move {
                let mut failed = 0;
                for (i, slice) in strings.chunks(SLICE).enumerate() {
                    if b.ai.translating.get() != run {
                        return; // another run, or the original asked for
                    }
                    let body = ai::translate_request(&model, &language, slice);
                    let answer = whole_answer(&endpoint, &body).await;
                    let Some(list) = answer.ok().and_then(|a| ai::translations(&a, slice.len())) else {
                        failed += 1;
                        continue;
                    };
                    let put = format!("window.__ripplTranslate.put({}, {})", i * SLICE, serde_json::to_string(&list).unwrap_or_default());
                    v.evaluate_javascript(&put, Some("rippl"), None, None::<&gio::Cancellable>, |_| {});
                }
                if failed > 0 {
                    b.notice(&format!("{failed} part{} of the page couldn't be translated", if failed == 1 { "" } else { "s" }));
                }
            });
        });
    }
}

/// Streams an answer, calling `piece` for each bit of it (`first` for the
/// first).
async fn stream_answer(endpoint: &str, body: &str, cancel: &gio::Cancellable, piece: impl Fn(&str, bool)) -> Result<(), String> {
    let (session, message) = request(endpoint, body)?;
    let stream = session.send_future(&message, glib::Priority::DEFAULT).await.map_err(|e| e.to_string())?;
    if message.status() != soup::Status::Ok {
        return Err(format!("HTTP {:?}", message.status()));
    }
    let lines = gio::DataInputStream::new(&stream);
    let mut first = true;
    loop {
        if cancel.is_cancelled() {
            return Ok(());
        }
        let line = lines.read_line_utf8_future(glib::Priority::DEFAULT).await.map_err(|e| e.to_string())?;
        let Some(line) = line else { break };
        if line.trim() == "data: [DONE]" {
            break;
        }
        if let Some(p) = ai::stream_piece(&line) {
            piece(&p, first);
            first = false;
        }
    }
    if first {
        return Err("the model answered nothing".into());
    }
    Ok(())
}

/// A whole answer, not streamed.
async fn whole_answer(endpoint: &str, body: &str) -> Result<String, String> {
    let (session, message) = request(endpoint, body)?;
    let bytes = session.send_and_read_future(&message, glib::Priority::DEFAULT).await.map_err(|e| e.to_string())?;
    if message.status() != soup::Status::Ok {
        return Err(format!("HTTP {:?}", message.status()));
    }
    ai::answer_text(&String::from_utf8_lossy(&bytes)).ok_or_else(|| "an answer it couldn't read".into())
}
