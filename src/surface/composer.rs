//! Model-driven surface composition (Phase C).
//!
//! The composer is a separate, groundless model call. It receives only the
//! user intent, the grounded answer, the indexed evidence, the closed widget
//! vocabulary, and placement rules. It never calls tools, never reads the
//! machine, and never emits code. Its only output is a `Surface` JSON object
//! matching the schema in `schema.rs`.
//!
//! Because the composer system prompt never contains the tool-advertisement
//! marker that `HttpBackend` looks for, no tool definitions are sent to the
//! model at all - the model physically cannot request a tool in this call.

use crate::model::{
    AgentRole, GatewayError, GenerationRequest, ModelGateway, ModelMessage, ModelRole, ModelTask,
    RoutingDecision,
};
use crate::planner::{extract_json, strip_think};
use crate::protocol::DataClassification;
use crate::surface::{EvidenceIndex, Surface, evidence_brief, validate_for_intent};
use crate::tools::ToolResult;

#[derive(Debug)]
pub enum SurfaceComposeError {
    Gateway(GatewayError),
    EmptyResponse,
    Format(String),
}

impl std::fmt::Display for SurfaceComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurfaceComposeError::Gateway(e) => write!(f, "model call failed: {e}"),
            SurfaceComposeError::EmptyResponse => write!(f, "model returned no text"),
            SurfaceComposeError::Format(reason) => {
                write!(f, "surface composition was not usable: {reason}")
            }
        }
    }
}

impl std::error::Error for SurfaceComposeError {}

impl From<GatewayError> for SurfaceComposeError {
    fn from(e: GatewayError) -> Self {
        SurfaceComposeError::Gateway(e)
    }
}

/// Run the composition model call and return the parsed surface. A failure is
/// returned to the caller and no replacement surface is created.
pub fn compose_surface(
    gateway: &ModelGateway,
    intent: &str,
    answer: &str,
    evidence: &EvidenceIndex,
    max_tokens: u32,
) -> Result<Surface, SurfaceComposeError> {
    compose_surface_with_meta(gateway, intent, answer, evidence, max_tokens).map(|(surface, _)| surface)
}

/// Like `compose_surface`, but also returns the routing decision the gateway
/// actually used for the call. The harness records this for monitoring so it
/// can confirm which provider/model served the composition.
///
/// The call is intentionally single-shot during development. A malformed
/// model response is an observable composition failure, never a correction or
/// provider fallback that could hide the problem.
pub fn compose_surface_with_meta(
    gateway: &ModelGateway,
    intent: &str,
    answer: &str,
    evidence: &EvidenceIndex,
    max_tokens: u32,
) -> Result<(Surface, RoutingDecision), SurfaceComposeError> {
    let task = ModelTask::new(AgentRole::SurfaceComposition, DataClassification::Public);
    let user = format!(
        "User intent:\n{intent}\n\nGrounded answer:\n{answer}\n\nEvidence:\n{}",
        evidence_brief(evidence)
    );
    let messages = vec![
        ModelMessage::new(ModelRole::System, surface_composition_instructions()),
        ModelMessage::new(ModelRole::User, user),
    ];

    let submit = |messages: &Vec<ModelMessage>| -> Result<(Surface, RoutingDecision), SurfaceComposeError> {
        let request = GenerationRequest {
            task_id: task.task_id,
            messages: messages.clone(),
            max_tokens,
            temperature: 0.2,
            seed: None,
        };
        let response = gateway.submit(&task, &request)?;
        let surface = parse_surface(&response.response.text)?;
        validate_for_intent(&surface, intent, evidence).map_err(|error| {
            SurfaceComposeError::Format(format!("surface validation failed: {error}"))
        })?;
        Ok((surface, response.decision))
    };

    submit(&messages)
}

