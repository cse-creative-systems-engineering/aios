use crate::model::{
    AgentRole, GatewayError, GenerationRequest, ModelGateway, ModelMessage, ModelRole, ModelTask,
};
use crate::protocol::DataClassification;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug)]
pub enum AgentError {
    Gateway(GatewayError),
    EmptyResponse,
    Format(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Gateway(e) => write!(f, "model call failed: {e}"),
            AgentError::EmptyResponse => write!(f, "model returned no text"),
            AgentError::Format(reason) => write!(f, "model output was not usable: {reason}"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<GatewayError> for AgentError {
    fn from(e: GatewayError) -> Self {
        AgentError::Gateway(e)
    }
}

pub fn submit(
    gateway: &ModelGateway,
    role: AgentRole,
    messages: Vec<ModelMessage>,
    max_tokens: u32,
) -> Result<String, AgentError> {
    let task = ModelTask::new(role, DataClassification::Public);
    let request = GenerationRequest {
        task_id: task.task_id,
        messages,
        max_tokens,
        temperature: 0.3,
        seed: None,
    };
    let response = gateway.submit(&task, &request)?;
    let text = strip_think(response.response.text.trim());
    if text.is_empty() {
        return Err(AgentError::EmptyResponse);
    }
    Ok(text)
}

pub fn strip_think(text: &str) -> String {
    if !text.contains("<think>") {
        return text.to_string();
    }
    let mut result = String::new();
    let mut in_think = false;
    let mut rest = text;
    loop {
        if !in_think {
            match rest.find("<think>") {
                Some(index) => {
                    result.push_str(&rest[..index]);
                    rest = &rest[index + "<think>".len()..];
                    in_think = true;
                }
                None => {
                    result.push_str(rest);
                    break;
                }
            }
        } else {
            match rest.find("</think>") {
                Some(index) => {
                    rest = &rest[index + "</think>".len()..];
                    in_think = false;
                }
                None => break,
            }
        }
    }
    result.trim().to_string()
}

pub struct Planner {
    pub gateway: Arc<ModelGateway>,
    pub max_tokens: u32,
}

impl Planner {
    pub fn new(gateway: Arc<ModelGateway>, max_tokens: u32) -> Self {
        Self {
            gateway,
            max_tokens,
        }
    }

    pub fn explain(
        &self,
        question: &str,
        local_context: Option<String>,
    ) -> Result<String, AgentError> {
        let mut system = String::from(
            "You are Aios, the assistant for a Linux system. Answer concisely and \
             plainly. You can use tools and check this machine, but never claim \
             you changed anything.",
        );
        if let Some(context) = local_context {
            system.push_str("\n\nCurrent local system state:\n");
            system.push_str(&context);
        }
        self.chat_with(
            vec![
                ModelMessage::new(ModelRole::System, system),
                ModelMessage::new(ModelRole::User, question),
            ],
            None,
        )
    }

    pub fn chat_with(
        &self,
        mut messages: Vec<ModelMessage>,
        local_context: Option<String>,
    ) -> Result<String, AgentError> {
        if let Some(context) = local_context {
            if let Some(first) = messages.first_mut() {
                if first.role == ModelRole::System {
                    first.content.push_str("\n\nCurrent local system state:\n");
                    first.content.push_str(&context);
                }
            }
        }
        submit(&self.gateway, AgentRole::Planner, messages, self.max_tokens)
    }

    pub fn plan(&self, intent: &str) -> Result<GeneratedPlan, AgentError> {
        let system = "You are the Aios Planner. Turn the user's request into a list of \
                      discrete steps. Reply with ONLY a JSON object of the form \
                      {\"intent\": \"...\", \"steps\": [{\"description\": \"...\", \
                      \"tool\": \"...\", \"resource\": \"...\", \"risk\": \"read-only\"}]}. \
                      risk is exactly one of read-only, staged, or critical. Do not add \
                      prose outside the JSON.";
        let text = submit(
            &self.gateway,
            AgentRole::Planner,
            vec![
                ModelMessage::new(ModelRole::System, system),
                ModelMessage::new(ModelRole::User, intent),
            ],
            self.max_tokens,
        )?;
        parse_plan(&text, intent)
    }
}

#[derive(Clone, Debug)]
pub struct PlanStep {
    pub description: String,
    pub tool: Option<String>,
    pub resource: Option<String>,
    pub risk: String,
}

#[derive(Clone, Debug)]
pub struct GeneratedPlan {
    pub intent: String,
    pub steps: Vec<PlanStep>,
    pub freeform: Option<String>,
}

#[derive(Deserialize)]
struct PlanJson {
    intent: Option<String>,
    #[serde(default)]
    steps: Vec<PlanStepJson>,
}

#[derive(Deserialize)]
struct PlanStepJson {
    description: Option<String>,
    tool: Option<String>,
    resource: Option<String>,
    risk: Option<String>,
}

pub fn parse_plan(text: &str, fallback_intent: &str) -> Result<GeneratedPlan, AgentError> {
    let body = match extract_json(text) {
        Some(body) => body,
        None => {
            return Ok(GeneratedPlan {
                intent: fallback_intent.into(),
                steps: Vec::new(),
                freeform: Some(text.to_string()),
            });
        }
    };
    match serde_json::from_str::<PlanJson>(&body) {
        Ok(plan) => Ok(GeneratedPlan {
            intent: plan.intent.unwrap_or_else(|| fallback_intent.into()),
            steps: plan
                .steps
                .into_iter()
                .map(|step| PlanStep {
                    description: step.description.unwrap_or_default(),
                    tool: step.tool.filter(|t| !t.trim().is_empty()),
                    resource: step.resource.filter(|r| !r.trim().is_empty()),
                    risk: step
                        .risk
                        .filter(|r| r == "read-only" || r == "staged" || r == "critical")
                        .unwrap_or_else(|| "read-only".into()),
                })
                .collect(),
            freeform: None,
        }),
        Err(_) => Ok(GeneratedPlan {
            intent: fallback_intent.into(),
            steps: Vec::new(),
            freeform: Some(text.to_string()),
        }),
    }
}

pub(crate) fn extract_json(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        if bytes[start] != b'{' {
            continue;
        }
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for end in start..bytes.len() {
            let byte = bytes[end];
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        let candidate = &text[start..=end];
                        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                            return Some(candidate.to_string());
                        }
                        // Models often emit a trailing comma before } or ].
                        // Repair the candidate in place before falling back to
                        // scanning for a nested fragment.
                        if let Some(repaired) = repair_trailing_commas(candidate)
                            && serde_json::from_str::<serde_json::Value>(&repaired).is_ok()
                        {
                            return Some(repaired);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Remove commas that are immediately followed by `}` or `]`, the most common
/// JSON syntax slip from generative models. Returns `None` when nothing
/// changed.
fn repair_trailing_commas(candidate: &str) -> Option<String> {
    let bytes = candidate.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if in_string {
            out.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                out.push(byte);
            }
            b',' => {
                let mut j = i + 1;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
                    j += 1;
                }
                if j < bytes.len() && matches!(bytes[j], b'}' | b']') {
                    // Drop the trailing comma.
                } else {
                    out.push(byte);
                }
            }
            _ => out.push(byte),
        }
        i += 1;
    }
    let repaired = String::from_utf8(out).ok()?;
    if repaired == candidate {
        None
    } else {
        Some(repaired)
    }
}

#[derive(Clone, Debug)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: String,
}

/// Parse an OpenAI-style `tool_calls` request out of a model reply.
///
/// Accepts either the native shape
/// `{"tool_calls":[{"function":{"name":"...","arguments":"..."}}]}`
/// or a simpler `{"tool_calls":[{"tool":"...","args":"..."}]}`. If the
/// model wraps arguments as a JSON object such as `{"target":"wifi0"}`
/// the single target value is unwrapped into a plain argument string.
pub fn parse_tool_calls(text: &str) -> Vec<ToolCallRequest> {
    let json = match extract_json(text) {
        Some(json) => json,
        None => return Vec::new(),
    };
    let value: serde_json::Value = match serde_json::from_str(&json) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let calls = match value
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
    {
        Some(calls) => calls,
        None => return Vec::new(),
    };
    calls
        .iter()
        .filter_map(|call| {
            if let Some(function) = call.get("function") {
                let name = normalize_tool_name(function.get("name")?.as_str()?);
                let raw = function
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("");
                return Some(ToolCallRequest {
                    name,
                    arguments: normalize_arguments(raw),
                });
            }
            let name = call.get("tool")?.as_str()?;
            let raw = call.get("args").and_then(|a| a.as_str()).unwrap_or("");
            Some(ToolCallRequest {
                name: normalize_tool_name(name),
                arguments: normalize_arguments(raw),
            })
        })
        .collect()
}

fn normalize_tool_name(name: &str) -> String {
    match name {
        "wifi_observe_device" => "wifi.observe_device".to_string(),
        "wifi_diagnose_fault" => "wifi.diagnose_fault".to_string(),
        "storage_observe_storage" => "storage.observe_storage".to_string(),
        "storage_diagnose_fault" => "storage.diagnose_fault".to_string(),
        "network_observe_network" => "network.observe_network".to_string(),
        "network_diagnose_fault" => "network.diagnose_fault".to_string(),
        "drivers_observe_device" => "drivers.observe_device".to_string(),
        "drivers_diagnose_fault" => "drivers.diagnose_fault".to_string(),
        "graphics_observe_graphics" => "graphics.observe_graphics".to_string(),
        "graphics_diagnose_fault" => "graphics.diagnose_fault".to_string(),
        "memory_observe_memory" => "memory.observe_memory".to_string(),
        "memory_diagnose_fault" => "memory.diagnose_fault".to_string(),
        "processes_observe_process" => "processes.observe_process".to_string(),
        "processes_diagnose_fault" => "processes.diagnose_fault".to_string(),
        "power_observe_thermal" => "power.observe_thermal".to_string(),
        "power_diagnose_fault" => "power.diagnose_fault".to_string(),
        "security_observe_security" => "security.observe_security".to_string(),
        "security_diagnose_fault" => "security.diagnose_fault".to_string(),
        "packages_observe_package" => "packages.observe_package".to_string(),
        "packages_diagnose_fault" => "packages.diagnose_fault".to_string(),
        "boot_observe_boot" => "boot.observe_boot".to_string(),
        "boot_diagnose_fault" => "boot.diagnose_fault".to_string(),
        other => other.to_string(),
    }
}

fn normalize_arguments(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return trimmed.to_string();
    }
    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(_) => return trimmed.to_string(),
    };
    let Some(object) = value.as_object() else {
        return trimmed.to_string();
    };
    for key in ["target", "args", "arg", "value", "resource"] {
        if let Some(v) = object.get(key).and_then(serde_json::Value::as_str) {
            return v.to_string();
        }
    }
    if object.len() == 1 {
        if let Some((_, v)) = object.iter().next() {
            if let Some(s) = v.as_str() {
                return s.to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Drop the trailing tool-calls JSON object from a reply.
pub fn strip_tool_calls_json(text: &str) -> String {
    match (text.find('{'), text.rfind('}')) {
        (Some(start), Some(end)) if end >= start => {
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..start]);
            out.push_str(&text[end + 1..]);
            out
        }
        _ => text.to_string(),
    }
}

pub fn format_plan(plan: &GeneratedPlan) -> String {
    if let Some(freeform) = &plan.freeform {
        return format!("intent: {}\nsteps (unstructured):\n{freeform}", plan.intent);
    }
    let mut lines = vec![format!("intent: {}", plan.intent)];
    for (i, step) in plan.steps.iter().enumerate() {
        let tool = step.tool.as_deref().unwrap_or("-");
        let resource = step.resource.as_deref().unwrap_or("-");
        lines.push(format!(
            "  {}. [{}] {} (tool={tool}, resource={resource})",
            i + 1,
            step.risk,
            step.description
        ));
    }
    if plan.steps.is_empty() {
        lines.push("  (no steps produced)".into());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_plan_json() {
        let text = r#"{"intent":"fix wifi","steps":[{"description":"check link","tool":"iw dev","resource":"wifi0","risk":"read-only"}]}"#;
        let plan = parse_plan(text, "fallback").expect("parse");
        assert_eq!(plan.intent, "fix wifi");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].tool.as_deref(), Some("iw dev"));
        assert_eq!(plan.steps[0].risk, "read-only");
        assert!(plan.freeform.is_none());
    }

    #[test]
    fn extracts_json_from_prose() {
        let text = "Here is my plan:\n{\"intent\":\"x\",\"steps\":[{\"description\":\"d\"}]}\nHope that helps";
        let plan = parse_plan(text, "fallback").expect("parse");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].description, "d");
        assert_eq!(plan.intent, "x");
    }

    #[test]
    fn missing_intent_falls_back() {
        let text = r#"{"steps":[{"description":"d"}]}"#;
        let plan = parse_plan(text, "fallback").expect("parse");
        assert_eq!(plan.intent, "fallback");
        assert_eq!(plan.steps[0].description, "d");
    }

    #[test]
    fn unknown_risk_defaults_to_read_only() {
        let text = r#"{"steps":[{"description":"d","risk":"explosive"}]}"#;
        let plan = parse_plan(text, "fallback").expect("parse");
        assert_eq!(plan.steps[0].risk, "read-only");
    }

    #[test]
    fn garbage_becomes_freeform() {
        let plan = parse_plan("no json here at all", "fallback").expect("parse");
        assert_eq!(plan.steps.len(), 0);
        assert_eq!(plan.freeform.as_deref(), Some("no json here at all"));
    }

    #[test]
    fn empty_steps_still_parses() {
        let plan = parse_plan(r#"{"intent":"i"}"#, "fallback").expect("parse");
        assert_eq!(plan.intent, "i");
        assert!(plan.steps.is_empty());
        assert!(plan.freeform.is_none());
    }
}

