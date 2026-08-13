use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    rsx! {
        div {
            class: "flex flex-col w-[15%] min-w-[200px] max-w-[400px] bg-surface-100 border-r border-surface-300/50 h-full",
            SidebarHeader {},
            div {
                class: "flex-1 overflow-y-auto p-4",
                ChatMessages {},
            },
            ChatInput {},
            ApprovalQueue {},
        }
    }
}

#[component]
fn SidebarHeader() -> Element {
    rsx! {
        div {
            class: "flex items-center justify-between p-4 border-b border-surface-300/50",
            h1 {
                class: "text-lg font-bold text-primary-300",
                "Aios"
            }
            div {
                class: "flex gap-2",
                button {
                    class: "px-2 py-1 text-xs bg-surface-300 rounded hover:bg-surface-200 transition-colors",
                    "Settings"
                }
                button {
                    class: "px-2 py-1 text-xs bg-surface-300 rounded hover:bg-surface-200 transition-colors",
                    "Minimize"
                }
            }
        }
    }
}

#[component]
fn ChatMessages() -> Element {
    rsx! {
        div {
            class: "flex flex-col gap-3",
            div {
                class: "flex flex-col gap-1",
                p {
                    class: "text-xs text-gray-500",
                    "System: Aios ready"
                }
            }
        }
    }
}

#[component]
fn ChatInput() -> Element {
    let mut input: String = use_signal(|| String::new());

    rsx! {
        div {
            class: "p-3 border-t border-surface-300/50",
            div {
                class: "flex gap-2",
                input {
                    class: "flex-1 bg-surface-300 border border-surface-300/50 rounded px-3 py-2 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-primary-400",
                    placeholder: "Ask Aios...",
                    value: "{input}",
                    oninput: move |e| input.set(e.value),
                    onkeydown: move |e| {
                        if e.key == "Enter" && !input().is_empty() {
                            let _ = input;
                        }
                    },
                }
                button {
                    class: "px-3 py-2 bg-primary-500 text-white rounded text-sm hover:bg-primary-600 transition-colors",
                    "Send"
                }
            }
        }
    }
}

#[component]
fn ApprovalQueue() -> Element {
    rsx! {
        div {
            class: "mt-3 p-3 bg-surface-300/50 rounded-lg border border-surface-300/30",
            h3 {
                class: "text-xs font-semibold text-gray-400 mb-2",
                "Approval Queue"
            }
            p {
                class: "text-xs text-gray-500",
                "No pending approvals"
            }
        }
    }
}