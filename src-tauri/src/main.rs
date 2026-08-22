use aios::facade::Facade;
use aios::graph::NodeType;
use aios::progress::{GraphActivity, GraphPhase, ProgressReporter};
#[cfg(target_os = "linux")]
use gdk::prelude::*;
#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use gtk::cairo::{RectangleInt, Region};
#[cfg(target_os = "linux")]
use gtk_layer_shell::{Edge, Layer, LayerShell};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
#[cfg(target_os = "linux")]
use tauri::AppHandle;
use tauri::{LogicalPosition, Manager, PhysicalPosition, PhysicalSize, Position, Size};
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
    sidebar_status: Arc<Mutex<Option<SidebarStatusResponse>>>,
    graph_snapshot: Arc<Mutex<Option<SystemGraphSnapshot>>>,
    app: tauri::AppHandle,
}

struct TauriProgressReporter {
    handle: tauri::AppHandle,
}

impl ProgressReporter for TauriProgressReporter {
    fn report(&self, activity: GraphActivity) {
        use tauri::Emitter;
        if let Err(error) = self.handle.emit("graph_activity", activity) {
            eprintln!("Aios graph: failed to emit activity: {error}");
        }
    }
}

fn emit_graph_activity(handle: &tauri::AppHandle, phase: GraphPhase, active_node_ids: &[&str]) {
    use tauri::Emitter;
    let activity = GraphActivity {
        phase,
        active_node_ids: active_node_ids.iter().map(|id| (*id).to_string()).collect(),
        timestamp_ms: aios::progress::now_ms(),
    };
    if let Err(error) = handle.emit("graph_activity", activity) {
        eprintln!("Aios graph: failed to emit activity: {error}");
    }
}

enum BackendRequest {
    Prompt {
        prompt: String,
        response: mpsc::Sender<Result<PromptResponse, String>>,
    },
    CloseSurface {
        id: String,
        response: mpsc::Sender<Result<(), String>>,
    },
    AddProvider {
        id: String,
        kind: String,
        tier: String,
        endpoint: Option<String>,
        model: Option<String>,
        api_key: Option<String>,
        http_timeout_ms: Option<u64>,
        response: mpsc::Sender<Result<(), String>>,
    },
    RemoveProvider {
        id: String,
        response: mpsc::Sender<Result<(), String>>,
    },
    SetProviderCredential {
        id: String,
        api_key: String,
        response: mpsc::Sender<Result<(), String>>,
    },
    SetRoleAssignment {
        role: String,
        provider_id: String,
        model: String,
        response: mpsc::Sender<Result<(), String>>,
    },
    SetRoleGroupAssignment {
        group: String,
        provider_id: String,
        model: String,
        response: mpsc::Sender<Result<Vec<String>, String>>,
    },
    RoleRoute {
        role: String,
        response: mpsc::Sender<Result<Option<SidebarRoute>, String>>,
    },
    DiscoverModels {
        provider_id: String,
        response: mpsc::Sender<Result<Vec<DiscoveredModel>, String>>,
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
    /// A surface authored by the separate groundless surface model after
    /// passing the value-fidelity gate. `None` means nothing may be displayed.
    experimental_html: Option<SurfaceCard>,
    backend_status: BackendStatus,
}

/// One generated surface living on the canvas. Several coexist; each is a
/// self-contained HTML fragment the frontend hosts, drags, and measures.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceCard {
    id: String,
    html: String,
}

