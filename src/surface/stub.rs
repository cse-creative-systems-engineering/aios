//! Deterministic stub model server for the surface harness (`--stub`).
//!
//! A tiny HTTP server that answers like an OpenAI-compatible backend so the
//! whole pipeline (chat tool loop, then composition) can run without a real
//! model or network. It also records every request body it receives, which the
//! harness uses to verify on the wire that the composer call never advertises
//! tools.

use crate::surface::schema::{
    ChartPoint, DockEdge, LayoutMode, RegionPriority, StatusItem, Surface, SurfaceDensity,
    SurfaceLayout,
    SurfacePlacement, SurfaceRegion, SurfaceWidget, WidthClass,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Canned surface the stub returns for the composition call. Binds to `tool-0`
/// and only uses widgets with soft value checks, so it validates against
/// whatever the real `health` tool happens to return.
pub const STUB_SURFACE_JSON: &str = r#"{"intent":"stub health","title":"Stub Health","subtitle":"deterministic harness run","placement":{"edge":"left","width":"narrow","float":false},"layout":{"mode":"grid","columns":12},"regions":[{"id":"main","span":12,"priority":"primary","widgets":["health-list"]}],"widgets":[{"type":"statusList","id":"health-list","title":"Node health roll-up","items":[{"label":"overall","status":"mixed","detail":null}],"evidence":["tool-0"]}]}"#;

pub const STUB_ANSWER: &str = "stub: the system health was rolled up from the graph";

#[derive(Clone, Copy, PartialEq)]
enum StubKind {
    Plain,
    /// Theme-aware responses for the UI end-to-end test: the composed surface
    /// varies with the user prompt so the test can drive many metric themes
    /// and assert on each rendered composition.
    Themed,
}

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
        Self::spawn_with(StubKind::Plain)
    }

    /// Spawn with theme-aware responses (see `StubKind::Themed`).
    pub fn spawn_themed() -> Self {
        Self::spawn_with(StubKind::Themed)
    }

    fn spawn_with(kind: StubKind) -> Self {
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
                let response_body = respond(kind, &body);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{response_body}"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self {
            port,
            requests,
        }
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

fn respond(kind: StubKind, body: &str) -> String {
    if body.contains("Aios surface composer") {
        let content = match kind {
            StubKind::Plain => STUB_SURFACE_JSON.to_string(),
            StubKind::Themed => themed_surface_json(body),
        };
        openai_response(&content)
    } else if body.contains("tool health result") {
        openai_response(STUB_ANSWER)
    } else {
        openai_response(r#"{"tool_calls":[{"tool":"health","args":""}]}"#)
    }
}

/// Theme bucket for the UI end-to-end test. Each theme maps to a distinct
/// surface so the test can verify several metric compositions in one run.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Theme {
    Disk,
    Memory,
    Cpu,
    Network,
    Health,
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
    } else if any(&["network", "wifi", "internet"]) {
        Theme::Network
    } else {
        Theme::Health
    }
}

/// The composed surface for a request body. The `Health` theme reads the real
/// `health` tool numbers out of the evidence in the body and binds them into
/// a `MetricCard`/`SensorGauge`, exercising the value-provenance validator
/// end to end; the other themes use advisory widgets (soft checks) so they
/// validate against any health roll-up.
fn themed_surface_json(body: &str) -> String {
    let prompt = user_intent_from(body).unwrap_or_default();
    let surface = themed_surface(theme_of(&prompt), body);
    serde_json::to_string(&surface).expect("serialize themed surface")
}