/// Groundless generation path (ADR-0007): the model receives only the user
/// request and the specialist data relayed by Aios. It has no tools, no
/// backend, and no other input. It returns a self-contained HTML fragment.
pub fn compose_unconstrained_html(
    gateway: &ModelGateway,
    intent: &str,
    evidence: &EvidenceIndex,
    previous_html: Option<&str>,
    max_tokens: u32,
) -> Result<(String, RoutingDecision), SurfaceComposeError> {
    let task = ModelTask::new(AgentRole::SurfaceComposition, DataClassification::Public);
    let request = GenerationRequest {
        task_id: task.task_id,
        messages: vec![
            ModelMessage::new(
                ModelRole::System,
                "You are a generative UI designer for Aios, a Linux system assistant. Design the best widget for the user's request using ONLY the specialist data provided. If a previous generated design is provided, revise that design rather than starting over, preserving its visual language unless the user asks otherwise. Return only a complete self-contained HTML fragment with inline CSS. You may choose any HTML structure, visual hierarchy, layout, typography, colors, controls, and styling. Set explicit width and height on the root element so the host window can fit the design. You may re-shape values (formatting, units, gauges) but must never change a numeric value or invent one that is not in the specialist data. Wrap every displayed data value in a span with a data-aios attribute naming the source field, for example <span data-aios=\"cpu_utilization_percent\">6.5%</span>. If the widget has a header, mark it with data-tauri-drag-region. Do not explain your design and do not use markdown fences.",
            ),
            ModelMessage::new(ModelRole::User, {
                let previous = previous_html
                    .map(|html| format!("\n\nPrevious generated design:\n{html}"))
                    .unwrap_or_default();
                let fields = specialist_fields_from_index(evidence)
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "User request:\n{intent}\n\nSpecialist data:\n{}\n\nAvailable fields (use these exact names in data-aios):\n{}{}",
                    evidence_brief(evidence),
                    fields,
                    previous
                )
            }),
        ],
        max_tokens,
        temperature: 0.7,
        seed: None,
    };
    let response = gateway.submit(&task, &request)?;
    let html = strip_think(response.response.text.trim());
    if html.trim().is_empty() {
        return Err(SurfaceComposeError::EmptyResponse);
    }
    Ok((html, response.decision))
}

/// Parse a composition reply into a `Surface`. Handles empty replies and
/// model prose that does not contain a JSON object.
fn parse_surface(reply: &str) -> Result<Surface, SurfaceComposeError> {
    let text = strip_think(reply.trim());
    if text.is_empty() {
        return Err(SurfaceComposeError::EmptyResponse);
    }
    let body = extract_json(&text).ok_or_else(|| {
        SurfaceComposeError::Format(format!(
            "no JSON object in model reply (start: {})",
            truncate(&text, 300)
        ))
    })?;
    serde_json::from_str::<Surface>(&body).map_err(|error| {
        SurfaceComposeError::Format(format!(
            "surface schema rejected: {error} (start: {})",
            truncate(&text, 300)
        ))
    })
}

fn truncate(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(max).collect();
        out.push_str("...");
        out
    }
}

/// Deterministic coverage gate (ADR-0007 step 4): before Aios relays anything
/// to the generation model, it checks that the specialist results actually
/// cover the domains the user asked about. Returns the requested domains for
/// which no specialist evidence was gathered. This is verification only — it
/// never decides which specialists to call (the planner owns that) and never
/// suppresses evidence; an uncovered domain fails the request visibly
/// (ADR-0003) instead of producing a widget that cannot answer it.
pub fn coverage_gaps(intent: &str, evidence: &[ToolResult]) -> Vec<String> {
    const DOMAINS: &[(&str, &[&str])] = &[
        ("processes", &["cpu", "process", "service"]),
        ("memory", &["ram", "memory", "swap"]),
        ("storage", &["disk", "storage", "drive", "filesystem", "partition"]),
        ("network", &["network", "wifi", "internet", "ethernet", "wireless"]),
        ("graphics", &["gpu", "graphics"]),
        ("power", &["thermal", "temperature", "fan", "heat"]),
        ("security", &["security", "identity", "firewall"]),
        ("packages", &["package", "apt", "deb"]),
        ("boot", &["boot", "recovery", "kernel", "firmware"]),
    ];
    let lower = intent.to_ascii_lowercase();
    let mut gaps = Vec::new();
    for (domain, keywords) in DOMAINS {
        if !keywords.iter().any(|keyword| lower.contains(keyword)) {
            continue;
        }
        let covered = evidence
            .iter()
            .any(|result| result.tool.starts_with(&format!("{domain}.")));
        if !covered {
            gaps.push((*domain).to_string());
        }
    }
    gaps
}

/// Value-fidelity gate (ADR-0007 step 7): after the generation model returns
/// HTML, Aios verifies that every value the model marked with `data-aios`
/// matches the specialist data. Presentation is free, but a marked value may
/// not differ from what the specialist reported — numeric values match by
/// magnitude, string values match by content. Returns the first mismatch.
pub fn verify_value_fidelity(html: &str, evidence: &[ToolResult]) -> Result<(), String> {
    let fields: std::collections::HashMap<String, String> =
        specialist_fields(evidence).into_iter().collect();
    let markers = aios_markers(html);
    if markers.is_empty() {
        return Err("generated surface marks no values with data-aios".into());
    }
    for (field, content) in markers {
        let Some(expected) = fields.get(&field) else {
            return Err(format!("generated value '{field}' is not in specialist data"));
        };
        if !value_matches(&content, expected) {
            return Err(format!(
                "generated value '{field}' shows {:?} but specialist reported {:?}",
                content, expected
            ));
        }
    }
    Ok(())
}

