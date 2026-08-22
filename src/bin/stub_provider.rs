//! Standalone stub surface model for driving the real desktop app.
//!
//! A tiny HTTP server that answers like an OpenAI-compatible backend so the
//! Tauri app can be pointed at it via `AIOS_CONFIG` and exercised end to end
//! without a real model or network. It plays both roles the app talks to:
//!
//! - planner turns get a canned `health` tool call,
//! - tool continuations get a plain grounded answer,
//! - groundless generation calls (system prompt starts with "You are a
//!   generative UI designer") get a themed HTML fragment that marks every
//!   available specialist field verbatim, so the value-fidelity gate passes
//!   and each prompt theme renders a distinct surface.
//!
//! Prints the listening port on stdout and serves until terminated.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

const STUB_ANSWER: &str = "stub: the system health was rolled up from the graph";

fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub provider");
    let port = listener.local_addr().expect("addr").port();
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "stub provider listening on 127.0.0.1:{port}");
    let _ = stdout.flush();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut request_line = String::new();
        let _ = reader.read_line(&mut request_line);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read") == 0 || line.trim().is_empty() {
                break;
            }
            headers.push_str(&line);
        }
        let mut body = String::new();
        if let Some(len) = headers.lines().find_map(|l| {
            l.trim()
                .to_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().to_string())
        }) {
            let len: usize = len.parse().unwrap_or(0);
            let mut buf = vec![0u8; len];
            let _ = reader.read_exact(&mut buf);
            body = String::from_utf8_lossy(&buf).into_owned();
        }
        let response_body = respond(&body);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{response_body}"
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }
}

fn openai_response(content: &str) -> String {
    serde_json::json!({
        "choices": [{
            "message": { "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "total_tokens": 10 }
    })
    .to_string()
}

fn respond(body: &str) -> String {
    if body.contains("generative UI designer") {
        openai_response(&themed_surface_html(body))
    } else if body.contains("tool health result") {
        openai_response(STUB_ANSWER)
    } else {
        openai_response(r#"{"tool_calls":[{"tool":"health","args":""}]}"#)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Theme {
    Disk,
    Memory,
    Cpu,
    Network,
    Health,
}

impl Theme {
    fn key(self) -> &'static str {
        match self {
            Theme::Disk => "disk",
            Theme::Memory => "memory",
            Theme::Cpu => "cpu",
            Theme::Network => "network",
            Theme::Health => "health",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Theme::Disk => "Disk",
            Theme::Memory => "Memory",
            Theme::Cpu => "CPU",
            Theme::Network => "Network",
            Theme::Health => "System health",
        }
    }
}

fn theme_of(prompt: &str) -> Theme {
    let prompt = prompt.to_ascii_lowercase();
    let any = |words: &[&str]| words.iter().any(|word| prompt.contains(word));
    if any(&["disk", "drive", "storage"]) {
        Theme::Disk
    } else if any(&["memory", "ram", "swap"]) {
        Theme::Memory
    } else if any(&["cpu", "process", "load"]) {
        Theme::Cpu
    } else if any(&["network", "wifi", "internet", "connectivity"]) {
        Theme::Network
    } else {
        Theme::Health
    }
}

/// The current user prompt out of a request body: the last `user` message's
/// first line after the relay header.
fn user_intent_from(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let messages = value.get("messages")?.as_array()?;
    let last_user = messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(|r| r.as_str()) == Some("user"))?;
    let content = last_user.get("content")?.as_str()?;
    let prompt = content.strip_prefix("User request:\n").unwrap_or(content);
    prompt.lines().next().map(|line| line.trim().to_string())
}

/// Specialist fields out of the relayed user message. The app lists them one
/// `name=value` per line under this exact header; echoing them back verbatim
/// keeps the fidelity gate satisfied without inventing anything.
const FIELDS_HEADER: &str = "Available fields (use these exact names in data-aios):\n";

fn fields_from_body(body: &str) -> Vec<(String, String)> {
    let Some(start) = body.find(FIELDS_HEADER) else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    for line in body[start + FIELDS_HEADER.len()..].lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Previous generated design:") {
            break;
        }
        if let Some((name, value)) = line.split_once('=') {
            fields.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    fields
}

fn themed_surface_html(body: &str) -> String {
    let theme = user_intent_from(body).map_or(Theme::Health, |intent| theme_of(&intent));
    let fields = fields_from_body(body);
    let rows: Vec<String> = fields
        .iter()
        .take(12)
        .map(|(name, value)| {
            format!(
                "<li><span class=\"field-name\">{}</span> <span data-aios=\"{}\">{}</span></li>",
                escape_html(name),
                escape_html(name),
                escape_html(value)
            )
        })
        .collect();
    let height = (180 + 34 * rows.len()).min(620);
    // Deliberately emits the legacy `data-tauri-drag-region` attribute: the
    // canvas renames it on render, and the e2e suite relies on that path to
    // prove surfaces authored against the old prompt stay draggable.
    format!(
        "<section class=\"surface aios-surface\" data-aios-theme=\"{}\" style=\"width:420px;height:{}px;display:flex;flex-direction:column;font-family:sans-serif;background:#161b26;color:#e8ecf4;padding:18px;border-radius:14px\" data-tauri-drag-region><h1 style=\"font-size:18px;margin:0 0 12px\">{} health roll-up</h1><ul style=\"list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:8px;font-size:14px\">{}</ul></section>",
        theme.key(),
        height,
        theme.title(),
        rows.join("")
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
