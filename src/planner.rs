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
    let response = gateway.submit_with_fallback(&task, &request)?;
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
        Self { gateway, max_tokens }
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
        submit(
            &self.gateway,
            AgentRole::Planner,
            messages,
            self.max_tokens,
        )
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

fn extract_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(text[start..=end].to_string())
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
        assert_eq!(strip_think("<think>x</think>first<think>y</think>second"), "firstsecond");
    }
}
