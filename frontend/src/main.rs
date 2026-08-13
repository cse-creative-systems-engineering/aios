mod app;
mod components;

use dioxus::prelude::*;

fn main() {
    dioxus_desktop::launch(app::App);
}