//! End-to-end UI test that drives the real desktop app the way a user does:
//! type a prompt in the sidebar, wait for the canvas window to open, verify
//! the generated surface, then repeat across many metric themes.
//!
//! Surfaces accumulate: every generated card stays on screen until closed,
//! so after N prompts the canvas hosts exactly N surfaces. The run finishes
//! by closing them one by one and asserting the canvas empties.
//!
//! The stub provider plays the groundless surface model: each theme gets an
//! HTML fragment whose values are marked `data-aios` and trace back to the
//! specialist evidence, so the fidelity gate stays exercised end to end. No
//! fallbacks are tolerated: a missing canvas or unmarked surface is a hard
//! failure.
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
    key: &'static str,
}

const THEMES: &[Theme] = &[
    Theme {
        prompt: "generate a surface displaying the disk usage please",
        key: "disk",
    },
    Theme {
        prompt: "generate a surface displaying the memory usage please",
        key: "memory",
    },
    Theme {
        prompt: "generate a surface displaying the cpu usage please",
        key: "cpu",
    },
    Theme {
        prompt: "generate a surface for network status please",
        key: "network",
    },
    Theme {
        prompt: "how healthy is the system graph",
        key: "health",
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

/// Environment variables that break WebKit processes when inherited from a
/// snap-packaged editor terminal. GTK_PATH is the proven killer: it makes
/// GTK load immodules from the snap's core20 glibc, which crashes against
/// the system one ("undefined symbol: __libc_pthread_init"). The others are
/// stripped as cheap insurance for GUI module paths from the same source.
const SNAP_RUNTIME_VARS: &[&str] = &[
    "GTK_PATH",
    "GTK_EXE_PREFIX",
    "GIO_MODULE_DIR",
    "GSETTINGS_SCHEMA_DIR",
    "LOCPATH",
];

fn sanitized(mut command: Command) -> Command {
    // Defense in depth for `cargo test` runs launched straight from a snap
    // editor terminal; scripts/ui-e2e.sh already unsets these shell-wide.
    for var in SNAP_RUNTIME_VARS {
        command.env_remove(var);
    }
    command
}

fn spawn_driver() -> ChildGuard {
    let child = sanitized(Command::new("tauri-driver"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tauri-driver on PATH");
    wait_for_port(4444, Duration::from_secs(15));
    ChildGuard(child)
}

fn spawn_app(config: &Path) -> ChildGuard {
    let child = sanitized(Command::new(app_binary()))
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
    expected_key: &str,
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

    let selector = format!("[data-aios-theme=\"{expected_key}\"]");
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
            if client.find(Locator::Css(&selector)).await.is_ok() {
                *canvas = Some(handle.clone());
                return;
            }
        }
        if Instant::now() > deadline {
            dump_windows(client, &windows).await;
            panic!(
                "canvas never showed a {expected_key:?} surface within {PROMPT_TIMEOUT:?}"
            );
        }
        tokio::time::sleep(POLL_PERIOD).await;
    }
}

async fn assert_surface(
    client: &Client,
    canvas: &fantoccini::wd::WindowHandle,
    theme: &Theme,
    expected_total: usize,
) {
    client.switch_to_window(canvas.clone()).await.expect("switch to canvas");

    // Surfaces accumulate: after N prompts the canvas hosts N cards.
    let surfaces = client
        .find_all(Locator::Css(".aios-surface"))
        .await
        .expect("find .aios-surface");
    assert_eq!(
        surfaces.len(),
        expected_total,
        "expected {expected_total} generated surfaces on screen, found {}",
        surfaces.len()
    );

    let legacy = client.find_all(Locator::Css(".widget-grid")).await.expect("find .widget-grid");
    assert!(
        legacy.is_empty(),
        "legacy widget-grid fallback rendered; generation did not produce a surface"
    );

    // Every hosted surface carries a close affordance.
    let close_buttons = client
        .find_all(Locator::Css("[data-close]"))
        .await
        .expect("find [data-close] buttons");
    assert_eq!(
        close_buttons.len(),
        expected_total,
        "expected {expected_total} close buttons for the open surfaces"
    );

    // The surface model must mark its values so the fidelity gate can bind
    // them to specialist evidence; the host must show those marks.
    let markers = client
        .find_all(Locator::Css("[data-aios]"))
        .await
        .expect("find data-aios markers");
    assert!(
        !markers.is_empty(),
        "theme {:?} rendered a surface with no data-aios-marked values",
        theme.prompt
    );
}

/// Close every open surface through its UI button and verify the canvas is
/// empty afterwards.
async fn close_all_surfaces(client: &Client, canvas: &fantoccini::wd::WindowHandle) {
    client.switch_to_window(canvas.clone()).await.expect("switch to canvas");
    for _ in 0..THEMES.len() {
        let button = match client.find(Locator::Css("[data-close]")).await {
            Ok(button) => button,
            Err(_) => break,
        };
        button.click().await.expect("click close button");
        tokio::time::sleep(POLL_PERIOD).await;
    }
    let remaining = client
        .find_all(Locator::Css(".surface-host"))
        .await
        .expect("find .surface-host after closing");
    assert!(
        remaining.is_empty(),
        "canvas still hosts {} surfaces after closing all of them",
        remaining.len()
    );
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

    // wait_for_port only proves the TCP listener bound; tauri-driver needs a
    // beat before its HTTP layer answers, and an instant first connect dies
    // with an incomplete-message error.
    let client = {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match ClientBuilder::native().connect(DRIVER_URL).await {
                Ok(client) => break client,
                Err(error) if attempt < 20 => {
                    eprintln!("ui_e2e: driver connect attempt {attempt} failed: {error}");
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Err(error) => panic!("connect to tauri-driver after {attempt} attempts: {error}"),
            }
        }
    };

    let sidebar = sidebar_handle(&client).await;
    let mut canvas: Option<fantoccini::wd::WindowHandle> = None;

    for (index, theme) in THEMES.iter().enumerate() {
        eprintln!("--- prompt: {:?}", theme.prompt);
        submit_and_open_surface(&client, &sidebar, &mut canvas, theme.prompt, theme.key).await;
        let canvas_handle = canvas.as_ref().expect("canvas handle discovered");
        assert_surface(&client, canvas_handle, theme, index + 1).await;
    }

    close_all_surfaces(&client, canvas.as_ref().expect("canvas handle")).await;

    client.close().await.expect("close webdriver session");
    eprintln!(
        "ui_e2e: all {} themes rendered and accumulated; surfaces closed cleanly",
        THEMES.len()
    );
}
