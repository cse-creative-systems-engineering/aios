use fantoccini::{Client, ClientBuilder, Locator};
use std::process::{Command, Child};
use std::time::{Duration, Instant};
use std::path::PathBuf;

const DRIVER_URL: &str = "http://127.0.0.1:4444";

fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() { return; }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("port {} not up", port);
}

fn app_binary() -> PathBuf {
    if let Ok(p) = std::env::var("AIOS_APP_BIN") { return PathBuf::from(p); }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src-tauri/target/debug/aios-tauri")
}

#[tokio::test]
#[ignore]
async fn file_artifact_via_ui() {
    // This test drives the real Tauri app like a user would: type natural language file prompt,
    // verify artifact card appears and file on disk.
    let ws = std::env::var("AIOS_WORKSPACE").map(PathBuf::from).unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join("workspace")
    });
    let target = ws.join("hello.py");
    let _ = std::fs::remove_file(&target);

    // Start WebDriver infrastructure like ui_e2e.rs does
    let driver = Command::new("tauri-driver").arg("--port=4444").spawn().expect("tauri-driver");
    let webkit = Command::new("WebKitWebDriver").args(["--port=4445"]).spawn().expect("webkit");
    wait_for_port(4444, Duration::from_secs(10));
    // Find stub provider helper - for file writes we don't need a model, but ui_e2e needs stub for LLM surfaces
    // For file artifact we can use the same stub approach: file writes are deterministic artifact, not LLM
    // So we don't need a model provider for this test - the artifact is generated without LLM

    // Start app with test config
    let mut config = std::env::temp_dir();
    config.push(format!("aios-file-e2e-{}.toml", std::process::id()));
    // Use stub provider if available, else empty config (file artifact doesn't need LLM)
    let stub_bin = std::env::var("CARGO_BIN_EXE_stub_provider").ok();
    let port = if let Some(stub) = stub_bin { 
        // spawn stub
        0
    } else { 0 };

    let app = Command::new(app_binary()).env("AIOS_CONFIG", &config).spawn().unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;

    let client = ClientBuilder::native().connect(DRIVER_URL).await.unwrap();
    let handles = client.windows().await.unwrap();
    println!("file_ui: handles={:?}", handles);
    for h in &handles {
        let _ = client.switch_to_window(h.clone()).await;
        let url = client.current_url().await.unwrap();
        println!("handle {:?} url {:?}", h, url);
    }

    // Find sidebar prompt
    // The sidebar is the main window; try to find #prompt
    let prompt = client.find(Locator::Css("#prompt")).await;
    match prompt {
        Ok(el) => {
            el.send_keys("create a file hello.py with hello world").await.unwrap();
            let form = client.find(Locator::Css("#prompt-form button[type='submit']")).await.unwrap();
            form.click().await.unwrap();
            println!("sent prompt");
            tokio::time::sleep(Duration::from_secs(5)).await;
            // Check file on disk
            let exists = target.exists();
            let content = std::fs::read_to_string(&target).unwrap_or("MISSING".into());
            println!("ON_DISK exists={} content={:?}", exists, content);
            assert!(exists, "hello.py should exist at {:?}", target);
            assert!(content.contains("hello"), "content should contain hello: {}", content);
            // Try to find artifact card
            let body = client.source().await.unwrap();
            println!("body len {} contains artifact={}", body.len(), body.contains("Artifact"));
            assert!(body.contains("Artifact"), "artifact card should be in source");
        }
        Err(e) => {
            println!("prompt not found: {:?}", e);
            let body = client.source().await.unwrap();
            println!("body: {}", body.chars().take(1000).collect::<String>());
            panic!("prompt element not found");
        }
    }

    let _ = client.close().await;
    let _ = client.close_window().await;
    // Kill children
    let _ = driver;
    let _ = webkit;
    let _ = app;
}
