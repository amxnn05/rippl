//! The AI add-on's words:
//! what is asked of a model, and reading what it answers. The model is one
//! you run on this computer — Ollama, llama.cpp's `llama-server`, LM Studio
//! — reached through the OpenAI-style chat API they all speak
//! (`<endpoint>/chat/completions`). Off until `ai = on`; nothing is sent
//! before you ask, and only the page you asked about.

use serde_json::{Value, json};

/// The most of a page's text sent with a question, in characters: enough
/// for an article, short enough for a small model's context.
pub const PAGE_MOST: usize = 12_000;

/// The page's text, trimmed to what is sent.
pub fn page_text(text: &str) -> String {
    let squeezed: String = text.split('\n').map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    squeezed.chars().take(PAGE_MOST).collect()
}

/// A chat request: a system message, the page, and the question.
/// `question` empty: a summary.
pub fn chat_request(model: &str, title: &str, url: &str, text: &str, question: &str, stream: bool) -> String {
    let system = "You answer questions about one web page, from its text below. Be brief and plain. \
                  If the page doesn't say, say so. Answer in the language of the question, or the page's for a summary.";
    let ask = if question.trim().is_empty() {
        "Summarize this page in a few short bullet points: what it is, and what matters in it.".to_string()
    } else {
        question.trim().to_string()
    };
    json!({
        "model": model,
        "stream": stream,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": format!("Page: {title}\nAddress: {url}\n\n{}\n\n---\n{ask}", page_text(text))},
        ]
    })
    .to_string()
}

/// A translation request: `strings` as a JSON array, the same array back
/// in `language`.
pub fn translate_request(model: &str, language: &str, strings: &[String]) -> String {
    let system = format!(
        "You translate text from web pages into {language}. You are given a JSON array of strings. \
         Answer with only a JSON array of exactly as many strings, each the translation of the one at the \
         same place. Keep numbers, names, URLs, and leading or trailing spaces. Strings already in {language} \
         stay as they are."
    );
    json!({
        "model": model,
        "stream": false,
        "temperature": 0.1,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": serde_json::to_string(strings).unwrap_or_default()},
        ]
    })
    .to_string()
}

/// One line of a streamed answer (server-sent events): the words it adds,
/// `None` at the end or for anything else.
pub fn stream_piece(line: &str) -> Option<String> {
    let data = line.strip_prefix("data:")?.trim();
    if data == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(data).ok()?;
    v.pointer("/choices/0/delta/content").and_then(|c| c.as_str()).map(String::from)
}

/// A whole (not streamed) answer's text.
pub fn answer_text(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.pointer("/choices/0/message/content").and_then(|c| c.as_str()).map(String::from)
}

/// A model's translation of `n` strings: the JSON array in its answer
/// (fences and talk around it allowed), only if it has exactly `n`.
pub fn translations(answer: &str, n: usize) -> Option<Vec<String>> {
    let start = answer.find('[')?;
    let end = answer.rfind(']')?;
    let list: Vec<Value> = serde_json::from_str(answer.get(start..=end)?).ok()?;
    let out: Vec<String> = list.into_iter().map(|v| v.as_str().map(String::from)).collect::<Option<_>>()?;
    (out.len() == n).then_some(out)
}

/// The languages offered for "Translate page to".
pub const LANGUAGES: [&str; 16] = [
    "English", "Hindi", "Spanish", "French", "German", "Portuguese", "Italian", "Dutch", "Russian", "Japanese", "Korean",
    "Chinese", "Arabic", "Turkish", "Bengali", "Urdu",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_answers() {
        let r: Value = serde_json::from_str(&chat_request("llama3.2", "T", "https://x/", "  a \n\n b ", "", true)).unwrap();
        assert_eq!(r["model"], "llama3.2");
        assert!(r["messages"][1]["content"].as_str().unwrap().contains("a\nb\n\n---\nSummarize"));
        assert_eq!(stream_piece(r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#).as_deref(), Some("Hel"));
        assert_eq!(stream_piece("data: [DONE]"), None);
        assert_eq!(stream_piece(": keep-alive"), None);
        assert_eq!(answer_text(r#"{"choices":[{"message":{"content":"ok"}}]}"#).as_deref(), Some("ok"));
        assert_eq!(page_text(&"x".repeat(20_000)).len(), PAGE_MOST);
    }

    #[test]
    fn translations_must_match() {
        assert_eq!(translations("```json\n[\"Hola\", \" mundo\"]\n```", 2), Some(vec!["Hola".into(), " mundo".into()]));
        assert_eq!(translations("[\"one\"]", 2), None, "a missing string is no translation");
        assert_eq!(translations("sorry", 1), None);
        let r: Value = serde_json::from_str(&translate_request("m", "Hindi", &["Hello".into()])).unwrap();
        assert_eq!(r["messages"][1]["content"], "[\"Hello\"]");
    }
}
