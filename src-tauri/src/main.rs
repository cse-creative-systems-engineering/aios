use aios::facade::Facade;
use aios::tools::ToolResult;
#[cfg(target_os = "linux")]
use gdk::prelude::*;
#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use gtk_layer_shell::{Edge, Layer, LayerShell};
use serde::Serialize;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
#[cfg(target_os = "linux")]
use tauri::AppHandle;
use tauri::{LogicalPosition, Manager, Position};
#[cfg(target_os = "linux")]
use x11rb::CURRENT_TIME;
#[cfg(target_os = "linux")]
use x11rb::connection::Connection;
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, InputFocus, PropMode, Window};
#[cfg(target_os = "linux")]
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

struct AppState {
    requests: mpsc::Sender<BackendRequest>,
    status: Arc<Mutex<BackendStatus>>,
    app: tauri::AppHandle,
}

enum BackendRequest {
    Prompt {
        prompt: String,
        response: mpsc::Sender<Result<PromptResponse, String>>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendStatus {
    ready: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptResponse {
    answer: String,
    evidence: Vec<EvidenceItem>,
    widgets: Vec<UiWidget>,
    backend_status: BackendStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceItem {
    tool: String,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum UiWidget {
    MetricCard {
        label: String,
        value: String,
        unit: String,
        status: String,
    },
    StatusList {
        title: String,
        items: Vec<String>,
    },
    Notice {
        title: String,
        body: String,
    },
}

#[tauri::command]
fn focus_sidebar(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("sidebar")
        .ok_or_else(|| "sidebar window is unavailable".to_string())?;
    let _ = window.set_focus();

    #[cfg(target_os = "linux")]
    {
        let gtk_window = window
            .gtk_window()
            .map_err(|_| "native sidebar window is unavailable".to_string())?;
        let gdk_window = gtk_window
            .window()
            .ok_or_else(|| "sidebar has no native surface".to_string())?;
        let x11_window = gdk_window
            .downcast_ref::<gdkx11::X11Window>()
            .ok_or_else(|| "sidebar is not using X11".to_string())?;
        let (connection, _) =
            x11rb::connect(None).map_err(|error| format!("cannot connect to X11: {error}"))?;
        connection
            .set_input_focus(InputFocus::PARENT, x11_window.xid() as Window, CURRENT_TIME)
            .map_err(|error| format!("cannot focus sidebar: {error}"))?
            .check()
            .map_err(|error| format!("X11 rejected sidebar focus: {error}"))?;
        connection
            .flush()
            .map_err(|error| format!("cannot flush sidebar focus: {error}"))?;
    }

    Ok(())
}

#[tauri::command]
fn backend_status(state: tauri::State<'_, AppState>) -> Result<BackendStatus, String> {
    state
        .status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "backend status lock is poisoned".to_string())
}

#[tauri::command]
async fn submit_prompt(
    prompt: String,
    state: tauri::State<'_, AppState>,
) -> Result<PromptResponse, String> {
    let requests = state.requests.clone();
    let app = state.app.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::Prompt {
                prompt,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        let response = response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?;
        if let Ok(ref payload) = response {
            if !payload.widgets.is_empty() {
                use tauri::Emitter;
                let _ = app.emit_to("canvas", "canvas_response", payload);
            }
        }
        response
    })
    .await
    .map_err(|error| format!("prompt worker failed: {error}"))?
}

fn main() {
    #[cfg(target_os = "linux")]
    prefer_x11_when_xwayland_is_available();

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("sidebar") {
                #[cfg(target_os = "linux")]
                {
                    if configure_sidebar_layer_shell(&window) {
                        eprintln!("Aios sidebar: GTK Layer Shell configured");
                        if let Err(error) = window.show() {
                            eprintln!("Aios sidebar: failed to show Layer Shell window: {error}");
                        }
                    } else {
                        eprintln!(
                            "Aios sidebar: Layer Shell unavailable; configuring X11 dock fallback"
                        );
                        prepare_x11_dock_window(&window);
                        let _ = window
                            .set_position(Position::Logical(LogicalPosition { x: 0.0, y: 0.0 }));
                        configure_x11_dock(&window);
                        if let Err(error) = window.show() {
                            eprintln!("Aios sidebar: failed to show X11 fallback: {error}");
                        }
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ =
                        window.set_position(Position::Logical(LogicalPosition { x: 0.0, y: 0.0 }));
                    if let Err(error) = window.show() {
                        eprintln!("Aios sidebar: failed to show window: {error}");
                    }
                }
            }
            if let Some(window) = app.get_webview_window("canvas") {
                let _ =
                    window.set_position(Position::Logical(LogicalPosition { x: 440.0, y: 48.0 }));
            }

            let (requests_tx, requests_rx) = mpsc::channel();
            let status = Arc::new(Mutex::new(BackendStatus {
                ready: false,
                error: Some("backend is starting".to_string()),
            }));
            let worker_status = Arc::clone(&status);

            thread::spawn(move || {
                let mut facade = match Facade::boot() {
                    Ok(facade) => {
                        set_status(
                            &worker_status,
                            BackendStatus {
                                ready: true,
                                error: None,
                            },
                        );
                        facade
                    }
                    Err(error) => {
                        let message = error.to_string();
                        set_status(
                            &worker_status,
                            BackendStatus {
                                ready: false,
                                error: Some(message.clone()),
                            },
                        );
                        while let Ok(BackendRequest::Prompt { response, .. }) = requests_rx.recv() {
                            let _ =
                                response.send(Err(format!("backend failed to boot: {message}")));
                        }
                        return;
                    }
                };

                while let Ok(BackendRequest::Prompt { prompt, response }) = requests_rx.recv() {
                    let status = BackendStatus {
                        ready: true,
                        error: None,
                    };
                    let answer = facade.run_line(&prompt);
                    let evidence = facade.take_tool_results();
                    let widgets = if answer.contains("failed:") {
                        Vec::new()
                    } else {
                        compile_widgets(&evidence)
                    };
                    let evidence = evidence
                        .iter()
                        .map(|result| EvidenceItem {
                            tool: result.tool.to_string(),
                            text: result.text.clone(),
                        })
                        .collect();
                    let result = Ok(PromptResponse {
                        answer,
                        evidence,
                        widgets,
                        backend_status: status,
                    });
                    let _ = response.send(result);
                }
            });

            app.manage(AppState {
                requests: requests_tx,
                status,
                app: app.handle().clone(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            backend_status,
            focus_sidebar,
            submit_prompt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

fn compile_widgets(evidence: &[ToolResult]) -> Vec<UiWidget> {
    if evidence.is_empty() {
        return Vec::new();
    }
    vec![UiWidget::StatusList {
        title: "Specialist evidence".to_string(),
        items: evidence
            .iter()
            .map(|result| format!("{}: {}", result.tool, result.text))
            .collect(),
    }]
}

#[cfg(target_os = "linux")]
fn prepare_x11_dock_window(window: &tauri::WebviewWindow) {
    if let Ok(gtk_window) = window.gtk_window() {
        gtk_window.set_type_hint(gdk::WindowTypeHint::Dock);
        gtk_window.set_skip_taskbar_hint(true);
        gtk_window.set_accept_focus(true);
        gtk_window.set_focus_on_map(true);
        // Hidden Tauri windows may not have an X11 surface yet. Realize it
        // without mapping so EWMH properties can be installed before show().
        gtk_window.realize();
    }
}

#[cfg(target_os = "linux")]
fn configure_x11_dock(window: &tauri::WebviewWindow) {
    let Ok(gtk_window) = window.gtk_window() else {
        eprintln!("Aios sidebar: cannot access GTK window for X11 dock setup");
        return;
    };
    let Some(gdk_window) = gtk_window.window() else {
        eprintln!("Aios sidebar: GTK window has no native surface for X11 dock setup");
        return;
    };
    let Some(x11_window) = gdk_window.downcast_ref::<gdkx11::X11Window>() else {
        eprintln!("Aios sidebar: native surface is not X11; skipping EWMH dock setup");
        return;
    };

    let Some(monitor) = gdk_window.display().monitor_at_window(&gdk_window) else {
        eprintln!("Aios sidebar: cannot determine the active monitor for X11 dock setup");
        return;
    };
    let geometry = monitor.geometry();
    gtk_window.set_default_size(420, geometry.height());
    gtk_window.resize(420, geometry.height());

    let Ok((connection, _screen_number)) = x11rb::connect(None) else {
        eprintln!("Aios sidebar: cannot connect to X11 for EWMH dock setup");
        return;
    };
    let xid: Window = x11_window.xid() as Window;

    let Ok(dock_cookie) = connection.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK") else {
        eprintln!("Aios sidebar: cannot resolve _NET_WM_WINDOW_TYPE_DOCK");
        return;
    };
    let Ok(dock_atom) = dock_cookie.reply() else {
        eprintln!("Aios sidebar: cannot read _NET_WM_WINDOW_TYPE_DOCK");
        return;
    };
    let Ok(window_type_cookie) = connection.intern_atom(false, b"_NET_WM_WINDOW_TYPE") else {
        eprintln!("Aios sidebar: cannot resolve _NET_WM_WINDOW_TYPE");
        return;
    };
    let Ok(window_type_atom) = window_type_cookie.reply() else {
        eprintln!("Aios sidebar: cannot read _NET_WM_WINDOW_TYPE");
        return;
    };
    let Ok(strut_cookie) = connection.intern_atom(false, b"_NET_WM_STRUT_PARTIAL") else {
        eprintln!("Aios sidebar: cannot resolve _NET_WM_STRUT_PARTIAL");
        return;
    };
    let Ok(strut_atom) = strut_cookie.reply() else {
        eprintln!("Aios sidebar: cannot read _NET_WM_STRUT_PARTIAL");
        return;
    };
    let Ok(cardinal_cookie) = connection.intern_atom(false, b"CARDINAL") else {
        eprintln!("Aios sidebar: cannot resolve CARDINAL atom");
        return;
    };
    let Ok(cardinal_atom) = cardinal_cookie.reply() else {
        eprintln!("Aios sidebar: cannot read CARDINAL atom");
        return;
    };
    let Ok(strut_legacy_cookie) = connection.intern_atom(false, b"_NET_WM_STRUT") else {
        eprintln!("Aios sidebar: cannot resolve _NET_WM_STRUT");
        return;
    };
    let Ok(strut_legacy_atom) = strut_legacy_cookie.reply() else {
        eprintln!("Aios sidebar: cannot read _NET_WM_STRUT");
        return;
    };

    let height = geometry.height().max(1) as u32;
    let start_y = geometry.y().max(0) as u32;
    let end_y = start_y.saturating_add(height).saturating_sub(1);
    let strut = [420_u32, 0, 0, 0, start_y, end_y, 0, 0, 0, 0, 0, 0];

    let _ = connection.change_property32(
        PropMode::REPLACE,
        xid,
        window_type_atom.atom,
        AtomEnum::ATOM,
        &[dock_atom.atom],
    );
    let _ = connection.change_property32(
        PropMode::REPLACE,
        xid,
        strut_atom.atom,
        cardinal_atom.atom,
        &strut,
    );
    let _ = connection.change_property32(
        PropMode::REPLACE,
        xid,
        strut_legacy_atom.atom,
        cardinal_atom.atom,
        &[420_u32, 0, 0, 0],
    );
    let _ = connection.flush();
    eprintln!("Aios sidebar: X11 dock strut installed for {height}px monitor height");
}

#[cfg(target_os = "linux")]
fn prefer_x11_when_xwayland_is_available() {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let backend = std::env::var("GDK_BACKEND").ok();

    // GNOME/Mutter commonly exposes XWayland even when the desktop session is
    // Wayland. Using it gives the fallback a real X11 coordinate space instead
    // of a compositor-centered xdg_toplevel. Respect an explicit backend when
    // the user or launcher already selected one.
    if wayland && x11 && backend.is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
        eprintln!("Aios sidebar: using XWayland for controllable dock positioning");
    }
}

#[cfg(target_os = "linux")]
fn configure_sidebar_layer_shell(window: &tauri::WebviewWindow) -> bool {
    if !gtk_layer_shell::is_supported() {
        return false;
    }

    let Ok(gtk_window) = window.gtk_window() else {
        eprintln!("Aios sidebar: Tauri did not expose the native GTK window");
        return false;
    };

    // Layer Shell is ideally initialized before GTK realizes the window. Tauri
    // may realize hidden windows during construction, however, and rejecting
    // that case leaves the sidebar centered as an ordinary Wayland toplevel.
    // Attempt the native role anyway; compositors that accept this late setup
    // can still dock it, while the result is reported below.
    if gtk_window.is_realized() {
        eprintln!(
            "Aios sidebar: GTK realized the hidden window before Layer Shell setup; attempting native configuration"
        );
    }
    gtk_window.init_layer_shell();
    gtk_window.set_namespace("aios-sidebar");
    gtk_window.set_layer(Layer::Top);
    gtk_window.set_anchor(Edge::Left, true);
    gtk_window.set_anchor(Edge::Top, true);
    gtk_window.set_anchor(Edge::Bottom, true);
    gtk_window.set_exclusive_zone(420);
    // With top and bottom anchors, GTK Layer Shell assigns the height. The
    // width request is the only dimension the sidebar needs to own.
    gtk_window.set_size_request(420, -1);
    if !gtk_window.is_layer_window() {
        eprintln!("Aios sidebar: Layer Shell setup did not create a layer surface");
        return false;
    }
    true
}

fn set_status(status: &Arc<Mutex<BackendStatus>>, next: BackendStatus) {
    if let Ok(mut current) = status.lock() {
        *current = next;
    }
}
