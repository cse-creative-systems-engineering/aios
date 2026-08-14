use dioxus::prelude::*;
use crate::types::GenerativeWidget;
use crate::ipc::{self, ApprovalItem};

pub fn app() -> Element {
    let mut messages = use_signal(|| vec!["System: Aios ready".to_string()]);
    let mut input_value = use_signal(|| String::new());
    let panels: Signal<Vec<GenerativeWidget>> = use_signal(Vec::new);
    let mut is_loading = use_signal(|| false);
    let approvals: Signal<Vec<ApprovalItem>> = use_signal(Vec::new);

    let mut submit_message = move || {
        let prompt_text = input_value.read().clone();
        if !prompt_text.is_empty() && !*is_loading.read() {
            messages.push(format!("You: {}", prompt_text));
            input_value.set(String::new());
            is_loading.set(true);

            // Spawn async task to fetch response from backend
            let msg_handle = messages.clone();
            let panel_handle = panels.clone();
            let load_handle = is_loading.clone();
            let approval_handle = approvals.clone();

            spawn_message_task(prompt_text, msg_handle, panel_handle, load_handle, approval_handle);
        }
    };

    rsx! {
        div {
            class: "flex h-screen w-screen overflow-hidden bg-slate-900 text-white font-sans",
            
            // Sidebar - 15% width for chat
            div {
                class: "flex flex-col w-[15%] min-w-[200px] max-w-[400px] bg-slate-800 border-r border-slate-700 h-full",
                
                // Header
                div {
                    class: "flex items-center justify-between p-4 border-b border-slate-700",
                    h1 { class: "text-lg font-bold text-blue-400", "Aios" }
                    div { class: "flex gap-2",
                        button { class: "px-2 py-1 text-xs bg-slate-700 rounded hover:bg-slate-600 transition", title: "Settings", "⚙" }
                        button { class: "px-2 py-1 text-xs bg-slate-700 rounded hover:bg-slate-600 transition", title: "Minimize", "−" }
                    }
                }
                
                // Approval queue - Only show if there are approvals
                if !approvals.read().is_empty() {
                    div {
                        class: "p-3 bg-yellow-900/30 rounded border border-yellow-600/50 m-3 space-y-2 max-h-32 overflow-y-auto",
                        h3 { class: "text-xs font-semibold text-yellow-400 mb-2", "⚠ Pending Approvals ({approvals.read().len()})" }
                        for (idx, approval) in approvals.read().iter().enumerate() {
                            {render_approval_item(approval.clone(), idx, approvals)}
                        }
                    }
                }
                
                // Chat messages area
                div {
                    class: "flex-1 overflow-y-auto p-4 space-y-2",
                    for msg in messages.read().iter() {
                        div {
                            class: if msg.starts_with("You:") {
                                "text-sm text-right text-blue-300"
                            } else {
                                "text-sm text-left text-gray-300"
                            },
                            "{msg}"
                        }
                    }
                    if *is_loading.read() {
                        div {
                            class: "text-sm text-left text-gray-500 italic",
                            "Aios is thinking..."
                        }
                    }
                }
                
                // Chat input
                div {
                    class: "border-t border-slate-700 p-3",
                    div {
                        class: "flex gap-2",
                        input {
                            class: "flex-1 bg-slate-700 border border-slate-600 rounded px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 disabled:opacity-50",
                            placeholder: "Ask Aios...",
                            value: "{input_value}",
                            disabled: *is_loading.read(),
                            onchange: move |evt| input_value.set(evt.value()),
                            onkeydown: move |evt| {
                                if evt.key() == Key::Enter && !*is_loading.read() {
                                    submit_message();
                                }
                            }
                        }
                        button {
                            class: "px-3 py-2 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 transition disabled:opacity-50 disabled:cursor-not-allowed",
                            disabled: *is_loading.read(),
                            onclick: move |_| submit_message(),
                            if *is_loading.read() { "..." } else { "Send" }
                        }
                    }
                }
            }
            
            // Canvas - 85% width for panels
            div {
                class: "flex-1 bg-slate-900 flex flex-col relative overflow-hidden",
                
                // Canvas header
                div {
                    class: "flex items-center justify-between p-4 border-b border-slate-700",
                    h2 { class: "text-sm font-semibold text-gray-400", "Canvas" }
                    div {
                        class: "flex gap-2",
                        button { class: "px-3 py-1 text-xs bg-slate-700 rounded hover:bg-slate-600 transition", "Clear Panels" }
                    }
                }
                
                // Canvas content area
                div {
                    class: "flex-1 p-6 overflow-auto",
                    
                    if panels.read().is_empty() {
                        div {
                            class: "flex items-center justify-center h-full text-gray-500",
                            p { "Ask Aios for system info or other requests to generate panels" }
                        }
                    } else {
                        div {
                            class: "grid grid-cols-1 lg:grid-cols-2 gap-4",
                            for (idx, widget) in panels.read().iter().enumerate() {
                                {render_widget(widget.clone(), idx)}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn spawn_message_task(
    prompt: String,
    mut messages: Signal<Vec<String>>,
    mut panels: Signal<Vec<GenerativeWidget>>,
    mut is_loading: Signal<bool>,
    mut approvals: Signal<Vec<ApprovalItem>>,
) {
    spawn(async move {
        match ipc::submit_prompt(prompt).await {
            Ok(response) => {
                messages.push(format!("Aios: {}", response.response));
                
                // Add widgets to canvas
                for widget in response.widgets {
                    panels.push(widget);
                }
                
                // Add approvals to queue
                for approval in response.approvals {
                    approvals.push(approval);
                }
            }
            Err(err) => {
                messages.push(format!("Aios: Error - {}", err));
            }
        }
        is_loading.set(false);
    });
}

fn render_approval_item(
    approval: ApprovalItem,
    _idx: usize,
    mut approvals: Signal<Vec<ApprovalItem>>,
) -> Element {
    let risk_color = match approval.risk_level.as_str() {
        "high" | "critical" => "border-red-500/50 bg-red-900/20",
        "medium" => "border-yellow-500/50 bg-yellow-900/20",
        _ => "border-green-500/50 bg-green-900/20",
    };
    
    let risk_text_color = match approval.risk_level.as_str() {
        "high" | "critical" => "text-red-400",
        "medium" => "text-yellow-400",
        _ => "text-green-400",
    };

    // Pre-clone for use in both closures
    let approval_id_1 = approval.approval_id.clone();
    let approval_id_2 = approval.approval_id.clone();
    let approval_id_filter1 = approval.approval_id.clone();
    let approval_id_filter2 = approval.approval_id.clone();

    rsx! {
        div {
            class: "p-2 rounded border {risk_color}",
            div { class: "flex items-start justify-between gap-2",
                div { class: "flex-1 min-w-0",
                    p { class: "text-xs font-semibold text-gray-200 truncate", "{approval.tool_name}" }
                    p { class: "text-xs text-gray-400 mt-0.5", "{approval.summary}" }
                    if !approval.resources.is_empty() {
                        p { class: "text-xs text-gray-500 mt-1", "Resources: {approval.resources.join(\", \")}" }
                    }
                }
                span { class: "text-xs {risk_text_color} font-semibold flex-shrink-0", "{approval.risk_level}" }
            }
            div { class: "flex gap-1 mt-2",
                button {
                    class: "flex-1 px-2 py-1 text-xs bg-green-600 hover:bg-green-700 rounded transition",
                    onclick: move |_| {
                        let id = approval_id_1.clone();
                        let filter_id = approval_id_filter1.clone();
                        spawn(async move {
                            if let Err(e) = ipc::respond_to_approval(id, true).await {
                                eprintln!("Approval error: {}", e);
                            }
                        });
                        // Remove from list by filtering
                        let current = approvals.read().clone();
                        let filtered: Vec<_> = current.into_iter()
                            .filter(|a| a.approval_id != filter_id)
                            .collect();
                        approvals.set(filtered);
                    },
                    "✓ Approve"
                }
                button {
                    class: "flex-1 px-2 py-1 text-xs bg-red-600 hover:bg-red-700 rounded transition",
                    onclick: move |_| {
                        let id = approval_id_2.clone();
                        let filter_id = approval_id_filter2.clone();
                        spawn(async move {
                            if let Err(e) = ipc::respond_to_approval(id, false).await {
                                eprintln!("Rejection error: {}", e);
                            }
                        });
                        // Remove from list by filtering
                        let current = approvals.read().clone();
                        let filtered: Vec<_> = current.into_iter()
                            .filter(|a| a.approval_id != filter_id)
                            .collect();
                        approvals.set(filtered);
                    },
                    "✗ Deny"
                }
            }
        }
    }
}

fn render_widget(widget: GenerativeWidget, _idx: usize) -> Element {
    match widget {
        GenerativeWidget::MetricCard { label, value, unit, status } => {
            let status_color = match status.as_deref() {
                Some("Healthy") => "text-green-400",
                Some("Degraded") => "text-yellow-400",
                Some("Unknown") => "text-gray-400",
                _ => "text-gray-400",
            };

            rsx! {
                div {
                    class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
                    p { class: "text-xs text-gray-400 mb-2", "{label}" }
                    div { class: "flex items-baseline gap-1",
                        span { class: "text-2xl font-bold text-white", "{value}" }
                        if let Some(u) = unit {
                            span { class: "text-sm text-gray-400", "{u}" }
                        }
                    }
                    if let Some(s) = status {
                        p { class: "text-xs {status_color} mt-2", "{s}" }
                    }
                }
            }
        },
        GenerativeWidget::SensorGauge { label, value, min, max, unit } => {
            let range_min = min.unwrap_or(0.0);
            let range_max = max.unwrap_or(100.0);
            let percentage = ((value - range_min) / (range_max - range_min)).clamp(0.0, 1.0) * 100.0;

            rsx! {
                div {
                    class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
                    p { class: "text-xs text-gray-400 mb-2", "{label}" }
                    div { class: "w-full bg-slate-700 rounded-full h-2 mb-2",
                        div {
                            class: "bg-blue-500 h-2 rounded-full transition-all duration-300",
                            style: "width: {percentage}%",
                        }
                    }
                    div { class: "flex justify-between items-baseline text-xs",
                        span { class: "text-gray-500", "{range_min}" }
                        span { class: "text-white font-semibold", "{value}{unit.as_deref().unwrap_or(\"\")}" }
                        span { class: "text-gray-500", "{range_max}" }
                    }
                }
            }
        },
        GenerativeWidget::StatusList { title, items } => {
            rsx! {
                div {
                    class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
                    h4 { class: "text-sm font-semibold text-gray-300 mb-3", "{title}" }
                    div { class: "flex flex-col gap-2",
                        for item in items {
                            div { class: "flex items-center gap-2 text-xs",
                                div {
                                    class: match item.status.as_str() {
                                        "Healthy" => "w-2 h-2 rounded-full bg-green-500",
                                        "Degraded" => "w-2 h-2 rounded-full bg-yellow-500",
                                        _ => "w-2 h-2 rounded-full bg-gray-500",
                                    },
                                }
                                span { class: "text-gray-300", "{item.label}" }
                                if let Some(detail) = item.detail {
                                    span { class: "text-gray-500 ml-auto", "{detail}" }
                                }
                            }
                        }
                    }
                }
            }
        },
        GenerativeWidget::Chart { title, data, chart_type: _chart_type } => {
            let max_value = data.iter().map(|d| d.value).fold(f64::NEG_INFINITY, f64::max);

            rsx! {
                div {
                    class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
                    h4 { class: "text-sm font-semibold text-gray-300 mb-3", "{title}" }
                    div { class: "h-32 flex items-end gap-1",
                        for point in data {
                            div {
                                class: "flex-1 bg-blue-500 rounded-t",
                                style: "height: {(point.value / max_value) * 100.0}%; min-height: 4px;",
                                title: "{point.label}",
                            }
                        }
                    }
                }
            }
        },
        GenerativeWidget::ActionForm { action_name, description, fields, risk_level: _ } => {
            rsx! {
                div {
                    class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
                    h4 { class: "text-sm font-semibold text-gray-300 mb-1", "{action_name}" }
                    p { class: "text-xs text-gray-400 mb-3", "{description}" }
                    div { class: "flex flex-col gap-2",
                        for field in fields {
                            div { class: "flex flex-col gap-1",
                                label { class: "text-xs text-gray-400", "{field.name}" }
                                input {
                                    class: "bg-slate-700 border border-slate-600 rounded px-2 py-1 text-sm text-white focus:outline-none focus:border-blue-500",
                                    placeholder: "{field.placeholder.as_deref().unwrap_or(\"\")}",
                                }
                            }
                        }
                        button { class: "mt-2 px-3 py-1 bg-blue-600 text-white rounded text-xs hover:bg-blue-700 transition", "Execute" }
                    }
                }
            }
        },
    }
}