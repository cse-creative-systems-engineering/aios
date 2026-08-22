//! Groundless surface generation relay (ADR-0007).
//!
//! Aios never designs a surface itself and keeps no widget vocabulary. It
//! relays exactly two things to a separate surface model routed under the
//! `SurfaceComposition` role: the user's request and the specialist data
//! gathered through the broker. That call is groundless - no tools, no
//! backend access, no other input. The model authors a self-contained HTML
//! fragment; Aios verifies value fidelity against the evidence before
//! anything is displayed.
//!
//! Because the system prompt never contains the tool-advertisement marker
//! that `HttpBackend` looks for, no tool definitions are sent to the model at
//! all - the model physically cannot request a tool in this call.

use crate::model::{
    AgentRole, GatewayError, GenerationRequest, ModelGateway, ModelMessage, ModelRole, ModelTask,
    RoutingDecision,
};
use crate::planner::strip_think;
use crate::protocol::DataClassification;
use crate::surface::{EvidenceIndex, evidence_brief};
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

/// System prompt for the groundless surface generation call. Deliberately
/// plain prose: no tool advertisement, no framework, no widget menu. The
/// model chooses any structure it likes; only the value-binding rule is
/// enforced mechanically afterwards.
pub fn unconstrained_generation_instructions() -> &'static str {
    "You are a generative UI designer for Aios, a Linux system assistant. Design the best widget for the user's request using ONLY the specialist data provided. If a previous generated design is provided, revise that design rather than starting over, preserving its visual language unless the user asks otherwise. Return only a complete self-contained HTML fragment with inline CSS. You may choose any HTML structure, visual hierarchy, layout, typography, colors, controls, and styling. Set explicit width and height on the root element so the host window can fit the design. You may re-shape values (formatting, units, gauges) but must never change a numeric value or invent one that is not in the specialist data. Wrap every displayed data value in a span with a data-aios attribute naming the source field, for example <span data-aios=\"cpu_utilization_percent\">6.5%</span>. If the widget has a header, mark it with data-tauri-drag-region. Do not explain your design and do not use markdown fences."
}

/// Relay the user request and specialist data to the surface model and return
/// the generated HTML fragment. Aios gathers, relays, and verifies; the model
/// alone authors the presentation. The call is intentionally single-shot: a
/// malformed reply is an observable generation failure, never a correction or
/// provider fallback that could hide the problem.
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
            ModelMessage::new(ModelRole::System, unconstrained_generation_instructions()),
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
        model: None,
    };
    let response = gateway.submit(&task, &request)?;
    let html = strip_think(response.response.text.trim());
    if html.trim().is_empty() {
        return Err(SurfaceComposeError::EmptyResponse);
    }
    Ok((html, response.decision))
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

/// Ratios mapping a displayed unit back onto the raw specialist unit. The
/// generation prompt allows re-shaping values ("formatting, units"), so a
/// model may render `rss_kb=167352` as "167 MB"; the gate honours that by
/// comparing across common decimal and binary prefixes. An invented number
/// matches under no ratio and still fails.
const UNIT_SCALES: [f64; 7] = [
    1.0,
    1000.0,
    1024.0,
    1_000_000.0,
    1_048_576.0,
    1_000_000_000.0,
    1_073_741_824.0,
];

fn numbers_match(shown: f64, expected: f64) -> bool {
    UNIT_SCALES.iter().any(|&scale| {
        let scaled = shown * scale;
        (scaled - expected).abs() <= tolerance(expected).max(expected.abs() * 0.01)
    })
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
            .any(|number| expected.iter().any(|e| numbers_match(*number, *e)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_instructions_never_advertise_tools() {
        // HttpBackend only attaches tool definitions when the system message
        // contains this exact marker; the relay call must stay tool-less.
        let instructions = unconstrained_generation_instructions();
        assert!(!instructions.contains("Read-only machine tools are available"));
        assert!(!instructions.contains("tool_calls"));
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
    fn fidelity_accepts_unit_derived_value_from_composite_field() {
        // The prompt permits re-shaping values ("formatting, units"); a model
        // rendering rss_kb=167080 as "167 MB" must pass, matching the exact
        // failure seen live with dots-3.
        let evidence = [crate::tools::ToolResult {
            tool: "processes.observe_process",
            text: "top_cpu_1=\"pid=329762 comm=WebKitWebProces cpu_percent=55.6 rss_kb=167080 state=S\""
                .into(),
        }];
        let html = r#"<div><span data-aios="top_cpu_1">167 MB</span></div>"#;
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
