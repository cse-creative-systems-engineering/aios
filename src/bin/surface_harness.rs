//! Headless generative-surface test harness.
//!
//! Drives the real pipeline without a display: prompt -> planner tool loop
//! (specialists read real system data) -> composer model call -> validation ->
//! text/HTML surface rendering, and writes a monitoring report per prompt.
//!
//! Usage:
//!   surface-harness [--stub] [--config PATH] [--out DIR] [prompts...]
//!
//! - `--stub` runs against a deterministic stub model server (no network).
//! - `--out DIR` writes `surface-N.json`, `surface-N.html`, and `report.json`.
//! - Prompts are positional; a canned suite runs when none are given.
//! Exit code is non-zero when any probe failed.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use aios::config::AiosConfig;
use aios::coordinator::{ChatOutcome, Coordinator};
use aios::model::{ConnectivityProbe, ConnectivityState, ModelMessage, ModelRole};
use aios::surface::{
    EvidenceIndex, Surface, SurfaceComposeError, SURFACE_VERSION, diagnostics, render_html,
    render_text, validate,
};
use aios::tools::ToolResult;

const DEFAULT_PROMPTS: &[&str] = &[
    "How much of the disk is used and is the drive healthy?",
    "What is the CPU load and how many cores are active?",
    "How much memory is in use and is there pressure?",
    "What is the state of the network interfaces?",
    "Are any processes consuming an unusual amount of CPU?",
    "What are the current system temperatures?",
    "How is the graphics card doing?",
];

struct Options {
    stub: bool,
    config: Option<PathBuf>,
    out: PathBuf,
    prompts: Vec<String>,
}

fn parse_args() -> Options {
    let mut stub = false;
    let mut config = None;
    let mut out = PathBuf::from("harness-out");
    let mut prompts = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stub" => stub = true,
            "--config" => config = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from).unwrap_or(out),
            other => prompts.push(other.to_string()),
        }
    }
    if prompts.is_empty() {
        prompts = DEFAULT_PROMPTS.iter().map(|s| s.to_string()).collect();
    }
    Options {
        stub,
        config,
        out,
        prompts,
    }
}

struct StubProbe(ConnectivityState);

impl ConnectivityProbe for StubProbe {
    fn probe(&self) -> ConnectivityState {
        self.0
    }
}

