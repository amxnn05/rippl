//! `rippl --mcp`: the MCP server a coding agent runs,
//! over standard input and output, one JSON-RPC message a line, as MCP's
//! stdio transport has it. Each tool call becomes one `agent-…` command on
//! the running rippl's socket (agents.rs), where every rule is checked: this
//! process only passes words along. No window, no engine.
//!
//! In Claude Code: `claude mcp add rippl -- rippl --mcp` (with `agents = on`
//! in rippl's settings).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use serde_json::{Value, json};

/// The tools, as `tools/list` gives them.
fn tools() -> Value {
    let tab = json!({"type": "integer", "description": "The tab's number, from rippl_tabs or rippl_open"});
    json!([
        {"name": "rippl_tabs", "description": "List the tabs this agent may see: its own (own: true), and the user's if they allowed it. Each has its number, address, title and whether it is still loading.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "rippl_open", "description": "Open an http(s) address in a new tab of the agent's own, behind the user's. Returns its number.",
         "inputSchema": {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}},
        {"name": "rippl_navigate", "description": "Go to an http(s) address in one of the agent's own tabs.",
         "inputSchema": {"type": "object", "properties": {"tab": tab, "url": {"type": "string"}}, "required": ["tab", "url"]}},
        {"name": "rippl_read", "description": "The text of a tab's page, with its title and address.",
         "inputSchema": {"type": "object", "properties": {"tab": tab}, "required": ["tab"]}},
        {"name": "rippl_click", "description": "Click the first element a CSS selector matches, in one of the agent's own tabs.",
         "inputSchema": {"type": "object", "properties": {"tab": tab, "selector": {"type": "string"}}, "required": ["tab", "selector"]}},
        {"name": "rippl_type", "description": "Put text in the box a CSS selector matches, in one of the agent's own tabs. Never a password or card field.",
         "inputSchema": {"type": "object", "properties": {"tab": tab, "selector": {"type": "string"}, "text": {"type": "string"}}, "required": ["tab", "selector", "text"]}},
        {"name": "rippl_screenshot", "description": "A picture (PNG) of the visible part of a tab's page.",
         "inputSchema": {"type": "object", "properties": {"tab": tab}, "required": ["tab"]}},
        {"name": "rippl_close", "description": "Close one of the agent's own tabs.",
         "inputSchema": {"type": "object", "properties": {"tab": tab}, "required": ["tab"]}},
    ])
}

/// One command to rippl, and its answer.
fn ask(command: &str) -> Result<String, String> {
    let path = crate::control::socket_path();
    let mut stream = UnixStream::connect(&path)
        .map_err(|_| "rippl isn't running, or agents are off (rippl://settings › About › Developer)".to_string())?;
    writeln!(stream, "{command}").map_err(|e| e.to_string())?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).map_err(|e| e.to_string())?;
    Ok(answer.trim().to_string())
}

/// A tool call: its MCP result.
fn call(name: &str, args: &Value) -> Value {
    let verb = match name {
        "rippl_tabs" => "tabs",
        "rippl_open" => "open",
        "rippl_navigate" => "go",
        "rippl_read" => "read",
        "rippl_click" => "click",
        "rippl_type" => "type",
        "rippl_screenshot" => "shot",
        "rippl_close" => "close",
        _ => return json!({"content": [{"type": "text", "text": format!("no tool {name}")}], "isError": true}),
    };
    let answer = match ask(&format!("agent-{verb} {args}")) {
        Ok(a) => a,
        Err(e) => return json!({"content": [{"type": "text", "text": e}], "isError": true}),
    };
    let parsed: Value = serde_json::from_str(&answer).unwrap_or(json!({"text": answer}));
    if let Some(e) = parsed.get("error").and_then(Value::as_str) {
        return json!({"content": [{"type": "text", "text": e}], "isError": true});
    }
    if let Some(png) = parsed.get("png").and_then(Value::as_str) {
        return json!({"content": [{"type": "image", "data": png, "mimeType": "image/png"}]});
    }
    if verb == "read"
        && let Some(text) = parsed.get("text").and_then(Value::as_str)
    {
        let head = format!(
            "{}\n{}\n\n",
            parsed.get("title").and_then(Value::as_str).unwrap_or(""),
            parsed.get("url").and_then(Value::as_str).unwrap_or("")
        );
        return json!({"content": [{"type": "text", "text": head + text}]});
    }
    json!({"content": [{"type": "text", "text": parsed.to_string()}]})
}

/// One message in: the reply to send, if it wants one.
pub fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone(); // notifications get no reply
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(json!({}));
    let result = match method {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or("2024-11-05"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "rippl", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "rippl, a web browser on this computer. Open pages in tabs of your own with rippl_open, then read, click, type and take screenshots there. The user's own tabs are read-only, and only if they allowed it."
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            call(name, params.get("arguments").unwrap_or(&json!({})))
        }
        _ => return Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("no method {method}")}})),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// The loop: a line in, a line out, until standard input ends.
pub fn run() -> i32 {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(m) => handle(&m),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
        };
        if let Some(r) = reply {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaks_mcp() {
        let init = handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "rippl");
        assert!(handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let list = handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 8);
        let bad = handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "nope"})).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
    }
}