fn next_surface_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("surface-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceItem {
    tool: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidebarStatusResponse {
    backend_status: BackendStatus,
    connectivity: String,
    current_route: Option<SidebarRoute>,
    chat_route: Option<SidebarRoute>,
    route_error: Option<String>,
    local_model: Option<String>,
    providers: Vec<SidebarProvider>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidebarRoute {
    provider: String,
    model: String,
    connectivity: String,
    data_classification: String,
    reduced_confidence: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidebarProvider {
    id: String,
    kind: String,
    model: String,
    tier: String,
    capabilities: Vec<String>,
    health: String,
    last_checked: u64,
    latency_ms: Option<u32>,
    error_rate: f64,
    retry_after: Option<u64>,
    credential_configured: bool,
    consent_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemGraphSnapshot {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    total_nodes: usize,
    health_counts: Vec<(String, usize)>,
    phase: String,
    active_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNode {
    id: String,
    label: String,
    layer: String,
    node_type: String,
    health: String,
    active: bool,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphEdge {
    from: String,
    to: String,
    edge_type: String,
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
fn hide_canvas(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("canvas")
        .ok_or_else(|| "canvas window is unavailable".to_string())?;
    window
        .hide()
        .map_err(|error| format!("cannot hide canvas: {error}"))
}

/// Set the native input shape of the canvas window so only the widget region
/// captures clicks. Everything outside the rectangle passes through to apps
/// behind the transparent work-area overlay.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[tauri::command]
#[cfg(target_os = "linux")]
fn set_input_region(app: AppHandle, regions: Vec<InputRect>) -> Result<(), String> {
    let window = app
        .get_webview_window("canvas")
        .ok_or_else(|| "canvas window is unavailable".to_string())?;
    let gtk_window = window
        .gtk_window()
        .map_err(|e| format!("native window unavailable: {e}"))?;
    let gdk_window = gtk_window
        .window()
        .ok_or_else(|| "canvas has no GDK window".to_string())?;
    let region = Region::create();
    for rect in &regions {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            continue;
        }
        let rectangle = RectangleInt::new(
            rect.x.round() as i32,
            rect.y.round() as i32,
            rect.w.round().max(1.0) as i32,
            rect.h.round().max(1.0) as i32,
        );
        // A rectangle outside the window bounds is not an error here; GDK
        // clips it, and the frontend only ever reports measured surfaces.
        region.union(&Region::create_rectangle(&rectangle)).ok();
    }
    gdk_window.input_shape_combine_region(&region, 0, 0);

    eprintln!(
        "Aios canvas: input region set to {} rect(s)",
        regions.iter().filter(|r| r.w > 0.0 && r.h > 0.0).count()
    );
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn set_input_region(_app: AppHandle, _regions: Vec<InputRect>) -> Result<(), String> {
    Ok(())
}

/// Drop one generated surface from the live set. The frontend hides the
/// canvas window itself once the list runs empty.
#[tauri::command]
async fn close_surface(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::CloseSurface {
                id,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
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
fn sidebar_status(
    state: tauri::State<'_, AppState>,
) -> Result<SidebarStatusResponse, String> {
    state
        .sidebar_status
        .lock()
        .map_err(|_| "sidebar status lock is poisoned".to_string())?
        .clone()
        .ok_or_else(|| "sidebar status is not ready".to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogProviderEntry {
    id: String,
    label: String,
    endpoint: String,
    kind: String,
    tier: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveredModel {
    id: String,
    name: Option<String>,
}

#[tauri::command]
fn provider_catalog() -> Vec<CatalogProviderEntry> {
    aios::coordinator::PROVIDER_CATALOG
        .iter()
        .map(|p| CatalogProviderEntry {
            id: p.id.into(),
            label: p.label.into(),
            endpoint: p.endpoint.into(),
            kind: p.kind.into(),
            tier: p.tier.into(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoleDescriptorEntry {
    id: String,
    label: String,
    detail: String,
    fit: String,
}

#[tauri::command]
fn roles_catalog() -> Vec<RoleDescriptorEntry> {
    aios::coordinator::assignable_roles()
        .into_iter()
        .map(|r| RoleDescriptorEntry {
            id: r.id,
            label: r.label,
            detail: r.detail,
            fit: r.fit,
        })
        .collect()
}

#[tauri::command]
async fn discover_models(
    provider_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiscoveredModel>, String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::DiscoverModels {
                provider_id,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
fn system_graph(
    state: tauri::State<'_, AppState>,
) -> Result<SystemGraphSnapshot, String> {
    state
        .graph_snapshot
        .lock()
        .map_err(|_| "graph snapshot lock is poisoned".to_string())?
        .clone()
        .ok_or_else(|| "graph snapshot is not ready".to_string())
}

// ---- Settings panel commands ----
//
// The panel edits typed settings through these commands; it does not route
// model requests or hold provider credentials (docs/ui.md). API keys are
// write-only: they go in and are never returned to the frontend.

#[tauri::command]
async fn add_provider(
    id: String,
    kind: String,
    tier: String,
    endpoint: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    http_timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::AddProvider {
                id,
                kind,
                tier,
                endpoint,
                model,
                api_key,
                http_timeout_ms,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
async fn remove_provider(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::RemoveProvider {
                id,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
async fn set_provider_credential(
    id: String,
    api_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::SetProviderCredential {
                id,
                api_key,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
async fn set_role_assignment(
    role: String,
    provider_id: String,
    model: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::SetRoleAssignment {
                role,
                provider_id,
                model,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
async fn set_role_group_assignment(
    group: String,
    provider_id: String,
    model: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::SetRoleGroupAssignment {
                group,
                provider_id,
                model,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
}

#[tauri::command]
async fn role_route(
    role: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<SidebarRoute>, String> {
    let requests = state.requests.clone();
    tokio::task::spawn_blocking(move || {
        let (response_tx, response_rx) = mpsc::channel();
        requests
            .send(BackendRequest::RoleRoute {
                role,
                response: response_tx,
            })
            .map_err(|_| "backend worker is unavailable".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "backend worker closed the response channel".to_string())?
    })
    .await
    .map_err(|error| format!("settings worker failed: {error}"))?
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
            if payload.experimental_html.is_some() {
                if let Err(error) = set_input_region(app.clone(), Vec::new()) {
                    eprintln!("Aios canvas: failed to clear input region: {error}");
                }
                if let Some(window) = app.get_webview_window("canvas") {
                    // Showing first keeps hidden WebKitGTK views from missing
                    // the event listener. The input region is empty until the
                    // frontend measures the generated surface.
                    let _ = window.show();
                }
            }
            use tauri::Emitter;
            if let Err(error) = app.emit_to("canvas", "canvas_response", payload) {
                eprintln!("Aios canvas: failed to emit response: {error}");
            }
        }
        response
    })
    .await
    .map_err(|error| format!("prompt worker failed: {error}"))?
}

fn main() {    #[cfg(target_os = "linux")]
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
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let work_area = monitor.work_area();
                    let sidebar_right = monitor.position().x.saturating_add(476);
                    let canvas_x = work_area.position.x.max(sidebar_right);
                    let work_area_right = work_area
                        .position
                        .x
                        .saturating_add(work_area.size.width as i32);
                    let canvas_width = work_area_right
                        .saturating_sub(canvas_x)
                        .max(1) as u32;
                    let _ = window.set_position(Position::Physical(PhysicalPosition {
                        x: canvas_x,
                        y: work_area.position.y,
                    }));
                    let _ = window.set_size(Size::Physical(PhysicalSize {
                        width: canvas_width,
                        height: work_area.size.height,
                    }));
                    eprintln!(
                        "Aios canvas: work area=({}, {}) {}x{}, canvas=({}, {}) {}x{}",
                        work_area.position.x,
                        work_area.position.y,
                        work_area.size.width,
                        work_area.size.height,
                        canvas_x,
                        work_area.position.y,
                        canvas_width,
                        work_area.size.height
                    );
                }
                // Briefly show the webview so WebKitGTK actually loads the page JS
                // (hidden webviews may defer script execution).
                let _ = window.show();
                std::thread::sleep(std::time::Duration::from_millis(200));
                let _ = window.hide();
            }

            let worker_handle = app.handle().clone();
            let (requests_tx, requests_rx) = mpsc::channel();
            let status = Arc::new(Mutex::new(BackendStatus {
                ready: false,
                error: Some("backend is starting".to_string()),
            }));
            let sidebar_status = Arc::new(Mutex::new(None));
            let graph_snapshot = Arc::new(Mutex::new(None));
            let worker_status = Arc::clone(&status);
            let worker_sidebar_status = Arc::clone(&sidebar_status);
            let worker_graph_snapshot = Arc::clone(&graph_snapshot);

            thread::spawn(move || {
                let mut facade = match Facade::boot() {
                    Ok(mut facade) => {
                        eprintln!("Aios backend: ready");
                        facade.coordinator.set_progress_reporter(Arc::new(
                            TauriProgressReporter {
                                handle: worker_handle.clone(),
                            },
                        ));
                        let next_status = BackendStatus {
                            ready: true,
                            error: None,
                        };
                        set_status(&worker_status, next_status.clone());
                        refresh_sidebar_status(&worker_sidebar_status, &facade, next_status);
                        refresh_graph_snapshot(&worker_graph_snapshot, &facade);
                        facade
                    }
                    Err(error) => {
                        let message = error.to_string();
                        eprintln!("Aios backend: boot failed: {message}");
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

                let mut surfaces: Vec<SurfaceCard> = Vec::new();
                while let Ok(request) = requests_rx.recv() {
                    match request {
                        BackendRequest::Prompt { prompt, response } => {
                            handle_prompt(&mut facade, &worker_handle, &mut surfaces, prompt, response);
                        }
                        BackendRequest::CloseSurface { id, response } => {
                            let before = surfaces.len();
                            surfaces.retain(|surface| surface.id != id);
                            if surfaces.len() == before {
                                let _ = response.send(Err(format!("no surface '{id}' is open")));
                            } else {
                                let _ = response.send(Ok(()));
                            }
                        }
                        BackendRequest::AddProvider {
                            id,
                            kind,
                            tier,
                            endpoint,
                            model,
                            api_key,
                            http_timeout_ms,
                            response,
                        } => {
                            let result = facade
                                .coordinator
                                .add_provider(id, kind, tier, endpoint, model, api_key, http_timeout_ms);
                            if let Err(error) = &result {
                                eprintln!("Aios settings: add provider failed: {error}");
                            }
                            let _ = response.send(result);
                            refresh_sidebar_status(
                                &worker_sidebar_status,
                                &facade,
                                BackendStatus { ready: true, error: None },
                            );
                        }
                        BackendRequest::RemoveProvider { id, response } => {
                            let result = facade.coordinator.remove_provider(&id);
                            if let Err(error) = &result {
                                eprintln!("Aios settings: remove provider failed: {error}");
                            }
                            let _ = response.send(result);
                            refresh_sidebar_status(
                                &worker_sidebar_status,
                                &facade,
                                BackendStatus { ready: true, error: None },
                            );
                        }
                        BackendRequest::SetProviderCredential { id, api_key, response } => {
                            let result = facade.coordinator.set_provider_credential(&id, api_key);
                            if let Err(error) = &result {
                                eprintln!("Aios settings: credential update failed: {error}");
                            }
                            let _ = response.send(result);
                            refresh_sidebar_status(
                                &worker_sidebar_status,
                                &facade,
                                BackendStatus { ready: true, error: None },
                            );
                        }
                        BackendRequest::SetRoleAssignment { role, provider_id, model, response } => {
                            let result =
                                facade
                                    .coordinator
                                    .set_role_assignment(&role, &provider_id, &model);
                            if let Err(error) = &result {
                                eprintln!("Aios settings: role assignment failed: {error}");
                            }
                            let _ = response.send(result);
                            refresh_sidebar_status(
                                &worker_sidebar_status,
                                &facade,
                                BackendStatus { ready: true, error: None },
                            );
                        }
                        BackendRequest::SetRoleGroupAssignment { group, provider_id, model, response } => {
                            let result = facade.coordinator.set_role_group_assignment(
                                &group,
                                &provider_id,
                                &model,
                            );
                            if let Err(error) = &result {
                                eprintln!("Aios settings: group assignment failed: {error}");
                            }
                            let _ = response.send(result);
                            refresh_sidebar_status(
                                &worker_sidebar_status,
                                &facade,
                                BackendStatus { ready: true, error: None },
                            );
                        }
                        BackendRequest::RoleRoute { role, response } => {
                            let result = facade
                                .coordinator
                                .role_route(&role)
                                .map(|option| option.map(|route| SidebarRoute {
                                    provider: route.provider.to_string(),
                                    model: route.model.to_string(),
                                    connectivity: format!("{:?}", route.connectivity_state),
                                    data_classification: format!("{:?}", route.data_classification),
                                    reduced_confidence: route.reduced_confidence,
                                }));
                            let _ = response.send(result);
                        }
                        BackendRequest::DiscoverModels { provider_id, response } => {
                            let result = facade
                                .coordinator
                                .discover_models(&provider_id)
                                .map(|models| models.into_iter().map(|m| DiscoveredModel {
                                    id: m.id,
                                    name: m.name,
                                }).collect());
                            if let Err(error) = &result {
                                eprintln!("Aios settings: model discovery failed: {error}");
                            }
                            let _ = response.send(result);
                        }
                    }
                }
            });

            app.manage(AppState {
                requests: requests_tx,
                status,
                sidebar_status,
                graph_snapshot,
                app: app.handle().clone(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_provider,
            backend_status,
            discover_models,
            focus_sidebar,
            hide_canvas,
            provider_catalog,
            remove_provider,
            role_route,
            roles_catalog,
            set_provider_credential,
            set_role_assignment,
            set_role_group_assignment,
            sidebar_status,
            set_input_region,
            close_surface,
            submit_prompt,
            system_graph
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

#[allow(clippy::too_many_arguments)]
fn handle_prompt(
    facade: &mut Facade,
    worker_handle: &tauri::AppHandle,
    surfaces: &mut Vec<SurfaceCard>,
    prompt: String,
    response: mpsc::Sender<Result<PromptResponse, String>>,
) {
    let status = BackendStatus {
        ready: true,
        error: None,
    };
    let answer = facade.run_line(&prompt);
    let evidence = facade.take_tool_results();
    // Groundless generation (ADR-0007): Aios relays the prompt and specialist
    // data to the surface model, then verifies value fidelity before display.
    // There is no other surface path and no widget vocabulary. Each prompt
    // authors a fresh surface; revising an existing one is a separate,
    // explicit action, so nothing from earlier prompts leaks into this call.
    let experimental_html = if evidence.is_empty() {
        eprintln!("Aios canvas: no specialist evidence gathered; no surface");
        None
    } else {
        let gaps = aios::surface::coverage_gaps(&prompt, &evidence);
        if !gaps.is_empty() {
            eprintln!("Aios canvas: coverage gap for {}; no surface", gaps.join(", "));
            None
        } else {
            emit_graph_activity(worker_handle, GraphPhase::Composing, &["composer"]);
            match facade.compose_unconstrained_html(&prompt, &evidence, None) {
                Ok((html, routing)) => {
                    match aios::surface::verify_value_fidelity(&html, &evidence) {
                        Ok(()) => {
                            eprintln!(
                                "Aios surface: provider={} model={} bytes={}",
                                routing.provider,
                                routing.model,
                                html.len()
                            );
                            write_surface_trace(
                                &prompt,
                                &answer,
                                &evidence,
                                Some((&routing, html.len())),
                                None,
                            );
                            let card = SurfaceCard {
                                id: next_surface_id(),
                                html,
                            };
                            surfaces.push(card.clone());
                            Some(card)
                        }
                        Err(error) => {
                            eprintln!("Aios canvas: fidelity check failed: {error}");
                            write_surface_trace(
                                &prompt,
                                &answer,
                                &evidence,
                                None,
                                Some(&error),
                            );
                            None
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Aios canvas: composition failed: {error}");
                    write_surface_trace(&prompt, &answer, &evidence, None, Some(&error.to_string()));
                    None
                }
            }
        }
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
        experimental_html,
        backend_status: status,
    });
    if let Ok(ref response) = result {
        refresh_sidebar_status_from_handle(worker_handle, response.backend_status.clone());
        refresh_graph_snapshot_from_handle(worker_handle);
    }
    emit_graph_activity(worker_handle, GraphPhase::Idle, &[]);
    let _ = response.send(result);
}

fn refresh_sidebar_status_from_handle(_handle: &tauri::AppHandle, _status: BackendStatus) {
    // Placeholder: the worker loop refreshes status directly with the facade.
    // Settings mutations refresh explicitly after each command.
}

fn refresh_graph_snapshot_from_handle(_handle: &tauri::AppHandle) {
    // Placeholder: same as above.
}

fn refresh_sidebar_status(
    target: &Arc<Mutex<Option<SidebarStatusResponse>>>,
    facade: &Facade,
    backend_status: BackendStatus,
) {
    match sidebar_status_snapshot(facade, backend_status) {
        Ok(snapshot) => {
            if let Ok(mut current) = target.lock() {
                *current = Some(snapshot);
            }
        }
        Err(error) => eprintln!("Aios sidebar: status snapshot failed: {error}"),
    }
}

fn sidebar_status_snapshot(
    facade: &Facade,
    backend_status: BackendStatus,
) -> Result<SidebarStatusResponse, String> {
    let coordinator = &facade.coordinator;
    let to_sidebar_route = |route: aios::model::RoutingDecision| SidebarRoute {
        provider: route.provider.to_string(),
        model: route.model.to_string(),
        connectivity: format!("{:?}", route.connectivity_state),
        data_classification: format!("{:?}", route.data_classification),
        reduced_confidence: route.reduced_confidence,
    };
    let (current_route, route_error) = match coordinator.current_route() {
        Ok(route) => (Some(to_sidebar_route(route)), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let chat_route = match coordinator.chat_route() {
        Ok(route) => Some(to_sidebar_route(route)),
        Err(error) => {
            eprintln!("Aios sidebar: chat route unavailable: {error}");
            None
        }
    };
    let providers = coordinator
        .provider_entries()
        .into_iter()
        .map(|entry| {
            let provider_id = entry.provider.to_string();
            // A registry entry without configuration is a leftover from an
            // older remove path; report it rather than failing the whole
            // snapshot, which would freeze the settings panel on stale data.
            let Some(config) = coordinator.config.provider(&provider_id) else {
                eprintln!(
                    "Aios sidebar: skipping registry entry without configuration: {provider_id}"
                );
                return Ok(None);
            };
            let consent_scopes = coordinator
                .gateway
                .router()
                .consent_for(&entry.provider)
                .map(|consent| {
                    consent
                        .data_scope
                        .iter()
                        .map(|scope| format!("{:?}", scope))
                        .collect()
                })
                .unwrap_or_default();
            Ok(Some(SidebarProvider {
                id: provider_id,
                kind: config.kind.clone(),
                model: entry.model_id.to_string(),
                tier: format!("{:?}", entry.tier),
                capabilities: entry
                    .capabilities
                    .iter()
                    .map(|capability| format!("{:?}", capability))
                    .collect(),
                health: format!("{:?}", entry.health.state),
                last_checked: entry.health.last_checked,
                latency_ms: entry.health.latency_ms,
                error_rate: entry.health.error_rate,
                retry_after: entry.health.retry_after,
                credential_configured: config.api_key.is_some() || config.api_key_env.is_some(),
                consent_scopes,
            }))
        })
        .collect::<Result<Vec<Option<_>>, String>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(SidebarStatusResponse {
        backend_status,
        connectivity: format!("{:?}", coordinator.connectivity()),
        current_route,
        chat_route,
        route_error,
        local_model: coordinator
            .local_model_path()
            .map(|path| path.display().to_string()),
        providers,
    })
}

fn refresh_graph_snapshot(
    target: &Arc<Mutex<Option<SystemGraphSnapshot>>>,
    facade: &Facade,
) {
    let snapshot = build_graph_snapshot(facade);
    if let Ok(mut current) = target.lock() {
        *current = Some(snapshot);
    }
}

fn build_graph_snapshot(facade: &Facade) -> SystemGraphSnapshot {
    let coordinator = &facade.coordinator;
    // Take the panel snapshot before holding the graph read guard; panel::snapshot
    // reads the same lock internally.
    let panel = aios::panel::snapshot(coordinator);
    let graph = coordinator.graph.read().expect("graph lock");

    let health_for_id = |id: &str| {
        graph
            .get_node(&aios::graph::NodeId(id.to_string()))
            .map(|node| format!("{:?}", node.health))
            .unwrap_or_else(|| "Unknown".into())
    };

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Orchestration layer
    nodes.push(GraphNode {
        id: "facade".into(),
        label: "Facade".into(),
        layer: "orchestration".into(),
        node_type: "Facade".into(),
        health: health_for_id("facade"),
        active: false,
        detail: "Entry point".into(),
    });
    nodes.push(GraphNode {
        id: "coordinator".into(),
        label: "Coordinator".into(),
        layer: "orchestration".into(),
        node_type: "Coordinator".into(),
        health: health_for_id("coordinator"),
        active: false,
        detail: format!("{} graph nodes", graph.nodes().len()),
    });

    edges.push(GraphEdge {
        from: "facade".into(),
        to: "coordinator".into(),
        edge_type: "orchestrates".into(),
    });

    // Agent layer
    let planner_nodes = graph.get_nodes_by_type(NodeType::PlannerAgent);
    let verifier_nodes = graph.get_nodes_by_type(NodeType::VerificationAgent);
    let guardian_nodes = graph.get_nodes_by_type(NodeType::Guardian);

    let node_health = |nodes: &[aios::graph::NodeMetadata]| -> String {
        nodes
            .first()
            .filter(|node| node.health != aios::protocol::HealthState::Unknown)
            .or_else(|| nodes.iter().find(|node| node.health != aios::protocol::HealthState::Unknown))
            .map(|n| format!("{:?}", n.health))
            .unwrap_or_else(|| "Unknown".into())
    };

    nodes.push(GraphNode {
        id: "planner".into(),
        label: "Planner".into(),
        layer: "agent".into(),
        node_type: "PlannerAgent".into(),
        health: node_health(&planner_nodes),
        active: false,
        detail: "Plan generation".into(),
    });
    nodes.push(GraphNode {
        id: "verifier".into(),
        label: "Verifier".into(),
        layer: "agent".into(),
        node_type: "VerificationAgent".into(),
        health: node_health(&verifier_nodes),
        active: false,
        detail: "Plan review".into(),
    });
    nodes.push(GraphNode {
        id: "broker".into(),
        label: "Broker".into(),
        layer: "agent".into(),
        node_type: "Broker".into(),
        health: health_for_id("broker"),
        active: false,
        detail: "Policy enforcement".into(),
    });
    nodes.push(GraphNode {
        id: "guardian".into(),
        label: "Guardian".into(),
        layer: "agent".into(),
        node_type: "Guardian".into(),
        health: guardian_nodes
            .first()
            .map(|node| format!("{:?}", node.health))
            .unwrap_or_else(|| health_for_id("guardian")),
        active: false,
        detail: "Invariant enforcement".into(),
    });

    for agent in &["planner", "verifier", "broker"] {
        edges.push(GraphEdge {
            from: "coordinator".into(),
            to: (*agent).into(),
            edge_type: "calls".into(),
        });
    }
    edges.push(GraphEdge {
        from: "broker".into(),
        to: "guardian".into(),
        edge_type: "consults".into(),
    });

    // Model gateway layer
    let provider_nodes = graph.get_nodes_by_type(NodeType::InternetProvider);
    let local_nodes = graph.get_nodes_by_type(NodeType::LocalModel);
    let lan_nodes = graph.get_nodes_by_type(NodeType::LanGateway);

    nodes.push(GraphNode {
        id: "gateway".into(),
        label: "Gateway".into(),
        layer: "model".into(),
        node_type: "ModelGateway".into(),
        health: health_for_id("gateway"),
        active: false,
        detail: format!(
            "{} providers",
            provider_nodes.len() + local_nodes.len() + lan_nodes.len()
        ),
    });

    for agent in &["planner", "verifier"] {
        edges.push(GraphEdge {
            from: (*agent).into(),
            to: "gateway".into(),
            edge_type: "calls".into(),
        });
    }

    // Provider sub-nodes (max 3 to fit layout)
    let mut provider_iter = provider_nodes.iter().chain(local_nodes.iter()).chain(lan_nodes.iter());
    for (i, provider) in provider_iter.by_ref().take(3).enumerate() {
        let pid = format!("provider:{i}");
        nodes.push(GraphNode {
            id: pid.clone(),
            label: provider.label.clone(),
            layer: "model".into(),
            node_type: format!("{:?}", provider.node_type),
            health: format!("{:?}", provider.health),
            active: false,
            detail: provider
                .attributes
                .get("model")
                .cloned()
                .unwrap_or_default(),
        });
        edges.push(GraphEdge {
            from: "gateway".into(),
            to: pid,
            edge_type: "routes".into(),
        });
    }

    // Specialists
    let specialist_defs: &[(&str, &str, &str)] = &[
        ("wifi", "WiFi", "wifi.specialist"),
        ("storage", "Store", "storage.specialist"),
        ("network", "Net", "network.specialist"),
        ("drivers", "Drv", "drivers.specialist"),
        ("graphics", "GFX", "graphics.specialist"),
        ("memory", "Mem", "memory.specialist"),
        ("power", "Pwr", "power.specialist"),
        ("processes", "Proc", "processes.specialist"),
        ("security", "Sec", "security.specialist"),
        ("boot", "Boot", "boot.specialist"),
        ("packages", "Pkg", "packages.specialist"),
    ];

    let specialist_nodes = graph.get_nodes_by_type(NodeType::Specialist);

    for (id, label, package_id) in specialist_defs {
        let matching = specialist_nodes.iter().find(|n| {
            n.node_id.0.starts_with(&format!("specialist:{id}:"))
                || n.label.contains(package_id)
                || n.attributes.values().any(|v| v.contains(package_id))
        });
        let (health, detail) = if let Some(node) = matching {
            (
                format!("{:?}", node.health),
                format!("{}: {:?}", package_id, node.health),
            )
        } else {
            ("Unknown".into(), format!("{}: not instantiated", package_id))
        };

        nodes.push(GraphNode {
            id: (*id).into(),
            label: (*label).into(),
            layer: "specialist".into(),
            node_type: "Specialist".into(),
            health,
            active: false,
            detail,
        });

        edges.push(GraphEdge {
            from: "broker".into(),
            to: (*id).into(),
            edge_type: "routes".into(),
        });
        edges.push(GraphEdge {
            from: (*id).into(),
            to: "graph".into(),
            edge_type: "owns".into(),
        });
    }

    // Infrastructure
    nodes.push(GraphNode {
        id: "graph".into(),
        label: "Graph".into(),
        layer: "infrastructure".into(),
        node_type: "SystemGraph".into(),
        health: health_for_id("graph"),
        active: false,
        detail: format!(
            "{} nodes, {} edges",
            graph.nodes().len(),
            graph.edges().len()
        ),
    });

    // Surface pipeline
    nodes.push(GraphNode {
        id: "composer".into(),
        label: "Composer".into(),
        layer: "surface".into(),
        node_type: "SurfaceComposer".into(),
        health: health_for_id("composer"),
        active: false,
        detail: "Groundless surface generation".into(),
    });

    edges.push(GraphEdge {
        from: "coordinator".into(),
        to: "composer".into(),
        edge_type: "calls".into(),
    });
    edges.push(GraphEdge {
        from: "gateway".into(),
        to: "composer".into(),
        edge_type: "generates".into(),
    });

    for (id, label, detail) in [
        ("staged", "Stage", "Checkpointed execution"),
        ("audit", "Audit", "Append-only decision log"),
        ("tools", "Tools", "Graph query registry"),
    ] {
        nodes.push(GraphNode {
            id: id.into(),
            label: label.into(),
            layer: "infrastructure".into(),
            node_type: id.to_string(),
            health: health_for_id(id),
            active: false,
            detail: detail.into(),
        });
        edges.push(GraphEdge {
            from: "coordinator".into(),
            to: id.into(),
            edge_type: "controls".into(),
        });
    }
    for (id, label, detail) in [
        ("evidence", "Evid", "Evidence index"),
        ("validator", "Valid", "Surface value validation"),
    ] {
        nodes.push(GraphNode {
            id: id.into(),
            label: label.into(),
            layer: "surface".into(),
            node_type: id.to_string(),
            health: health_for_id(id),
            active: false,
            detail: detail.into(),
        });
    }
    edges.push(GraphEdge {
        from: "composer".into(),
        to: "evidence".into(),
        edge_type: "indexes".into(),
    });
    edges.push(GraphEdge {
        from: "evidence".into(),
        to: "validator".into(),
        edge_type: "validates".into(),
    });

    // Health counts from panel
    let health_counts: Vec<(String, usize)> = panel
        .health_counts
        .iter()
        .map(|(state, count)| (format!("{:?}", state), *count))
        .collect();

    SystemGraphSnapshot {
        nodes,
        edges,
        total_nodes: panel.total_nodes,
        health_counts,
        phase: "idle".into(),
        active_node_ids: vec![],
    }
}

fn write_surface_trace(
    prompt: &str,
    answer: &str,
    evidence: &[aios::tools::ToolResult],
    generated: Option<(&aios::model::RoutingDecision, usize)>,
    error: Option<&str>,
) {
    let Ok(path) = std::env::var("AIOS_SURFACE_TRACE") else {
        return;
    };
    let evidence = evidence
        .iter()
        .map(|result| json!({ "tool": result.tool, "text": result.text }))
        .collect::<Vec<_>>();
    let generated = generated.map(|(routing, bytes)| {
        json!({
            "source": "model",
            "validated": true,
            "provider": routing.provider.to_string(),
            "model": routing.model.to_string(),
            "html_bytes": bytes,
        })
    });
    let record = json!({
        "prompt": prompt,
        "answer": answer,
        "evidence": evidence,
        "generated": generated,
        "error": error,
    });
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(write_error) = writeln!(file, "{record}") {
                eprintln!("Aios surface trace write failed: {write_error}");
            }
        }
        Err(open_error) => eprintln!("Aios surface trace open failed: {open_error}"),
    }
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

    let Ok((connection, _screen_number)) = x11rb::connect(None) else {
        eprintln!("Aios sidebar: cannot connect to X11 for EWMH dock setup");
        return;
    };
    let screen_number = _screen_number;
    let xid: Window = x11_window.xid() as Window;
    let (work_x, work_y, work_width, work_height) =
        x11_work_area(&connection, screen_number, geometry);

    gtk_window.set_default_size(476, work_height as i32);
    gtk_window.resize(476, work_height as i32);
    if let Err(error) = window.set_position(Position::Logical(LogicalPosition {
        x: geometry.x() as f64,
        y: work_y as f64,
    })) {
        eprintln!("Aios sidebar: failed to position in work area: {error}");
    }

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

    let height = work_height.max(1);
    let start_y = work_y.max(0) as u32;
    let end_y = start_y.saturating_add(height).saturating_sub(1);
    let strut = [476_u32, 0, 0, 0, start_y, end_y, 0, 0, 0, 0, 0, 0];

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
        &[476_u32, 0, 0, 0],
    );
    let _ = connection.flush();
    eprintln!(
        "Aios sidebar: X11 dock work area=({work_x},{work_y}) {work_width}x{work_height}, strut installed"
    );
}

#[cfg(target_os = "linux")]
fn x11_work_area(
    connection: &x11rb::rust_connection::RustConnection,
    screen_number: usize,
    fallback: gdk::Rectangle,
) -> (i32, i32, u32, u32) {
    let fallback = (
        fallback.x(),
        fallback.y(),
        fallback.width().max(1) as u32,
        fallback.height().max(1) as u32,
    );
    let root = connection.setup().roots[screen_number].root;
    let Ok(atom_cookie) = connection.intern_atom(false, b"_NET_WORKAREA") else {
        return fallback;
    };
    let Ok(atom) = atom_cookie.reply() else {
        return fallback;
    };
    let Ok(property_cookie) = connection.get_property(
        false,
        root,
        atom.atom,
        AtomEnum::CARDINAL,
        0,
        4,
    ) else {
        return fallback;
    };
    let Ok(property) = property_cookie.reply() else {
        return fallback;
    };
    let values = property.value32().map(|values| values.collect::<Vec<_>>());
    let Some(values) = values else {
        return fallback;
    };
    if values.len() < 4 {
        return fallback;
    }
    (
        values[0] as i32,
        values[1] as i32,
        values[2].max(1),
        values[3].max(1),
    )
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
    gtk_window.set_exclusive_zone(476);
    // With top and bottom anchors, GTK Layer Shell assigns the height. The
    // width request is the only dimension the sidebar needs to own.
    gtk_window.set_size_request(476, -1);
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
