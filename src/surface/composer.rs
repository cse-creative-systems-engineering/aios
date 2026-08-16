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

use crate::model::{AgentRole, GatewayError, GenerationRequest, ModelGateway, ModelMessage, ModelRole, ModelTask};
use crate::planner::{extract_json, strip_think};
use crate::protocol::DataClassification;
use crate::surface::{EvidenceIndex, Surface, evidence_brief};

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

/// Run the composition model call and return the parsed surface. The caller
/// (Phase E) maps a failure to a plain answer plus a `Notice` widget; this
/// function never falls back silently.
pub fn compose_surface(
    gateway: &ModelGateway,
    intent: &str,
    answer: &str,
    evidence: &EvidenceIndex,
    max_tokens: u32,
) -> Result<Surface, SurfaceComposeError> {
    let user = format!(
        "User intent:\n{intent}\n\nGrounded answer:\n{answer}\n\nEvidence:\n{}",
        evidence_brief(evidence)
    );
    let task = ModelTask::new(AgentRole::SurfaceComposition, DataClassification::Public);
    let request = GenerationRequest {
        task_id: task.task_id,
        messages: vec![
            ModelMessage::new(ModelRole::System, surface_composition_instructions()),
            ModelMessage::new(ModelRole::User, user),
        ],
        max_tokens,
        temperature: 0.2,
        seed: None,
    };
    let response = gateway.submit_with_fallback(&task, &request)?;
    let text = strip_think(response.response.text.trim());
    if text.is_empty() {
        return Err(SurfaceComposeError::EmptyResponse);
    }
    let body = extract_json(&text)
        .ok_or_else(|| SurfaceComposeError::Format("no JSON object in model reply".into()))?;
    serde_json::from_str::<Surface>(&body)
        .map_err(|error| SurfaceComposeError::Format(format!("surface schema rejected: {error}")))
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
        "  layout      { mode: \"grid\"|\"stack\"|\"row\", columns: integer }",
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
        "- Use the smallest sensible set of widgets that covers the evidence.",
        "- Grid columns default to 12; each region span must fit within columns.",
        "- Only set placement when the user asked for a specific position or",
        "  size (for example \"docked to the right\" or \"narrow panel\"). Never",
        "  output pixel coordinates.",
        "- If the evidence cannot support a panel, output a single notice widget",
        "  explaining what is unavailable.",
        "- Reply with ONLY the JSON object. No prose, no markdown fences.",
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
}
