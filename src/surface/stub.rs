//! Deterministic stub model server for the surface harness (`--stub`).
//!
//! A tiny HTTP server that answers like an OpenAI-compatible backend so the
//! whole pipeline (chat tool loop, then composition) can run without a real
//! model or network. It also records every request body it receives, which the
//! harness uses to verify on the wire that the composer call never advertises
//! tools.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Canned surface the stub returns for the composition call. Binds to `tool-0`
/// and only uses widgets with soft value checks, so it validates against
/// whatever the real `health` tool happens to return.
pub const STUB_SURFACE_JSON: &str = r#"{"intent":"stub health","title":"Stub Health","subtitle":"deterministic harness run","placement":{"edge":"left","width":"narrow","float":false},"layout":{"mode":"grid","columns":12},"regions":[{"id":"main","span":12,"priority":"primary","widgets":["health-list"]}],"widgets":[{"type":"statusList","id":"health-list","title":"Node health roll-up","items":[{"label":"overall","status":"mixed","detail":null}],"evidence":["tool-0"]}]}"#;

pub const STUB_ANSWER: &str = "stub: the system health was rolled up from the graph";

/// A running stub server plus the request bodies it received.
pub struct StubServer {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<String>>>,
}

impl StubServer {
    /// Spawn the server. The handler dispatches on the request body:
    /// - composition call (system prompt marker) -> `STUB_SURFACE_JSON`
    /// - tool-loop continuation (tool result present) -> final answer
    /// - otherwise -> a `health` tool call for the planner loop
    pub fn spawn() -> Self {
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub server");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                let _ = reader.read_line(&mut request_line);
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).expect("line") == 0 || line.trim().is_empty() {
                        break;
                    }
                    headers.push_str(&line);
                }
                let mut body = String::new();
                if let Some(len) = headers.lines().find_map(|l| {
                    let lower = l.trim().to_lowercase();
                    lower
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().to_string())
                }) {
                    let len: usize = len.parse().unwrap_or(0);
                    let mut buf = vec![0u8; len];
                    let _ = reader.read_exact(&mut buf);
                    body = String::from_utf8_lossy(&buf).into_owned();
                }
                captured.lock().expect("stub lock").push(body.clone());
                let response_body = if body.contains("Aios surface composer") {
                    openai_response(STUB_SURFACE_JSON)
                } else if body.contains("tool health result") {
                    openai_response(STUB_ANSWER)
                } else {
                    openai_response(r#"{"tool_calls":[{"tool":"health","args":""}]}"#)
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{response_body}"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, requests }
    }

    /// All request bodies received, oldest first.
    pub fn captured(&self) -> Vec<String> {
        self.requests.lock().expect("stub lock").clone()
    }

    /// True if no request body advertises a `tools` array or `tool_calls`.
    /// The composition call must never carry tool definitions (structural
    /// guard, see `surface::composer`).
    pub fn no_tool_advertisement(&self) -> bool {
        self.captured()
            .iter()
            .filter(|body| body.contains("Aios surface composer"))
            .all(|body| !body.contains("\"tools\"") && !body.contains("tool_calls"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_server_serves_all_stages() {
        let server = StubServer::spawn();
        let client = std::net::TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
        let mut client = client;
        // planner turn -> health tool call
        let body = r#"{"messages":[{"role":"system","content":"Read-only machine tools are available"}]}"#;
        let _ = client.write_all(
            format!(
                "POST /v1/chat HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .as_bytes(),
        );
        let _ = client.flush();
        let mut reader = BufReader::new(client);
        let mut line = String::new();
        // consume headers
        loop {
            line.clear();
            if reader.read_line(&mut line).expect("read") == 0 || line.trim().is_empty() {
                break;
            }
        }
        let mut payload = String::new();
        let _ = reader.read_to_string(&mut payload);
        assert!(payload.contains("tool_calls"), "{payload}");
        drop(reader);
    }
}
