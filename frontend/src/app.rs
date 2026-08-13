use dioxus::prelude::*;

use crate::components::sidebar::Sidebar;
use crate::components::canvas::Canvas;

pub fn App() -> Element {
    rsx! {
        div {
            class: "flex h-screen w-screen overflow-hidden bg-surface-50 text-white font-sans",
            Sidebar {},
            Canvas {},
        }
    }
}