fn tolerance(value: f64) -> f64 {
    (value.abs() * 0.001).max(0.005)
}

/// True when the model's marked `content` faithfully represents the specialist
/// `expected` value. If the shown content contains a number, that number must
/// appear in the specialist value (tolerating "113.7%" vs "113.7"). If it
/// contains no number, the text must trace to the specialist value — either is
/// a substring of the other, case-insensitive — so extracting `rustrover` from
/// a composite `top_cpu_0` row is accepted while an invented label is not.
fn value_matches(content: &str, expected: &str) -> bool {
    let shown = content_numbers(content);
    if !shown.is_empty() {
        let expected = content_numbers(expected);
        if expected.is_empty() {
            return false;
        }
        return shown
            .iter()
            .any(|number| expected.iter().any(|e| (number - e).abs() <= tolerance(*e)));
    }
    let content = content.to_lowercase();
    let expected = expected.to_lowercase();
    !content.is_empty() && (expected.contains(&content) || content.contains(&expected))
}

/// Named specialist fields, keyed by the field name the model uses in
/// `data-aios`. Parsed from `key=value` tokens in the specialist text; both
/// numeric and string values are retained. Quoted values are kept atomic.
fn specialist_fields(evidence: &[ToolResult]) -> Vec<(String, String)> {
    specialist_fields_from_texts(evidence.iter().map(|result| result.text.as_str()))
}

fn specialist_fields_from_index(index: &EvidenceIndex) -> Vec<(String, String)> {
    specialist_fields_from_texts(index.entries().iter().map(|entry| entry.text.as_str()))
}

fn specialist_fields_from_texts<'a>(texts: impl Iterator<Item = &'a str>) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for text in texts {
        for token in tokenize(text) {
            let Some((key, value)) = token.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_matches('"');
            let value = value.trim().trim_matches('"');
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            if seen.insert(key.to_string()) {
                out.push((key.to_string(), value.to_string()));
            }
        }
    }
    out
}

