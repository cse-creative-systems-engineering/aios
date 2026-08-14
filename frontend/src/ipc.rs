use serde::{Deserialize, Serialize};
use crate::types::GenerativeWidget;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalItem {
    pub approval_id: String,
    pub tool_name: String,
    pub risk_level: String,
    pub resources: Vec<String>,
    pub summary: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PromptResponse {
    pub response: String,
    pub widgets: Vec<GenerativeWidget>,
    pub approvals: Vec<ApprovalItem>,
}

/// Mock implementation for testing UI behavior
/// In a full implementation, this would call Tauri IPC to the backend
pub async fn submit_prompt(prompt_text: String) -> Result<PromptResponse, String> {
    // Simulate network delay
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Mock responses based on keyword
    let lower = prompt_text.to_lowercase();
    if lower.contains("system") || lower.contains("info") {
        Ok(PromptResponse {
            response: "System information snapshot loaded.".to_string(),
            widgets: vec![
                GenerativeWidget::MetricCard {
                    label: "CPU Usage".to_string(),
                    value: "42".to_string(),
                    unit: Some("%".to_string()),
                    status: Some("Healthy".to_string()),
                },
                GenerativeWidget::MetricCard {
                    label: "Memory".to_string(),
                    value: "8.2".to_string(),
                    unit: Some("GB / 16 GB".to_string()),
                    status: Some("Healthy".to_string()),
                },
                GenerativeWidget::SensorGauge {
                    label: "Disk Usage".to_string(),
                    value: 47.8,
                    min: Some(0.0),
                    max: Some(100.0),
                    unit: Some("%".to_string()),
                },
            ],
            approvals: vec![],
        })
    } else {
        Ok(PromptResponse {
            response: "Processing your request... Try asking for 'system' info.".to_string(),
            widgets: vec![],
            approvals: vec![],
        })
    }
}

#[allow(dead_code)]
pub async fn get_approval_queue() -> Result<Vec<ApprovalItem>, String> {
    // Mock implementation - will be used in future approval queue feature
    Ok(vec![])
}

#[allow(dead_code)]
pub async fn respond_to_approval(approval_id: String, approved: bool) -> Result<String, String> {
    // Mock implementation - will be used in future approval queue feature
    Ok(format!(
        "Approval {} {}",
        approval_id,
        if approved { "approved" } else { "denied" }
    ))
}