fn themed_surface(theme: Theme, body: &str) -> Surface {
    let nodes = number_before(body, " nodes total");
    let healthy = number_after(body, "Healthy:");
    let evidence = vec!["tool-0".to_string()];
    let (title, subtitle, widgets, region_id) = match theme {
        Theme::Disk => (
            "Disk health",
            "storage roll-up",
            vec![
                SurfaceWidget::StatusList {
                    id: "disk-overview".to_string(),
                    title: "Disk overview".to_string(),
                    items: vec![
                        StatusItem {
                            label: "storage devices".to_string(),
                            status: "Healthy".to_string(),
                            detail: Some("no failing drives reported".to_string()),
                        },
                        StatusItem {
                            label: "filesystems".to_string(),
                            status: "Healthy".to_string(),
                            detail: None,
                        },
                    ],
                    evidence: evidence.clone(),
                },
                SurfaceWidget::Notice {
                    id: "disk-note".to_string(),
                    title: "Disk health".to_string(),
                    body: "The health roll-up reports no degraded storage devices.".to_string(),
                    evidence,
                },
            ],
            "disk",
        ),
        Theme::Memory => (
            "Memory",
            "memory pressure check",
            vec![
                SurfaceWidget::StatusList {
                    id: "memory-overview".to_string(),
                    title: "Memory overview".to_string(),
                    items: vec![
                        StatusItem {
                            label: "system memory".to_string(),
                            status: "Healthy".to_string(),
                            detail: None,
                        },
                        StatusItem {
                            label: "swap".to_string(),
                            status: "Healthy".to_string(),
                            detail: None,
                        },
                    ],
                    evidence: evidence.clone(),
                },
                SurfaceWidget::Notice {
                    id: "memory-note".to_string(),
                    title: "Memory pressure".to_string(),
                    body: "The specialists report no memory pressure.".to_string(),
                    evidence,
                },
            ],
            "memory",
        ),
        Theme::Cpu => (
            "CPU",
            "load snapshot",
            vec![
                SurfaceWidget::StatusList {
                    id: "cpu-overview".to_string(),
                    title: "CPU overview".to_string(),
                    items: vec![StatusItem {
                        label: "overall load".to_string(),
                        status: "normal".to_string(),
                        detail: Some("sampled from the health roll-up".to_string()),
                    }],
                    evidence: evidence.clone(),
                },
                SurfaceWidget::Chart {
                    id: "cpu-history".to_string(),
                    title: "Recent activity".to_string(),
                    data: vec![
                        ChartPoint {
                            label: "idle".to_string(),
                            value: 70.0,
                        },
                        ChartPoint {
                            label: "busy".to_string(),
                            value: 30.0,
                        },
                    ],
                    evidence,
                },
            ],
            "cpu",
        ),
        Theme::Network => (
            "Network",
            "connectivity check",
            vec![
                SurfaceWidget::StatusList {
                    id: "network-overview".to_string(),
                    title: "Network overview".to_string(),
                    items: vec![
                        StatusItem {
                            label: "connectivity".to_string(),
                            status: "Internet".to_string(),
                            detail: None,
                        },
                        StatusItem {
                            label: "interfaces".to_string(),
                            status: "up".to_string(),
                            detail: None,
                        },
                    ],
                    evidence: evidence.clone(),
                },
                SurfaceWidget::Notice {
                    id: "network-note".to_string(),
                    title: "Network".to_string(),
                    body: "Connectivity is available and interfaces are up.".to_string(),
                    evidence,
                },
            ],
            "network",
        ),
        Theme::Health => {
            let nodes = nodes.unwrap_or(0);
            let healthy = healthy.unwrap_or(0);
            let overall = if nodes > 0 && healthy == nodes {
                "healthy"
            } else {
                "degraded"
            };
            (
                "System health",
                "graph roll-up",
                vec![
                    SurfaceWidget::MetricCard {
                        id: "node-count".to_string(),
                        title: "Graph nodes".to_string(),
                        value: nodes.to_string(),
                        unit: Some("nodes".to_string()),
                        status: Some("scanned".to_string()),
                        evidence: evidence.clone(),
                    },
                    SurfaceWidget::SensorGauge {
                        id: "healthy-gauge".to_string(),
                        title: "Healthy nodes".to_string(),
                        value: healthy as f64,
                        min: Some(0.0),
                        max: Some(nodes.max(1) as f64),
                        unit: None,
                        evidence: evidence.clone(),
                    },
                    SurfaceWidget::StatusList {
                        id: "health-breakdown".to_string(),
                        title: "Health roll-up".to_string(),
                        items: vec![StatusItem {
                            label: "overall".to_string(),
                            status: overall.to_string(),
                            detail: Some("from the specialist health roll-up".to_string()),
                        }],
                        evidence,
                    },
                ],
                "health",
            )
        }
    };
    Surface {
        intent: "stub health".to_string(),
        title: title.to_string(),
        subtitle: Some(subtitle.to_string()),
        placement: SurfacePlacement {
            edge: Some(DockEdge::Left),
            width: Some(WidthClass::Narrow),
            float: false,
        },
        layout: SurfaceLayout {
            mode: LayoutMode::Grid,
            columns: 12,
            density: SurfaceDensity::Comfortable,
        },
        regions: vec![SurfaceRegion {
            id: region_id.to_string(),
            span: 12,
            priority: RegionPriority::Primary,
            widgets: widgets.iter().map(|widget| widget.id().to_string()).collect(),
        }],
        widgets,
    }
}

/// The current user prompt out of a composition request body: the last `user`
/// message, with the composer's "User intent:" header stripped.
fn user_intent_from(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let messages = value.get("messages")?.as_array()?;
    let last_user = messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(|r| r.as_str()) == Some("user"))?;
    let content = last_user.get("content")?.as_str()?;
    let prompt = content
        .strip_prefix("User intent:")
        .unwrap_or(content)
        .trim();
    prompt
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

