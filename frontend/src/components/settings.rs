use dioxus::prelude::*;

#[component]
pub fn SettingsPanel() -> Element {
    let mut providers: String = use_signal(|| String::new());
    let mut api_keys: String = use_signal(|| String::new());
    let mut models: String = use_signal(|| String::new());

    rsx! {
        div {
            class: "bg-surface-200 rounded-lg p-4 border border-surface-300/30",
            h3 {
                class: "text-sm font-semibold text-gray-300 mb-3",
                "Settings"
            }
            div {
                class: "flex flex-col gap-3",
                div {
                    label {
                        class: "text-xs text-gray-400",
                        "Providers"
                    }
                    textarea {
                        class: "bg-surface-300 border border-surface-300/50 rounded px-2 py-1 text-xs text-white focus:outline-none focus:border-primary-400",
                        rows: "3",
                        value: "{providers}",
                        oninput: move |e| providers.set(e.value),
                        placeholder: "Enter provider names (comma-separated)",
                    }
                }
                div {
                    label {
                        class: "text-xs text-gray-400",
                        "API Keys"
                    }
                    textarea {
                        class: "bg-surface-300 border border-surface-300/50 rounded px-2 py-1 text-xs text-white focus:outline-none focus:border-primary-400",
                        rows: "3",
                        value: "{api_keys}",
                        oninput: move |e| api_keys.set(e.value),
                        placeholder: "Enter API keys (comma-separated)",
                    }
                }
                div {
                    label {
                        class: "text-xs text-gray-400",
                        "Default Models"
                    }
                    textarea {
                        class: "bg-surface-300 border border-surface-300/50 rounded px-2 py-1 text-xs text-white focus:outline-none focus:border-primary-400",
                        rows: "3",
                        value: "{models}",
                        oninput: move |e| models.set(e.value),
                        placeholder: "Enter default models per task type",
                    }
                }
                button {
                    class: "mt-2 px-3 py-1 bg-primary-500 text-white rounded text-xs hover:bg-primary-600 transition-colors",
                    "Save Settings"
                }
            }
        }
    }
}