fn boot(
    options: &Options,
) -> (Coordinator, Option<Arc<aios::surface::stub::StubServer>>) {
    if options.stub {
        let stub = aios::surface::stub::StubServer::spawn();
        let config = AiosConfig {
            model: None,
            shell: None,
            provider: vec![aios::config::ProviderConfig {
                id: "stub".into(),
                kind: "openai-compatible".into(),
                tier: "internet".into(),
                model: Some("stub-model".into()),
                endpoint: Some(format!("http://127.0.0.1:{}", stub.port)),
                api_key: None,
                api_key_env: None,
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        };
        let coordinator = Coordinator::boot_with_probe(
            config,
            Box::new(StubProbe(ConnectivityState::Internet)),
        )
        .expect("stub boot");
        (coordinator, Some(Arc::new(stub)))
    } else {
        let config = match &options.config {
            Some(path) => AiosConfig::load_from(path),
            None => AiosConfig::load(),
        }
        .unwrap_or_else(|error| {
            eprintln!("cannot load config: {error}");
            std::process::exit(2);
        });
        (Coordinator::boot_with(config).expect("boot"), None)
    }
}

fn chat_messages(prompt: &str) -> Vec<ModelMessage> {
    vec![
        ModelMessage::new(
            ModelRole::System,
            "You are Aios. Help the user understand their machine using the available tools.",
        ),
        ModelMessage::new(ModelRole::User, prompt),
    ]
}

struct Probe {
    index: usize,
    prompt: String,
    chat: Result<ChatOutcome, String>,
    compose: Result<(Surface, aios::model::RoutingDecision), SurfaceComposeError>,
    validation: Result<(), aios::surface::ValidationError>,
    diagnostics: Vec<String>,
}

fn probe_one(
    coordinator: &Coordinator,
    prompt: &str,
    index: usize,
    out: &PathBuf,
) -> Probe {
    let chat = coordinator
        .chat_with_tools_outcome(chat_messages(prompt))
        .map_err(|error| error.to_string());

    let mut compose: Result<(Surface, aios::model::RoutingDecision), SurfaceComposeError> =
        Err(SurfaceComposeError::Format("chat did not produce an answer".into()));
    if let Ok(outcome) = &chat {
        compose = coordinator.compose_surface_with_meta(
            prompt,
            &outcome.answer,
            &outcome.tool_results,
        );
    }

    let mut validation: Result<(), aios::surface::ValidationError> =
        Err(aios::surface::ValidationError {
            stage: "compose",
            message: "no surface to validate".into(),
        });
    let mut diagnostics_out = Vec::new();
    if let (Ok(outcome), Ok((surface, _))) = (&chat, &compose) {
        let evidence = EvidenceIndex::from_results(&outcome.tool_results);
        diagnostics_out = diagnostics(surface, &evidence);
        validation = validate(surface, &evidence);
    }

    if let Ok((surface, _)) = &compose {
        let text_render = render_text(surface);
        std::fs::write(out.join(format!("surface-{index}.txt")), text_render)
            .expect("write surface txt");
        std::fs::write(
            out.join(format!("surface-{index}.json")),
            serde_json::to_string_pretty(surface).expect("surface json"),
        )
        .expect("write surface json");
        std::fs::write(out.join(format!("surface-{index}.html")), render_html(surface))
            .expect("write surface html");
    }

    Probe {
        index,
        prompt: prompt.to_string(),
        chat,
        compose,
        validation,
        diagnostics: diagnostics_out,
    }
}

fn probe_to_json(probe: &Probe) -> serde_json::Value {
    let chat = match &probe.chat {
        Ok(outcome) => serde_json::json!({
            "ok": true,
            "error": null,
            "answer": outcome.answer,
            "tool_results": outcome.tool_results.iter().map(tool_result_json).collect::<Vec<_>>(),
            "evidence_keys": (0..outcome.tool_results.len()).map(|i| format!("tool-{i}")).collect::<Vec<_>>(),
        }),
        Err(error) => serde_json::json!({
            "ok": false,
            "error": error,
            "answer": null,
            "tool_results": [],
            "evidence_keys": [],
        }),
    };

    let compose = match &probe.compose {
        Ok((surface, decision)) => serde_json::json!({
            "ok": true,
            "error": null,
            "route": route_json(decision),
            "surface": serde_json::to_value(surface).expect("surface value"),
        }),
        Err(error) => serde_json::json!({
            "ok": false,
            "error": { "kind": compose_error_kind(error), "message": error.to_string() },
            "route": null,
            "surface": null,
        }),
    };

    let validation = match &probe.validation {
        Ok(()) => serde_json::json!({
            "ok": true,
            "error": null,
            "diagnostics": probe.diagnostics,
        }),
        Err(error) => serde_json::json!({
            "ok": false,
            "error": { "stage": error.stage, "message": error.message },
            "diagnostics": probe.diagnostics,
        }),
    };

    let ok = probe.chat.is_ok() && probe.compose.is_ok() && probe.validation.is_ok();

    serde_json::json!({
        "index": probe.index,
        "prompt": probe.prompt,
        "ok": ok,
        "chat": chat,
        "compose": compose,
        "validation": validation,
        "surface_version": SURFACE_VERSION,
    })
}

fn tool_result_json(result: &ToolResult) -> serde_json::Value {
    serde_json::json!({
        "tool": result.tool,
        "text": result.text,
    })
}

fn route_json(decision: &aios::model::RoutingDecision) -> serde_json::Value {
    serde_json::json!({
        "provider": decision.provider.to_string(),
        "model": decision.model.to_string(),
        "connectivity": format!("{:?}", decision.connectivity_state),
        "classification": format!("{:?}", decision.data_classification),
        "reduced_confidence": decision.reduced_confidence,
    })
}

fn compose_error_kind(error: &SurfaceComposeError) -> &'static str {
    match error {
        SurfaceComposeError::Gateway(_) => "gateway",
        SurfaceComposeError::EmptyResponse => "empty_response",
        SurfaceComposeError::Format(_) => "format",
    }
}

fn main() -> ExitCode {
    let options = parse_args();
    let (coordinator, stub) = boot(&options);
    std::fs::create_dir_all(&options.out).expect("create out dir");

    let probes: Vec<Probe> = options
        .prompts
        .iter()
        .enumerate()
        .map(|(index, prompt)| {
            let probe = probe_one(&coordinator, prompt, index, &options.out);
            let status = if probe.chat.is_ok()
                && probe.compose.is_ok()
                && probe.validation.is_ok()
            {
                "PASS"
            } else {
                "FAIL"
            };
            println!("[{status}] {prompt}");
            probe
        })
        .collect();

    let passed = probes
        .iter()
        .filter(|p| p.chat.is_ok() && p.compose.is_ok() && p.validation.is_ok())
        .count();
    let warnings: usize = probes.iter().map(|p| p.diagnostics.len()).sum();
    let failed = probes.len() - passed;

    if let Some(stub) = &stub {
        let no_tools = stub.no_tool_advertisement();
        println!(
            "composer request carried tool definitions: {}",
            if no_tools { "never" } else { "YES (bug)" }
        );
        if !no_tools {
            eprintln!("FATAL: composer request advertised tools; see surface::composer");
            return ExitCode::from(1);
        }
    }

    let report = serde_json::json!({
        "mode": if options.stub { "stub" } else { "live" },
        "summary": {
            "total": probes.len(),
            "passed": passed,
            "failed": failed,
            "warnings": warnings,
        },
        "probes": probes.iter().map(probe_to_json).collect::<Vec<_>>(),
    });
    let report_path = options.out.join("report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).expect("report json"))
        .expect("write report");

    println!(
        "summary: {} passed, {} failed, {} diagnostic warning(s); report -> {}",
        passed,
        failed,
        warnings,
        report_path.display()
    );

    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
