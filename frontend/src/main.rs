mod app;
mod types;
mod ipc;

fn main() {
    let cfg = dioxus_desktop::Config::default();
    dioxus_desktop::launch::launch(app::app, vec![], cfg);
}