/// Digits immediately preceding `needle` (e.g. "42 nodes total").
fn number_before(body: &str, needle: &str) -> Option<u32> {
    body.find(needle).and_then(|index| {
        let digits: String = body[..index]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse().ok()
        }
    })
}

/// Digits immediately following `needle` (e.g. "Healthy: 40").
fn number_after(body: &str, needle: &str) -> Option<u32> {
    body.find(needle).and_then(|index| {
        let digits: String = body[index + needle.len()..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse().ok()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::schema::SurfaceWidget;
    use crate::surface::{EvidenceIndex, validate};
    use crate::tools::ToolResult;

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

    fn compose_body(prompt: &str, evidence_text: &str) -> String {
        serde_json::json!({
            "model": "stub-model",
            "messages": [
                { "role": "system", "content": "You are the Aios surface composer." },
                { "role": "user", "content": format!(
                    "User intent:\n{prompt}\n\nGrounded answer:\nstub answer\n\nEvidence:\ntool-0 (health): {evidence_text}"
                ) }
            ],
            "max_tokens": 1024
        })
        .to_string()
    }

    fn health_evidence() -> String {
        "42 nodes total\nHealthy: 40, Unknown: 2\n  Device: 5 Healthy\n  Memory: 4 Healthy\n  Process: 20 Healthy\n  Service: 13 Healthy".to_string()
    }

    #[test]
    fn themed_stub_detects_theme_from_intent() {
        let cases = [
            ("How much disk space is left?", "Disk health"),
            ("Is the drive healthy?", "Disk health"),
            ("How is memory pressure?", "Memory"),
            ("What does the ram usage look like?", "Memory"),
            ("How busy is the cpu?", "CPU"),
            ("Any runaway processes?", "CPU"),
            ("Is the network up?", "Network"),
            ("Are we connected to the internet?", "Network"),
            ("How is my system doing?", "System health"),
        ];
        for (prompt, expected_title) in cases {
            let body = compose_body(prompt, &health_evidence());
            let surface: Surface =
                serde_json::from_str(&themed_surface_json(&body)).expect("parse surface");
            assert_eq!(surface.title, expected_title, "prompt: {prompt}");
        }
    }

    #[test]
    fn themed_stub_detects_theme_on_plain_text_prompt() {
        let body = serde_json::json!({
            "messages": [
                { "role": "user", "content": "how much disk space is free?" }
            ]
        })
        .to_string();
        assert_eq!(theme_of(&user_intent_from(&body).unwrap()), Theme::Disk);
    }

    #[test]
    fn all_themed_surfaces_validate_against_health_evidence() {
        let evidence = ToolResult {
            tool: "health",
            text: health_evidence(),
        };
        let index = EvidenceIndex::from_results(&[evidence]);
        for (prompt, _) in [
            ("how much disk space is left?", Theme::Disk),
            ("how is memory pressure?", Theme::Memory),
            ("how busy is the cpu?", Theme::Cpu),
            ("is the network up?", Theme::Network),
            ("how is my system?", Theme::Health),
        ] {
            let body = compose_body(prompt, &health_evidence());
            let surface: Surface =
                serde_json::from_str(&themed_surface_json(&body)).expect("parse surface");
            let result = validate(&surface, &index);
            assert!(result.is_ok(), "theme {prompt:?} failed: {result:?}");
        }
    }

    #[test]
    fn health_theme_binds_real_numbers_from_evidence() {
        let body = compose_body("how is my system?", &health_evidence());
        let surface: Surface =
            serde_json::from_str(&themed_surface_json(&body)).expect("parse surface");
        let widgets = surface
            .widgets
            .iter()
            .map(|widget| match widget {
                SurfaceWidget::MetricCard { value, .. } => ("metricCard", value.clone()),
                SurfaceWidget::SensorGauge { value, .. } => ("sensorGauge", value.to_string()),
                _ => (widget.id(), String::new()),
            })
            .collect::<Vec<_>>();
        assert!(widgets.contains(&("metricCard", "42".to_string())), "{widgets:?}");
        assert!(widgets.contains(&("sensorGauge", "40".to_string())), "{widgets:?}");
    }

    #[test]
    fn themed_stub_still_answers_continuation_with_plain_answer() {
        let body = serde_json::json!({
            "messages": [{ "role": "user", "content": "tool health result\n\n42 nodes total" }]
        })
        .to_string();
        let payload = respond(StubKind::Themed, &body);
        assert!(payload.contains(STUB_ANSWER), "{payload}");
    }
}