#[cfg(test)]
mod strip_think_tests {
    use super::strip_think;

    #[test]
    fn removes_think_block() {
        assert_eq!(
            strip_think("<think>let me reason</think>Answer here"),
            "Answer here"
        );
    }

    #[test]
    fn no_think_block_unchanged() {
        assert_eq!(strip_think("plain answer"), "plain answer");
    }

    #[test]
    fn strips_leading_think_only() {
        assert_eq!(
            strip_think("<think>x</think>first<think>y</think>second"),
            "firstsecond"
        );
    }
}

#[cfg(test)]
mod tool_calls_tests {
    use super::*;

    #[test]
    fn parses_openai_function_shape() {
        let text = r#"Let me check. {"tool_calls":[{"id":"call_1","type":"function","function":{"name":"observe","arguments":"wifi0"}}]}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "observe");
        assert_eq!(calls[0].arguments, "wifi0");
    }

    #[test]
    fn parses_simple_tool_shape() {
        let calls = parse_tool_calls(r#"{"tool_calls":[{"tool":"health","args":""}]}"#);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "health");
        assert_eq!(calls[0].arguments, "");
    }

    #[test]
    fn unwraps_json_argument_object() {
        let text = r#"{"tool_calls":[{"function":{"name":"query","arguments":"{\"target\":\"device\"}"}}]}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls[0].name, "query");
        assert_eq!(calls[0].arguments, "device");
    }

    #[test]
    fn no_tool_calls_means_empty() {
        assert!(parse_tool_calls("hello from stub").is_empty());
        assert!(parse_tool_calls("no json").is_empty());
        assert!(parse_tool_calls(r#"{"answer":"42"}"#).is_empty());
    }

    #[test]
    fn multiple_calls_parsed_in_order() {
        let text = r#"{"tool_calls":[
            {"function":{"name":"observe","arguments":"wifi0"}},
            {"function":{"name":"health","arguments":""}}
        ]}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "observe");
        assert_eq!(calls[1].name, "health");
    }

