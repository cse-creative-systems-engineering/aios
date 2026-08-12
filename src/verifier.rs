use crate::model::{AgentRole, ModelGateway, ModelMessage, ModelRole};
use crate::planner::{AgentError, GeneratedPlan, submit};
use crate::protocol::VerificationVerdict;
use serde::Deserialize;
use std::sync::Arc;

pub struct Verifier {
    pub gateway: Arc<ModelGateway>,
    pub max_tokens: u32,
}

impl Verifier {
    pub fn new(gateway: Arc<ModelGateway>, max_tokens: u32) -> Self {
        Self { gateway, max_tokens }
    }

    pub fn review(&self, plan: &GeneratedPlan) -> Result<ReviewResult, AgentError> {
        let plan_text = match &plan.freeform {
            Some(freeform) => format!("intent: {}\nsteps:\n{freeform}", plan.intent),
            None => {
                let steps: Vec<String> = plan
                    .steps
                    .iter()
                    .map(|step| {
                        format!(
                            "{{description: {}, tool: {}, resource: {}, risk: {}}}",
                            step.description,
                            step.tool.as_deref().unwrap_or("-"),
                            step.resource.as_deref().unwrap_or("-"),
                            step.risk
                        )
                    })
                    .collect();
                format!("intent: {}\nsteps: [{}]", plan.intent, steps.join(", "))
            }
        };
        let system = "You are the Aios Verification agent. Review the action plan for \
                      safety, scope, and necessity. Reply with ONLY a JSON object of the \
                      form {\"verdict\": \"approve\" | \"approve_with_conditions\" | \
                      \"reject\" | \"insufficient_information\", \"concerns\": [...], \
                      \"tests\": [...]}. concerns and tests are arrays of short strings. \
                      No prose outside the JSON.";
        let text = submit(
            &self.gateway,
            AgentRole::Verification,
            vec![
                ModelMessage::new(ModelRole::System, system),
                ModelMessage::new(ModelRole::User, plan_text),
            ],
            self.max_tokens,
        )?;
        parse_review(&text)
    }
}

#[derive(Clone, Debug)]
pub struct ReviewResult {
    pub verdict: VerificationVerdict,
    pub concerns: Vec<String>,
    pub recommended_tests: Vec<String>,
    pub freeform: Option<String>,
}

#[derive(Deserialize)]
struct ReviewJson {
    verdict: Option<String>,
    #[serde(default)]
    concerns: Vec<String>,
    #[serde(default)]
    tests: Vec<String>,
}

pub fn parse_review(text: &str) -> Result<ReviewResult, AgentError> {
    let body = match extract_json(text) {
        Some(body) => body,
        None => return Ok(loose_review(text)),
    };
    match serde_json::from_str::<ReviewJson>(&body) {
        Ok(json) => {
            let verdict = match json.verdict.as_deref() {
                Some("approve") => VerificationVerdict::Approve,
                Some("approve_with_conditions") => {
                    VerificationVerdict::ApproveWithConditions(json.concerns.clone())
                }
                Some("reject") => VerificationVerdict::Reject(
                    json.concerns.first().cloned().unwrap_or_else(|| "rejected".into()),
                ),
                _ => VerificationVerdict::InsufficientInformation,
            };
            Ok(ReviewResult {
                verdict,
                concerns: json.concerns,
                recommended_tests: json.tests,
                freeform: None,
            })
        }
        Err(_) => Ok(loose_review(text)),
    }
}

fn loose_review(text: &str) -> ReviewResult {
    ReviewResult {
        verdict: VerificationVerdict::InsufficientInformation,
        concerns: Vec::new(),
        recommended_tests: Vec::new(),
        freeform: Some(text.to_string()),
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

pub fn format_review(review: &ReviewResult) -> String {
    if let Some(freeform) = &review.freeform {
        return format!("verdict: unstructured response\n{freeform}");
    }
    let verdict = match &review.verdict {
        VerificationVerdict::Approve => "approve".to_string(),
        VerificationVerdict::ApproveWithConditions(_) => {
            format!("approve with conditions: {}", review.concerns.join("; "))
        }
        VerificationVerdict::Reject(reason) => format!("reject: {reason}"),
        VerificationVerdict::InsufficientInformation => "insufficient information".to_string(),
    };
    let mut lines = vec![format!("verdict: {verdict}")];
    if !review.concerns.is_empty() {
        lines.push(format!("concerns: {}", review.concerns.join("; ")));
    }
    if !review.recommended_tests.is_empty() {
        lines.push(format!("recommended tests: {}", review.recommended_tests.join("; ")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_approve() {
        let review = parse_review(r#"{"verdict":"approve","concerns":[],"tests":["ping"]}"#).expect("parse");
        assert_eq!(review.verdict, VerificationVerdict::Approve);
        assert_eq!(review.recommended_tests, vec!["ping".to_string()]);
    }

    #[test]
    fn parses_approve_with_conditions() {
        let review = parse_review(
            r#"{"verdict":"approve_with_conditions","concerns":["backup first"],"tests":[]}"#,
        )
        .expect("parse");
        assert!(matches!(
            review.verdict,
            VerificationVerdict::ApproveWithConditions(conditions)
                if conditions == vec!["backup first".to_string()]
        ));
    }

    #[test]
    fn parses_reject() {
        let review = parse_review(r#"{"verdict":"reject","concerns":["too risky"]}"#).expect("parse");
        assert!(matches!(
            review.verdict,
            VerificationVerdict::Reject(reason) if reason == "too risky"
        ));
    }

    #[test]
    fn unknown_verdict_is_insufficient() {
        let review = parse_review(r#"{"verdict":"maybe"}"#).expect("parse");
        assert_eq!(review.verdict, VerificationVerdict::InsufficientInformation);
    }

    #[test]
    fn garbage_becomes_freeform() {
        let review = parse_review("nope").expect("parse");
        assert_eq!(review.verdict, VerificationVerdict::InsufficientInformation);
        assert_eq!(review.freeform.as_deref(), Some("nope"));
    }

    #[test]
    fn review_formats_verdict() {
        let review = ReviewResult {
            verdict: VerificationVerdict::Approve,
            concerns: Vec::new(),
            recommended_tests: vec!["ping".into()],
            freeform: None,
        };
        assert!(format_review(&review).contains("verdict: approve"));
    }
}
