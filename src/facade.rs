use crate::coordinator::{
    Coordinator, BootError, classification_help, providers_text, send_direct, status_text,
};
use crate::planner::{AgentError, format_plan};
use crate::protocol::DataClassification;
use crate::verifier::format_review;
use std::collections::VecDeque;
use std::io::{self, Write};

pub struct Facade {
    pub coordinator: Coordinator,
    history: VecDeque<String>,
    max_history: usize,
}

impl Facade {
    pub fn boot() -> Result<Self, BootError> {
        let coordinator = Coordinator::boot()?;
        Ok(Self::new(coordinator))
    }

    pub fn new(coordinator: Coordinator) -> Self {
        let max_history = coordinator
            .config
            .shell
            .as_ref()
            .map(|s| s.history_len)
            .unwrap_or(20);
        Self {
            coordinator,
            history: VecDeque::new(),
            max_history,
        }
    }

    pub fn banner(&self) -> String {
        format!(
            "aios shell\n{}\ntype 'help' for commands",
            status_text(&self.coordinator)
        )
    }

    pub fn run_line(&mut self, input: &str) -> String {
        let line = input.trim();
        if line.is_empty() {
            return String::new();
        }
        let (command, rest) = match line.find(char::is_whitespace) {
            Some(index) => (&line[..index], line[index..].trim()),
            None => (line, ""),
        };
        match command {
            "help" => help_text().to_string(),
            "status" => status_text(&self.coordinator),
            "providers" => providers_text(&self.coordinator),
            "scan" => self.coordinator.scan(),
            "graph" => self.coordinator.graph_summary(),
            "consent" => self.consent(rest),
            "plan" => {
                if rest.is_empty() {
                    "usage: plan <intent>".to_string()
                } else {
                    self.plan(rest)
                }
            }
            "model" => {
                if rest.is_empty() {
                    "usage: model <text>".to_string()
                } else {
                    self.direct(rest)
                }
            }
            "route" => match self.coordinator.current_route() {
                Ok(route) => format!(
                    "{} ({:?}) reduced-confidence={}",
                    route.provider, route.model, route.reduced_confidence
                ),
                Err(e) => format!("no route: {e}"),
            },
            "tools" => self.coordinator.tools_help(),
            "audit" => {
                if rest.is_empty() {
                    self.audit()
                } else {
                    self.audit_filtered(rest)
                }
            }
            "observe" | "diagnose" | "query" | "deps" | "impact" | "health" => {
                if command == "health" && rest.is_empty() {
                    self.tool(command, "")
                } else if rest.is_empty() {
                    format!("usage: {command} <target>")
                } else {
                    self.tool(command, rest)
                }
            }
            "exit" | "quit" => String::new(),
            _ => self.chat(line),
        }
    }

    fn audit(&self) -> String {
        let entries = self.coordinator.audit.entries();
        if entries.is_empty() {
            return "audit log: empty".into();
        }
        let mut lines = Vec::new();
        for entry in entries.iter().rev().take(20) {
            lines.push(format!(
                "{} {} {} {} {}",
                entry.timestamp, entry.actor, entry.action, entry.target, entry.outcome
            ));
        }
        lines.join("\n")
    }

    fn audit_filtered(&self, filter: &str) -> String {
        let entries = self
            .coordinator
            .audit
            .entries()
            .into_iter()
            .rev()
            .filter(|e| {
                e.actor.contains(filter)
                    || e.action.contains(filter)
                    || e.target.contains(filter)
            })
            .take(20)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return format!("audit log: no entries matching '{filter}'");
        }
        entries
            .iter()
            .map(|e| format!("{} {} {} {} {}", e.timestamp, e.actor, e.action, e.target, e.outcome))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn tool(&self, name: &str, args: &str) -> String {
        match self.coordinator.run_tool(name, args) {
            Ok(result) => result.text,
            Err(e) => format!("tool failed: {e}"),
        }
    }

    fn consent(&self, rest: &str) -> String {
        let mut parts = rest.split_whitespace();
        match (parts.next(), parts.next(), parts.next()) {
            (None, _, _) | (Some("list"), _, _) => {
                let mut lines = Vec::new();
                for entry in self.coordinator.provider_entries() {
                    let provider = entry.provider.to_string();
                    match self.coordinator.consent_for(&provider) {
                        Some(record) => {
                            let scope: Vec<String> = record
                                .data_scope
                                .iter()
                                .map(|c| format!("{c:?}"))
                                .collect();
                            let state = if record.revoked_at.is_some() {
                                "revoked"
                            } else {
                                "active"
                            };
                            lines.push(format!("{provider}: {} ({state})", scope.join(", ")));
                        }
                        None => lines.push(format!("{provider}: no consent granted")),
                    }
                }
                if lines.is_empty() {
                    classification_help()
                } else {
                    lines.join("\n")
                }
            }
            (Some(provider), Some(class), Some("on")) => {
                match parse_class(class) {
                    Some(class) => match self.coordinator.grant_consent(provider, class) {
                        Ok(()) => format!("consent granted: {provider} ({class:?})"),
                        Err(e) => format!("grant failed: {e}"),
                    },
                    None => format!("unknown class: {class}"),
                }
            }
            (Some(provider), Some(_class), Some("off")) => {
                self.coordinator.revoke_consent(provider);
                format!("consent revoked: {provider}")
            }
            _ => classification_help(),
        }
    }

