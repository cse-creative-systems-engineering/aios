use dioxus::prelude::*;

#[component]
pub fn ApprovalQueue() -> Element {
    let approvals: Vec<ApprovalItem> = vec![];

    rsx! {
        div {
            class: "mt-3 p-3 bg-surface-300/50 rounded-lg border border-surface-300/30",
            h3 {
                class: "text-xs font-semibold text-gray-400 mb-2",
                "Approval Queue"
            }
            if approvals.is_empty() {
                p {
                    class: "text-xs text-gray-500",
                    "No pending approvals"
                }
            } else {
                div {
                    class: "flex flex-col gap-2",
                    for item in approvals {
                        div {
                            class: "flex items-center justify-between bg-surface-200 rounded px-2 py-1",
                            span {
                                class: "text-xs text-gray-300",
                                "{item.tool_name}"
                            }
                            div {
                                class: "flex gap-1",
                                button {
                                    class: "px-2 py-0.5 text-xs bg-accent-500 text-white rounded hover:bg-accent-600",
                                    "Approve"
                                }
                                button {
                                    class: "px-2 py-0.5 text-xs bg-danger-500 text-white rounded hover:bg-danger-600",
                                    "Deny"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct ApprovalItem {
    pub tool_name: String,
    pub risk_level: String,
    pub resources: Vec<String>,
    pub summary: String,
    pub approval_id: String,
}