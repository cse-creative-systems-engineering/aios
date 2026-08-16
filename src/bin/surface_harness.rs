//! Headless generative-surface test harness.
//!
//! Drives the real pipeline without a display, as a natural conversation:
//! the user and Aios exchange turns (each turn runs the planner tool loop so
//! specialists read real system data), and at the end of the conversation the
//! composer call turns the exchange into a `Surface` which is validated and
//! rendered to text/HTML. A monitoring report is written per conversation.
//!
//! Usage:
//!   surface-harness [--stub] [--config PATH] [--out DIR]
//!                   [--conv NAME] [prompts...]
//!
//! - `--stub` runs against a deterministic stub model server (no network).
//! - `--conv NAME` runs one canned conversation (`overview`, `storage`,
//!   `memory`); by default the whole canned suite runs.
//! - `--out DIR` writes `surface-N.json`, `surface-N.html`, and `report.json`.
//! - Positional prompts run as single-turn conversations (back-compat).
//! Exit code is non-zero when any conversation failed.

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

/// Canned multi-turn conversations. Each ends on the question the panel
/// should answer; earlier turns are natural lead-ins that may also gather
/// evidence (all tool results across the conversation are available to the
/// composer).
const DEFAULT_CONVERSATIONS: &[(&str, &[&str])] = &[
    (
        "overview",
        &[
            "Hi, can you check on my system?",
            "Show me a panel with the overall health and the biggest problems.",
        ],
    ),
    (
        "storage",
        &[
            "How much space is left on my disk?",
            "And is the drive itself healthy?",
        ],
    ),
    (
        "memory",
        &[
            "How much memory is in use?",
            "Is there any memory pressure?",
        ],
    ),
];

const SYSTEM_PROMPT: &str =
    "You are Aios. Help the user understand their machine using the available tools.";

struct Options {
    stub: bool,
    config: Option<PathBuf>,
    out: PathBuf,
    conversations: Vec<Conversation>,
}

struct Conversation {
    name: String,
    turns: Vec<String>,
}

fn parse_args() -> Options {
    let mut stub = false;
    let mut config = None;
    let mut out = PathBuf::from("harness-out");
    let mut only: Option<String> = None;
    let mut prompts = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stub" => stub = true,
            "--config" => config = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from).unwrap_or(out),
            "--conv" => only = args.next(),
            other => prompts.push(other.to_string()),
        }
    }

    let mut conversations = Vec::new();
    for (name, turns) in DEFAULT_CONVERSATIONS {
        if let Some(only) = &only {
            if name != only {
                continue;
            }
        }
        conversations.push(Conversation {
            name: name.to_string(),
            turns: turns.iter().map(|s| s.to_string()).collect(),
        });
    }
    for prompt in prompts {
        conversations.push(Conversation {
            name: format!("prompt:{prompt}"),
            turns: vec![prompt],
        });
    }
    if conversations.is_empty() {
        conversations = DEFAULT_CONVERSATIONS
            .iter()
            .map(|(name, turns)| Conversation {
                name: name.to_string(),
                turns: turns.iter().map(|s| s.to_string()).collect(),
            })
            .collect();
    }

    Options {
        stub,
        config,
        out,
        conversations,
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

struct Turn {
    user: String,
    ok: bool,
    answer: String,
    tool_results: Vec<ToolResult>,
}

struct Probe {
    index: usize,
    conversation: String,
    turns: Vec<Turn>,
    used_tools: bool,
    compose: Result<(Surface, aios::model::RoutingDecision), SurfaceComposeError>,
    validation: Result<(), aios::surface::ValidationError>,
    diagnostics: Vec<String>,
}

/// Run one conversation: each user turn goes through the real planner tool
/// loop with the growing history, then a surface is composed from the last
/// question, the final answer, and every tool result gathered across turns.
fn probe_conversation(
    coordinator: &Coordinator,
    conversation: &Conversation,
    index: usize,
    out: &PathBuf,
) -> Probe {
    let mut history = vec![ModelMessage::new(ModelRole::System, SYSTEM_PROMPT)];
    let mut turns = Vec::new();
    let mut all_evidence: Vec<ToolResult> = Vec::new();

    for user_text in &conversation.turns {
        history.push(ModelMessage::new(ModelRole::User, user_text.clone()));
        let (ok, answer, tool_results) = match coordinator.chat_with_tools_outcome(history.clone()) {
            Ok(ChatOutcome {
                answer,
                tool_results,
            }) => (true, answer, tool_results),
            Err(error) => {
                let answer = format!("<error: {error}>");
                (false, answer, Vec::new())
            }
        };
        all_evidence.extend(tool_results.iter().cloned());
        turns.push(Turn {
            user: user_text.clone(),
            ok,
            answer: answer.clone(),
            tool_results,
        });
        history.push(ModelMessage::new(ModelRole::Assistant, answer));
    }

    let intent = conversation
        .turns
        .last()
        .cloned()
        .unwrap_or_default();
    let answer = turns
        .last()
        .map(|turn| turn.answer.clone())
        .unwrap_or_default();
    let compose = coordinator.compose_surface_with_meta(&intent, &answer, &all_evidence);

    let mut validation: Result<(), aios::surface::ValidationError> =
        Err(aios::surface::ValidationError {
            stage: "compose",
            message: "no surface to validate".into(),
        });
    let mut diagnostics_out = Vec::new();
    if let Ok((surface, _)) = &compose {
        let evidence = EvidenceIndex::from_results(&all_evidence);
        diagnostics_out = diagnostics(surface, &evidence);
        validation = validate(surface, &evidence);

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
        conversation: conversation.name.clone(),
        used_tools: turns
            .iter()
            .any(|turn| !turn.tool_results.is_empty()),
        turns,
        compose,
        validation,
        diagnostics: diagnostics_out,
    }
}

fn probe_to_json(probe: &Probe) -> serde_json::Value {
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

    let ok = probe
        .turns
        .iter()
        .all(|turn| turn.ok)
        && probe.compose.is_ok()
        && probe.validation.is_ok();

    serde_json::json!({
        "index": probe.index,
        "conversation": probe.conversation,
        "ok": ok,
        "turns": probe.turns.iter().map(turn_to_json).collect::<Vec<_>>(),
        "used_tools": probe.used_tools,
        "compose": compose,
        "validation": validation,
        "surface_version": SURFACE_VERSION,
    })
}

fn turn_to_json(turn: &Turn) -> serde_json::Value {
    serde_json::json!({
        "user": turn.user,
        "ok": turn.ok,
        "answer": turn.answer,
        "tool_results": turn.tool_results.iter().map(tool_result_json).collect::<Vec<_>>(),
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
        .conversations
        .iter()
        .enumerate()
        .map(|(index, conversation)| {
            let probe = probe_conversation(&coordinator, conversation, index, &options.out);
            let turns_ok = probe.turns.iter().all(|turn| turn.ok);
            let status = if turns_ok && probe.compose.is_ok() && probe.validation.is_ok() {
                "PASS"
            } else {
                "FAIL"
            };
            let tool_marker = if probe.used_tools {
                "tools used"
            } else {
                "no tool calls"
            };
            println!(
                "[{status}] {} ({} turns, {tool_marker})",
                probe.conversation,
                probe.turns.len()
            );
            probe
        })
        .collect();

    let passed = probes
        .iter()
        .filter(|probe| {
            probe.turns.iter().all(|turn| turn.ok)
                && probe.compose.is_ok()
                && probe.validation.is_ok()
        })
        .count();
    let warnings: usize = probes.iter().map(|probe| probe.diagnostics.len()).sum();
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
