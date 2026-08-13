use dioxus::prelude::*;

#[component]
pub fn MetricCard(label: String, value: String, unit: Option<String>, status: Option<String>) -> Element {
    let status_color = match status.as_deref() {
        Some("Healthy") => "text-accent-500",
        Some("Degraded") => "text-warning-500",
        Some("Unknown") => "text-gray-500",
        _ => "text-gray-400",
    };

    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            p {
                class: "text-xs text-gray-400 mb-1",
                "{label}"
            }
            div {
                class: "flex items-baseline gap-1",
                span {
                    class: "text-2xl font-bold text-white",
                    "{value}"
                }
                if let Some(unit) = unit {
                    span {
                        class: "text-sm text-gray-400",
                        "{unit}"
                    }
                }
            }
            if let Some(status) = status {
                p {
                    class: "text-xs {status_color} mt-1",
                    "{status}"
                }
            }
        }
    }
}

#[component]
pub fn SensorGauge(label: String, value: f64, min: Option<f64>, max: Option<f64>, unit: Option<String>) -> Element {
    let range_min = min.unwrap_or(0.0);
    let range_max = max.unwrap_or(100.0);
    let percentage = ((value - range_min) / (range_max - range_min)).clamp(0.0, 1.0) * 100.0;

    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            p {
                class: "text-xs text-gray-400 mb-2",
                "{label}"
            }
            div {
                class: "w-full bg-surface-300 rounded-full h-2 mb-1",
                div {
                    class: "bg-primary-500 h-2 rounded-full transition-all duration-300",
                    style: "width: {percentage}%",
                }
            }
            div {
                class: "flex justify-between items-baseline mt-1",
                span {
                    class: "text-xs text-gray-500",
                    "{range_min}"
                }
                span {
                    class: "text-sm font-semibold text-white",
                    "{value}{unit}"
                }
                span {
                    class: "text-xs text-gray-500",
                    "{range_max}"
                }
            }
        }
    }
}

#[component]
pub fn StatusList(title: String, items: Vec<StatusItem>) -> Element {
    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            h4 {
                class: "text-sm font-semibold text-gray-300 mb-2",
                "{title}"
            }
            div {
                class: "flex flex-col gap-1",
                for item in items {
                    div {
                        class: "flex items-center gap-2 text-xs",
                        StatusDot { status: item.status },
                        span {
                            class: "text-gray-300",
                            "{item.label}"
                        }
                        if let Some(detail) = item.detail {
                            span {
                                class: "text-gray-500 ml-auto",
                                "{detail}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn StatusDot(status: String) -> Element {
    let color = match status.as_str() {
        "Healthy" => "bg-accent-500",
        "Degraded" => "bg-warning-500",
        _ => "bg-gray-500",
    };

    rsx! {
        div {
            class: "w-2 h-2 rounded-full {color}",
        }
    }
}

#[component]
pub fn Chart(title: String, data: Vec<ChartDataPoint>, chart_type: String) -> Element {
    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            h4 {
                class: "text-sm font-semibold text-gray-300 mb-2",
                "{title}"
            }
            div {
                class: "h-32 flex items-end gap-1",
                for point in data {
                    div {
                        class: "flex-1 bg-primary-500 rounded-t",
                        style: "height: {point.value}%; min-height: 4px;",
                    }
                }
            }
        }
    }
}

#[component]
pub fn ActionForm(action_name: String, description: String, fields: Vec<FormField>, risk_level: String) -> Element {
    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            h4 {
                class: "text-sm font-semibold text-gray-300 mb-1",
                "{action_name}"
            }
            p {
                class: "text-xs text-gray-400 mb-3",
                "{description}"
            }
            div {
                class: "flex flex-col gap-2",
                for field in fields {
                    div {
                        class: "flex flex-col gap-1",
                        label {
                            class: "text-xs text-gray-400",
                            "{field.name}"
                        }
                        input {
                            class: "bg-surface-300 border border-surface-300/50 rounded px-2 py-1 text-sm text-white focus:outline-none focus:border-primary-400",
                            placeholder: "{field.placeholder}",
                        }
                    }
                }
                button {
                    class: "mt-2 px-3 py-1 bg-primary-500 text-white rounded text-xs hover:bg-primary-600 transition-colors",
                    "Execute"
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct StatusItem {
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Clone, PartialEq, Props)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Clone, PartialEq, Props)]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub placeholder: Option<String>,
    pub required: bool,
}