    fn plan(&self, intent: &str) -> String {
        match self.coordinator.plan_and_review(intent) {
            Ok((plan, review)) => format!("{}\n\n{}", format_plan(&plan), format_review(&review)),
            Err(AgentError::Gateway(e)) => {
                format!("planning failed: {e}\nhint: ensure a provider is healthy (see 'status')")
            }
            Err(e) => format!("planning failed: {e}"),
        }
    }

    fn direct(&self, text: &str) -> String {
        match send_direct(&self.coordinator, text) {
            Ok(answer) => {
                self.coordinator
                    .audit
                    .record("user", "model", text, "ok");
                answer
            }
            Err(e) => {
                self.coordinator
                    .audit
                    .record("user", "model", text, &format!("error: {e}"));
                format!("model call failed: {e}")
            }
        }
    }

    fn chat(&mut self, text: &str) -> String {
        self.history.push_back(format!("user: {text}"));
        while self.history.len() > self.max_history {
            self.history.pop_front();
        }

        let mut messages = vec![crate::model::ModelMessage::new(
            crate::model::ModelRole::System,
            "You are Aios, the assistant for a Linux system. Answer concisely.",
        )];
        for turn in &self.history {
            let (role, content) = turn.split_once(':').unwrap_or(("user", turn));
            let role = match role {
                "user" => crate::model::ModelRole::User,
                _ => crate::model::ModelRole::Assistant,
            };
            messages.push(crate::model::ModelMessage::new(role, content));
        }

        let result = self
            .coordinator
            .planner
            .chat_with(messages, self.coordinator.local_context());
        match result {
            Ok(answer) => {
                self.coordinator
                    .audit
                    .record("user", "chat", text, "ok");
                self.history.push_back(format!("assistant: {answer}"));
                while self.history.len() > self.max_history {
                    self.history.pop_front();
                }
                answer
            }
            Err(e) => {
                self.coordinator
                    .audit
                    .record("user", "chat", text, &format!("error: {e}"));
                format!("chat failed: {e}\nhint: check 'status' for provider health")
            }
        }
    }
}

fn parse_class(class: &str) -> Option<DataClassification> {
    match class {
        "public" => Some(DataClassification::Public),
        "personal-memory" => Some(DataClassification::PersonalMemory),
        "system-config" => Some(DataClassification::SystemConfig),
        "protected" => Some(DataClassification::Protected),
        _ => None,
    }
}

pub fn help_text() -> &'static str {
    "commands:\n\
     \x20 status           connectivity, providers, route, local model\n\
     \x20 providers        provider details and consent\n\
     \x20 scan             run discovery and refresh the system graph\n\
     \x20 graph            show the current graph summary\n\
     \x20 consent          list consent\n\
     \x20 consent <p> <class> on|off   grant or revoke consent for a provider\n\
     \x20 plan <intent>    plan steps then verify them\n\
     \x20 model <text>     ask the model directly, no agent framing\n\
     \x20 route            show the current model route\n\
     \x20 tools            list read-only specialist tools\n\
     \x20 observe <t>      node details (id, label, or attribute value)\n\
     \x20 diagnose <t>     health and dependency summary for a node\n\
     \x20 query <type>     list nodes of a type (device, service, driver, ...)\n\
     \x20 deps <t>         full dependency chain of a node\n\
     \x20 impact <t>       what a node depends on and what depends on it\n\
     \x20 health           roll up node health across the graph\n\
     \x20 audit            recent audit log entries (or audit <filter>)\n\
     \x20 exit, quit       leave the shell\n\
     anything else is sent to the model as a chat message"
}

