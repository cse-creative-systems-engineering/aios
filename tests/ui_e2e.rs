//! End-to-end UI test that drives the real desktop app the way a user does:
//! type a prompt in the sidebar, wait for the canvas window to open, verify
//! the composed surface, close the window, repeat across many metric themes.
//!
//! No fallbacks are tolerated. The surface must appear; a legacy widget-grid
//! render or a missing canvas is a hard failure.
//!
//! Requires a display, the debug app binary, `tauri-driver`, and
//! `WebKitWebDriver` on PATH. Run via `scripts/ui-e2e.sh` (or
//! `cargo test --test ui_e2e -- --ignored` when the app is already built).

use fantoccini::{Client, ClientBuilder, Locator};
use std::io::BufRead;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const DRIVER_URL: &str = "http://127.0.0.1:4444";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_PERIOD: Duration = Duration::from_millis(200);

struct Theme {
    prompt: &'static str,
    title: &'static str,
    widget_classes: &'static [&'static str],
}

const THEMES: &[Theme] = &[
    Theme {
        prompt: "generate a surface displaying the disk usage please",
        title: "Disk health",
        widget_classes: &["status-widget", "notice-widget"],
    },
    Theme {
        prompt: "generate a surface displaying the memory usage please",
        title: "Memory",
        widget_classes: &["status-widget", "notice-widget"],
    },
    Theme {
        prompt: "generate a surface displaying the cpu usage please",
        title: "CPU",
        widget_classes: &["status-widget", "chart-widget"],
    },
    Theme {
        prompt: "generate a surface for network status please",
        title: "Network",
        widget_classes: &["status-widget", "notice-widget"],
    },
    Theme {
        prompt: "how healthy is the system graph",
        title: "System health",
        widget_classes: &["metric-widget", "gauge-widget", "status-widget"],
    },
];

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("port {port} never became reachable within {timeout:?}");
}

fn app_binary() -> PathBuf {
    if let Ok(path) = std::env::var("AIOS_APP_BIN") {
        return PathBuf::from(path);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src-tauri")
        .join("target")
        .join("debug")
        .join("aios-tauri")
}

fn write_test_config() -> PathBuf {
    let stub = env!("CARGO_BIN_EXE_stub_provider");
    let port = spawn_stub(stub);
    let mut config = std::env::temp_dir();
    config.push(format!("aios-e2e-{}.toml", std::process::id()));
    let text = format!(
        "[[provider]]\nid = \"stub\"\nkind = \"openai-compatible\"\ntier = \"internet\"\n\
         endpoint = \"http://127.0.0.1:{port}\"\nmodel = \"stub-model\"\n\
         http_timeout_ms = 5000\n\n[shell]\nmax_tokens = 1024\nhistory_len = 3\n"
    );
    std::fs::write(&config, text).expect("write test config");
    config
}

fn spawn_stub(binary: &str) -> u16 {
    let mut child = Command::new(binary)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn stub_provider");
    let stdout = child.stdout.take().expect("stub stdout");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let line = line.expect("read stub line");
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                if let Some(port) = line.rsplit(':').next().and_then(|p| p.parse().ok()) {
                    std::mem::forget(child);
                    return port;
                }
            }
            Err(_) if Instant::now() > deadline => {
                let _ = child.kill();
                panic!("stub_provider did not announce a port within 10s");
            }
            Err(_) => continue,
        }
    }
}

fn spawn_driver() -> ChildGuard {
    let child = Command::new("tauri-driver")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("tauri-driver on PATH");
    wait_for_port(4444, Duration::from_secs(15));
    ChildGuard(child)
}

fn spawn_app(config: &Path) -> ChildGuard {
    let child = Command::new(app_binary())
        .env("AIOS_CONFIG", config)
        .env("TAURI_WEBVIEW_AUTOMATION", "true")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn aios-tauri");
    ChildGuard(child)
}

async fn sidebar_handle(client: &Client) -> fantoccini::wd::WindowHandle {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let windows = client.windows().await.expect("list windows");
        eprintln!("ui_e2e: discovered {} window handles", windows.len());
        for handle in &windows {
            client.switch_to_window(handle.clone()).await.expect("switch window");
            let url = client.current_url().await.ok();
            let body = client
                .execute("return document.body ? document.body.innerText.slice(0, 160) : '(no body)';", vec![])
                .await
                .ok();
            eprintln!("ui_e2e: handle={handle:?} url={url:?} body={body:?}");
            if client.find(Locator::Css("#prompt")).await.is_ok() {
                return handle.clone();
            }
        }
        if Instant::now() > deadline {
            panic!("sidebar window with #prompt never appeared");
        }
        tokio::time::sleep(POLL_PERIOD).await;
    }
}

