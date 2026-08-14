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
                
                gtk_window.init_layer_shell();
                gtk_window.set_layer(Layer::Top);
                
                // Dock to left side
                gtk_window.set_anchor(Edge::Left, true);
                gtk_window.set_anchor(Edge::Top, true);
                gtk_window.set_anchor(Edge::Bottom, true);
                
                // Set exclusive zone to reserve screen space
                gtk_window.auto_exclusive_zone_enable();
                
                // Set fixed width
                gtk_window.set_width_request(400);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
