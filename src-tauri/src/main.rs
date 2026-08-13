use aios::capability::{Capability, Clearance, Operation, PrincipalId, ResourceId, RiskLevel};
use aios::protocol::{ToolParameters, ToolResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WidgetSpec {
    pub widget_type: String,
    pub props: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalItem {
    pub tool_name: String,
    pub risk_level: String,
    pub resources: Vec<String>,
    pub summary: String,
    pub approval_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcMessage {
    pub msg_type: String,
    pub payload: serde_json::Value,
}

#[tauri::command]
async fn submit_prompt(prompt: String) -> Result<String, String> {
    let response = format!("Echo: {}", prompt);
    Ok(response)
}

#[tauri::command]
async fn get_approval_queue() -> Result<Vec<ApprovalItem>, String> {
    Ok(vec![])
}

#[tauri::command]
async fn respond_to_approval(
    approval_id: String,
    approved: bool,
) -> Result<String, String> {
    Ok(format!(
        "Approval {} {}",
        approval_id,
        if approved { "approved" } else { "denied" }
    ))
}

#[tauri::command]
async fn get_settings() -> Result<serde_json::Value, String> {
    let settings = serde_json::json!({
        "providers": [],
        "api_keys": {},
        "default_models": {},
        "specialists": {}
    });
    Ok(settings)
}

#[tauri::command]
async fn update_settings(settings: serde_json::Value) -> Result<String, String> {
    Ok(format!("Settings updated: {}", settings))
}

#[tauri::command]
async fn get_widget_definitions() -> Result<serde_json::Value, String> {
    let definitions = serde_json::json!([
        {
            "type": "MetricCard",
            "props": ["label", "value", "unit", "status"]
        },
        {
            "type": "SensorGauge",
            "props": ["label", "value", "min", "max", "unit"]
        },
        {
            "type": "StatusList",
            "props": ["title", "items"]
        },
        {
            "type": "Chart",
            "props": ["title", "data", "chart_type"]
        },
        {
            "type": "ActionForm",
            "props": ["action_name", "description", "fields", "risk_level"]
        }
    ]);
    Ok(definitions)
}

#[tauri::command]
async fn get_system_info() -> Result<serde_json::Value, String> {
    let info = serde_json::json!({
        "version": "0.1.0",
        "status": "running",
        "specialists_loaded": 11
    });
    Ok(info)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            submit_prompt,
            get_approval_queue,
            respond_to_approval,
            get_settings,
            update_settings,
            get_widget_definitions,
            get_system_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}