    #[test]
    fn strips_trailing_json_object() {
        assert_eq!(
            strip_tool_calls_json("scanning... {\"tool_calls\":[]}"),
            "scanning... "
        );
        assert_eq!(
            strip_tool_calls_json("{\"tool_calls\":[{\"tool\":\"health\"}]}"),
            ""
        );
        assert_eq!(strip_tool_calls_json("plain answer"), "plain answer");
    }

    #[test]
    fn extracts_balanced_json_from_noisy_output() {
        let text = r#"<think>ignore {not json}</think>
            Here is the plan: {"intent":"check","steps":[]}
            trailing note with {unrelated}"#;
        assert_eq!(
            extract_json(text).as_deref(),
            Some(r#"{"intent":"check","steps":[]}"#)
        );
    }

    #[test]
    fn repairs_trailing_comma_before_object_close() {
        let text = r#"Here you go: {"intent":"check","steps":[],}
            afterthought"#;
        assert_eq!(
            extract_json(text).as_deref(),
            Some(r#"{"intent":"check","steps":[]}"#)
        );
    }

    #[test]
    fn repairs_trailing_comma_before_array_close() {
        let text = r#"{"steps":[1,2,],"extra":"x"}"#;
        assert_eq!(
            extract_json(text).as_deref(),
            Some(r#"{"steps":[1,2],"extra":"x"}"#)
        );
    }

    #[test]
    fn repair_ignores_commas_inside_strings() {
        let text = r#"{"note":"a,b,","steps":[1,],}"#;
        assert_eq!(
            extract_json(text).as_deref(),
            Some(r#"{"note":"a,b,","steps":[1]}"#)
        );
    }
}