pub fn run_interactive() {
    let mut facade = match Facade::boot() {
        Ok(facade) => facade,
        Err(e) => {
            eprintln!("aios: {e}");
            std::process::exit(1);
        }
    };
    println!("{}", facade.banner());
    let stdin = io::stdin();
    let mut input = String::new();
    loop {
        print!("aios> ");
        io::stdout().flush().ok();
        input.clear();
        match stdin.read_line(&mut input) {
            Ok(0) => break,
            Err(_) => break,
            _ => {}
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }
        println!("{}", facade.run_line(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiosConfig, ProviderConfig};
    use crate::coordinator::Coordinator;
    use crate::model::{ConnectivityState, ConnectivityProbe};
    use crate::testutil;

    struct FakeProbe(ConnectivityState);

    impl ConnectivityProbe for FakeProbe {
        fn probe(&self) -> ConnectivityState {
            self.0
        }
    }

    fn handler(body: &str) -> String {
        if body.contains("steps: ") {
            testutil::openai_response(
                r#"{"verdict":"approve","concerns":[],"tests":["ping"]}"#,
            )
        } else if body.contains("fix my wifi") {
            testutil::openai_response(
                r#"{"intent":"fix my wifi","steps":[{"description":"check link","tool":"iw dev","resource":"wifi0","risk":"read-only"}]}"#,
            )
        } else {
            testutil::openai_response("hello from stub")
        }
    }

    fn facade(port: u16) -> Facade {
        let config = AiosConfig {
            model: None,
            shell: None,
            provider: vec![ProviderConfig {
                id: "stub".into(),
                kind: "openai-compatible".into(),
                tier: "internet".into(),
                model: Some("stub-model".into()),
                endpoint: Some(format!("http://127.0.0.1:{port}")),
                api_key: None,
                api_key_env: None,
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        };
        let coordinator = Coordinator::boot_with_probe(
            config,
            Box::new(FakeProbe(ConnectivityState::Internet)),
        )
        .expect("boot");
        Facade::new(coordinator)
    }

    #[test]
    fn empty_line_returns_nothing() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        assert_eq!(f.run_line(""), "");
        assert_eq!(f.run_line("   "), "");
    }

    #[test]
    fn help_lists_commands() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("help");
        assert!(out.contains("status"));
        assert!(out.contains("scan"));
        assert!(out.contains("plan"));
    }

    #[test]
    fn status_shows_connectivity_and_provider() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("status");
        assert!(out.contains("connectivity: Internet"), "{out}");
        assert!(out.contains("stub"), "{out}");
    }

    #[test]
    fn providers_lists_entries() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("providers");
        assert!(out.contains("stub"), "{out}");
        assert!(out.contains("stub-model"), "{out}");
    }

    #[test]
    fn direct_model_query() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("model say hi");
        assert_eq!(out, "hello from stub");
    }

    #[test]
    fn plan_command_runs_planner_and_verifier() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("plan fix my wifi");
        assert!(out.contains("intent: fix my wifi"), "{out}");
        assert!(out.contains("read-only"), "{out}");
        assert!(out.contains("verdict: approve"), "{out}");
    }

    #[test]
    fn bare_chat_goes_to_model() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("hello there");
        assert_eq!(out, "hello from stub");
    }

    #[test]
    fn consent_commands_roundtrip() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("consent stub system-config on");
        assert!(out.contains("consent granted"), "{out}");
        let listed = f.run_line("consent");
        assert!(listed.contains("SystemConfig"), "{listed}");
        let off = f.run_line("consent stub system-config off");
        assert!(off.contains("revoked"), "{off}");
        let bad_class = f.run_line("consent stub nonsense on");
        assert!(bad_class.contains("unknown class"), "{bad_class}");
    }

    #[test]
    fn tools_lists_specialist_tools() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("tools");
        assert!(out.contains("observe"), "{out}");
        assert!(out.contains("diagnose"), "{out}");
        assert!(out.contains("query"), "{out}");
    }

    #[test]
    fn tool_commands_against_empty_graph() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("observe wifi0");
        assert!(out.contains("tool failed: nothing matches"), "{out}");
        let out = f.run_line("query device");
        assert!(out.contains("no device nodes found"), "{out}");
        let out = f.run_line("diagnose");
        assert!(out.contains("usage"), "{out}");
    }

    #[test]
    fn health_command_without_scan() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("health");
        assert!(out.contains("0 nodes total"), "{out}");
    }

    #[test]
    fn audit_records_boot_and_commands() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        let out = f.run_line("audit");
        assert!(out.contains("boot"), "{out}");
        let filtered = f.run_line("audit boot");
        assert!(filtered.contains("boot"), "{filtered}");
        let empty = f.run_line("audit zzz-nothing");
        assert!(empty.contains("no entries"), "{empty}");
    }

    #[test]
    fn audit_logs_chat_attempt() {
        let port = testutil::spawn_json_server(handler);
        let mut f = facade(port);
        f.run_line("hello there");
        let out = f.run_line("audit chat");
        assert!(out.contains("chat"), "{out}");
        assert!(out.contains("hello there"), "{out}");
    }
}
