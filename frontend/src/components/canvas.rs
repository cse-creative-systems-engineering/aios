use dioxus::prelude::*;

#[component]
pub fn Canvas() -> Element {
    rsx! {
        div {
            class: "flex-1 bg-surface-50 relative overflow-hidden",
            CanvasHeader {},
            div {
                class: "flex-1 p-4",
                p {
                    class: "text-gray-400 text-sm",
                    "Send a prompt to generate a UI panel"
                }
            }
        }
    }
}

#[component]
fn CanvasHeader() -> Element {
    rsx! {
        div {
            class: "flex items-center justify-between p-4 border-b border-surface-300/30",
            h2 {
                class: "text-sm font-semibold text-gray-400",
                "Canvas"
            }
            div {
                class: "flex gap-2",
                button {
                    class: "px-2 py-1 text-xs bg-surface-300 rounded hover:bg-surface-200 transition-colors",
                    "New Panel"
                }
            }
        }
    }
}