async fn submit_and_open_surface(
    client: &Client,
    sidebar: &fantoccini::wd::WindowHandle,
    canvas: &mut Option<fantoccini::wd::WindowHandle>,
    prompt: &str,
    expected_title: &str,
) {
    client.switch_to_window(sidebar.clone()).await.expect("switch to sidebar");
    let input = client
        .find(Locator::Css("#prompt"))
        .await
        .expect("sidebar prompt input");
    input.send_keys(prompt).await.expect("type prompt");
    client
        .execute(
            "document.getElementById('prompt-form').requestSubmit();",
            vec![],
        )
        .await
        .expect("submit prompt form");
    client
        .find(Locator::Css("#prompt"))
        .await
        .expect("prompt input after submit")
        .clear()
        .await
        .expect("clear prompt");

    let deadline = Instant::now() + PROMPT_TIMEOUT;
    loop {
        let windows = client.windows().await.expect("list windows");
        let candidates: Vec<_> = windows
            .iter()
            .filter(|handle| String::from((*handle).clone()) != String::from(sidebar.clone()))
            .cloned()
            .collect();
        for handle in &candidates {
            client.switch_to_window(handle.clone()).await.expect("switch candidate");
            if client.find(Locator::Css(".surface")).await.is_err() {
                continue;
            }
            let title = match client.find(Locator::Css(".canvas-header h1")).await {
                Ok(heading) => heading.text().await.ok(),
                Err(_) => None,
            };
            if title.as_deref() == Some(expected_title) {
                *canvas = Some(handle.clone());
                return;
            }
        }
        if Instant::now() > deadline {
            dump_windows(client, &windows).await;
            panic!(
                "canvas never showed a surface titled {expected_title:?} within {PROMPT_TIMEOUT:?}"
            );
        }
        tokio::time::sleep(POLL_PERIOD).await;
    }
}

async fn assert_surface(client: &Client, canvas: &fantoccini::wd::WindowHandle, theme: &Theme) {
    client.switch_to_window(canvas.clone()).await.expect("switch to canvas");

    let surfaces = client.find_all(Locator::Css(".surface")).await.expect("find .surface");
    assert_eq!(
        surfaces.len(),
        1,
        "expected exactly one composed surface, found {}",
        surfaces.len()
    );

    let grid = client.find_all(Locator::Css(".widget-grid")).await.expect("find .widget-grid");
    assert!(
        grid.is_empty(),
        "legacy widget-grid fallback rendered; composition did not produce a surface"
    );

    for class in theme.widget_classes {
        let selector = format!(".surface-widget.{}", class);
        let widgets = client.find_all(Locator::Css(&selector)).await.expect("find widgets");
        assert!(
            !widgets.is_empty(),
            "theme {:?} expected at least one {class}, found none",
            theme.prompt
        );
    }

    let chips = client
        .find_all(Locator::Css(".surface-widget .evidence-chip"))
        .await
        .expect("find evidence chips");
    assert!(
        !chips.is_empty(),
        "surface widgets must bind evidence; found no evidence chips"
    );
}

async fn close_canvas(client: &Client, canvas: &fantoccini::wd::WindowHandle) {
    client.switch_to_window(canvas.clone()).await.expect("switch to canvas");
    let close = client
        .find(Locator::Css("[data-close]"))
        .await
        .expect("canvas close button");
    close.click().await.expect("click canvas close");
}

async fn dump_windows(client: &Client, windows: &[fantoccini::wd::WindowHandle]) {
    eprintln!("=== window dump ({} handles) ===", windows.len());
    for handle in windows {
        if client.switch_to_window(handle.clone()).await.is_err() {
            eprintln!("  handle {handle:?}: switch failed");
            continue;
        }
        let snippet = client
            .execute("return document.body ? document.body.innerText.slice(0, 300) : '(no body)';", vec![])
            .await
            .ok()
            .map(|json| json.as_str().map(|s| s.to_string()).unwrap_or_default())
            .unwrap_or_else(|| "(execute failed)".to_string());
        eprintln!("  handle {handle:?}: {snippet:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a display, tauri-driver, WebKitWebDriver, and a built app"]
async fn user_loops_prompts_and_verifies_surfaces() {
    let config = write_test_config();
    let _driver = spawn_driver();
    let _app = spawn_app(&config);

    let client = ClientBuilder::native()
        .connect(DRIVER_URL)
        .await
        .expect("connect to tauri-driver");

    let sidebar = sidebar_handle(&client).await;
    let mut canvas: Option<fantoccini::wd::WindowHandle> = None;

    for theme in THEMES {
        eprintln!("--- prompt: {:?}", theme.prompt);
        submit_and_open_surface(&client, &sidebar, &mut canvas, theme.prompt, theme.title).await;
        let canvas_handle = canvas.as_ref().expect("canvas handle discovered");
        assert_surface(&client, canvas_handle, theme).await;
        close_canvas(&client, canvas_handle).await;
    }

    client.close().await.expect("close webdriver session");
    eprintln!("ui_e2e: all {} themes rendered a surface", THEMES.len());
}
