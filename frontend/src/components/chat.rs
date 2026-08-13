use dioxus::prelude::*;

#[component]
pub fn ChatMessages() -> Element {
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