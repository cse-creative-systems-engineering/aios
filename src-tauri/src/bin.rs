use tauri::Manager;
#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use gtk_layer_shell::{Edge, Layer, LayerShell};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "linux")]
            {
                let window = app.get_webview_window("main").unwrap();
                let gtk_window = window.gtk_window().unwrap();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error");
}