/// Split specialist text into tokens on whitespace/commas, respecting double
/// quotes so a quoted value (e.g. `top_cpu_0="pid=1 comm=x ..."`) stays one
/// token and does not leak its inner `key=value` pairs.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in text.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            current.push(c);
        } else if (c.is_whitespace() || c == ',') && !in_quotes {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Extract `data-aios` markers as `(field, inner text)` pairs.
fn aios_markers(html: &str) -> Vec<(String, String)> {
    const ATTR: &str = "data-aios=\"";
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find(ATTR) {
        let after = &rest[pos + ATTR.len()..];
        let Some(field_end) = after.find('"') else {
            break;
        };
        let field = after[..field_end].to_string();
        let Some(offset) = after[field_end..].find('>') else {
            break;
        };
        let content_start = field_end + offset + 1;
        let content_rest = &after[content_start..];
        let content_end = content_rest.find('<').unwrap_or(content_rest.len());
        let content = content_rest[..content_end].trim().to_string();
        out.push((field, content));
        rest = &content_rest[content_end..];
    }
    out
}

/// All numeric literals appearing in `text`.
fn content_numbers(text: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() || (bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            let mut end = i;
            while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
                end += 1;
            }
            if let Ok(number) = text[start..end].parse::<f64>() {
                numbers.push(number);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    numbers
}

/// System prompt for the surface composition call. Deliberately plain prose:
/// no tool advertisement, no code, a closed vocabulary, and an explicit
/// evidence-binding rule the validator later enforces mechanically.
pub fn surface_composition_instructions() -> String {
    [
        "You are the Aios surface composer. The user asked a question about this",
        "Linux system. Specialists gathered evidence and a grounded answer was",
        "written from that evidence only. Your job is to describe ONE panel",
        "surface that presents that evidence to the user.",
        "",
        "Produce ONLY a JSON object with exactly these fields:",
        "  intent      string  - echo of the user intent",
        "  title       string  - short panel heading",
        "  subtitle    string or null",
        "  placement   { edge: \"left\"|\"right\"|\"top\"|\"bottom\" or null,",
        "                width: \"narrow\"|\"medium\"|\"wide\" or null,",
        "                float: boolean }",
        "  layout      { mode: \"grid\"|\"stack\"|\"row\", columns: integer,",
        "                density: \"compact\"|\"comfortable\"|\"detailed\" }",
        "  regions     [ { id: string, span: integer, priority:",
        "                  \"primary\"|\"secondary\"|\"tertiary\",",
        "                  widgets: [widget id strings] } ]",
        "  widgets     [ one of the widget objects below ]",
        "",
        "Widget types (closed vocabulary - do not invent others):",
        "  { \"type\":\"metricCard\", id, title, value: string, unit: string or",
        "    null, status: string or null, evidence: [string] }",
        "  { \"type\":\"sensorGauge\", id, title, value: number, min: number or",
        "    null, max: number or null, unit: string or null, evidence: [string] }",
        "  { \"type\":\"statusList\", id, title, items: [ { label: string,",
        "    status: string, detail: string or null } ], evidence: [string] }",
        "  { \"type\":\"chart\", id, title, data: [ { label: string, value:",
        "    number } ], evidence: [string] }",
        "  { \"type\":\"notice\", id, title, body: string, evidence: [string] }",
        "",
        "Rules:",
        "- Bind every widget to evidence. Reference keys from the Evidence list",
        "  (tool-0, tool-1, ...). Copy string values verbatim from evidence text,",
        "  or extract a number that appears in it. Never invent a measurement.",
        "- Never compute or derive values. Do not subtract, add, convert, or",
        "  reformat numbers. If the evidence has no \"used\" figure, do not",
        "  calculate one from total minus available; show the available figure",
        "  instead. The validator rejects any value that is not present verbatim.",
        "- Use the smallest sensible set of widgets that covers the request.",
        "- Never dump all evidence into the panel. Summarize it into the fewest useful",
        "  metrics and statuses. A compact panel may have at most 6 widgets and 10",
        "  status rows. Omit secondary fields unless the user asks for detail.",
        "- If the user says compact, small, tiny, or narrow: set density to compact,",
        "  use a narrow placement, prefer grid or row, and design for a small persistent",
        "  panel. The renderer will reject a report-sized composition.",
        "- For compact system metrics, prefer one primary gauge or metric, two to",
        "  four supporting metrics, and at most one short status list.",
        "- Honor explicit quantities in the user request. If the user asks for the",
        "  top 10, use ten evidence-backed entries when ten are present. Never label",
        "  a partial result as a completed top-10 view or silently reduce the count.",
        "- Grid columns default to 12; each region span must fit within columns.",
        "- Only set placement when the user asked for a specific position or",
        "  size (for example \"docked to the right\" or \"narrow panel\"). Never",
        "  output pixel coordinates.",
        "- If the evidence cannot support a panel, output a single notice widget",
        "  explaining what is unavailable.",
        "- Reply with ONLY the JSON object. No prose, no markdown fences.",
        "",
        "Example (values copied verbatim from evidence):",
        "{\"intent\":\"How is the disk doing?\",\"title\":\"Disk health\",",
        " \"subtitle\":null,\"placement\":{\"edge\":null,\"width\":null,",
        " \"float\":false},\"layout\":{\"mode\":\"grid\",\"columns\":12},",
        " \"regions\":[{\"id\":\"top\",\"span\":12,\"priority\":\"primary\",",
        " \"widgets\":[\"disk_warn\"]}],",
        " \"widgets\":[{\"type\":\"metricCard\",\"id\":\"disk_warn\",",
        " \"title\":\"Filesystems degraded\",\"value\":\"2 degraded\",",
        " \"unit\":null,\"status\":\"degraded\",\"evidence\":[\"tool-0\"]}]}",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_never_advertise_tools() {
        // HttpBackend only attaches tool definitions when the system message
        // contains this exact marker; the composer must stay tool-less.
        let instructions = surface_composition_instructions();
        assert!(!instructions.contains("Read-only machine tools are available"));
        assert!(!instructions.contains("tool_calls"));
    }

    #[test]
    fn instructions_describe_closed_vocabulary() {
        let instructions = surface_composition_instructions();
        for widget in ["metricCard", "sensorGauge", "statusList", "chart", "notice"] {
            assert!(instructions.contains(widget), "missing {widget}");
        }
    }

    #[test]
    fn parse_surface_accepts_fenced_json() {
        let reply = "```json\n{\"intent\":\"x\",\"title\":\"t\",\"placement\":{\"edge\":null,\"width\":null,\"float\":false},\"layout\":{\"mode\":\"grid\",\"columns\":12},\"regions\":[],\"widgets\":[]}\n```";
        let surface = parse_surface(reply).expect("fenced JSON should parse");
        assert_eq!(surface.title, "t");
    }

    #[test]
    fn parse_surface_rejects_prose_without_json() {
        let error = parse_surface("Let me help you with that. The system looks healthy.")
            .expect_err("prose without JSON must fail");
        assert!(
            matches!(error, SurfaceComposeError::Format(ref message) if message.contains("no JSON object")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn parse_surface_rejects_empty_reply() {
        assert!(matches!(
            parse_surface(""),
            Err(SurfaceComposeError::EmptyResponse)
        ));
    }

    #[test]
    fn parse_surface_keeps_verbose_error_on_schema_rejection() {
        let reply = "{\"intent\":\"x\"}";
        let error = parse_surface(reply).expect_err("schema violation must fail");
        assert!(
            matches!(error, SurfaceComposeError::Format(ref message) if message.contains("surface schema rejected")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn instructions_forbid_derived_values() {
        let instructions = surface_composition_instructions();
        assert!(instructions.contains("Never compute or derive values"));
        assert!(instructions.contains("The validator rejects"));
    }

    #[test]
    fn coverage_gap_reported_for_requested_domain_without_evidence() {
        let evidence = [crate::tools::ToolResult {
            tool: "memory.observe_memory",
            text: "total = 37.5 GB".into(),
        }];
        let gaps = coverage_gaps("show cpu usage", &evidence);
        assert_eq!(gaps, vec!["processes".to_string()]);
    }

    #[test]
    fn coverage_satisfied_when_domain_present() {
        let evidence = [
            crate::tools::ToolResult {
                tool: "processes.observe_process",
                text: "cpu_utilization_percent=6.5".into(),
            },
            crate::tools::ToolResult {
                tool: "memory.observe_memory",
                text: "total = 37.5 GB".into(),
            },
        ];
        assert!(coverage_gaps("show cpu and ram", &evidence).is_empty());
    }

    #[test]
    fn coverage_ignores_unrelated_domains() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "cpu_utilization_percent=6.5".into(),
        }];
        assert!(coverage_gaps("show cpu usage", &evidence).is_empty());
        assert!(coverage_gaps("just a greeting", &evidence).is_empty());
    }

    #[test]
    fn fidelity_passes_when_marked_values_match() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "cpu_cores=20 cpu_utilization_percent=6.5".into(),
        }];
        let html = r#"<div><span data-aios="cpu_utilization_percent">6.5%</span><span data-aios="cpu_cores">20</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_ok());
    }

    #[test]
    fn fidelity_rejects_changed_value() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "cpu_utilization_percent=6.5".into(),
        }];
        let html = r#"<div><span data-aios="cpu_utilization_percent">12.3%</span></div>"#;
        let error = verify_value_fidelity(html, &evidence).expect_err("changed value must fail");
        assert!(error.contains("cpu_utilization_percent"), "{error}");
    }

    #[test]
    fn fidelity_matches_string_field() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "state=Available cpu_utilization_percent=6.5".into(),
        }];
        let html = r#"<div><span data-aios="state">Available</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_ok());
    }

    #[test]
    fn fidelity_extracts_subvalue_from_composite_field() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "top_cpu_0=\"pid=1 comm=rustrover cpu_percent=106.3 state=S\"".into(),
        }];
        let html = r#"<div><span data-aios="top_cpu_0">rustrover</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_ok());
    }

    #[test]
    fn fidelity_matches_number_inside_composite_field() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "top_cpu_0=\"pid=1 comm=rustrover cpu_percent=113.7 state=S\"".into(),
        }];
        let html = r#"<div><span data-aios="top_cpu_0">113.7%</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_ok());
    }

    #[test]
    fn fidelity_rejects_number_not_in_field() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "top_cpu_0=\"pid=1 comm=rustrover cpu_percent=113.7 state=S\"".into(),
        }];
        let html = r#"<div><span data-aios="top_cpu_0">999%</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_err());
    }

    #[test]
    fn fidelity_rejects_invented_string() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "state=Available".into(),
        }];
        let html = r#"<div><span data-aios="state">degraded</span></div>"#;
        assert!(verify_value_fidelity(html, &evidence).is_err());
    }

    #[test]
    fn fidelity_rejects_unknown_field() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "cpu_utilization_percent=6.5".into(),
        }];
        let html = r#"<div><span data-aios="made_up">99</span></div>"#;
        let error = verify_value_fidelity(html, &evidence).expect_err("unknown field must fail");
        assert!(error.contains("made_up"), "{error}");
    }

    #[test]
    fn fidelity_rejects_missing_markers() {
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "cpu_utilization_percent=6.5".into(),
        }];
        assert!(verify_value_fidelity("<div>6.5%</div>", &evidence).is_err());
    }
}
