use super::*;

impl Coordinator {
    pub fn chat(&self, text: &str) -> Result<String, AgentError> {
        let result = self.planner.explain(text, self.local_context());
        match &result {
            Ok(_) => self.record_audit("user", "chat", text, "ok"),
            Err(e) => self.record_audit("user", "chat", text, &format!("error: {e}")),
        }
        result
    }

    pub fn chat_with_tools(&self, messages: Vec<ModelMessage>) -> Result<String, AgentError> {
        Ok(self.chat_with_tools_outcome(messages)?.answer)
    }

    pub fn chat_with_tools_outcome(
        &self,
        messages: Vec<ModelMessage>,
    ) -> Result<ChatOutcome, AgentError> {
        const MAX_TOOL_TURNS: usize = 4;
        let mut messages = messages;
        let mut tool_results = Vec::new();
        if let Some(system) = messages
            .first_mut()
            .filter(|message| message.role == ModelRole::System)
        {
            system.content.push('\n');
            system.content.push_str(model_tool_instructions());
        }
        if let Some(context) = self.local_context() {
            let system = messages
                .first_mut()
                .ok_or_else(|| AgentError::Format("tool chat requires a system message".into()))?;
            if system.role != ModelRole::System {
                return Err(AgentError::Format(
                    "tool chat requires the first message to be system role".into(),
                ));
            }
            system.content.push_str("\n\nCurrent local system state:\n");
            system.content.push_str(&context);
        }
        let required_calls = required_specialist_calls(&messages);
        for call in &required_calls {
            let result = self
                .run_tool_as("planner", call)
                .map_err(|error| AgentError::Format(format!("required specialist failed: {error}")))?;
            let content = format!("tool {} result:\n{}", result.tool, result.text);
            tool_results.push(result);
            messages.push(ModelMessage::new(ModelRole::User, content));
        }
        if !required_calls.is_empty() {
            messages.push(ModelMessage::new(
                ModelRole::User,
                "The required specialist evidence has been gathered for this request. Do not ask for clarification about those domains. Answer using the returned evidence only.",
            ));
        }
        self.report(
            GraphPhase::Planning,
            &["facade", "coordinator", "planner", "gateway"],
        );
        let mut answer = self.planner.chat_with(messages.clone(), None)?;

        for turn in 0..MAX_TOOL_TURNS {
            let calls = parse_tool_calls(&answer);
            if calls.is_empty() {
                // Architecture §4: conversational answers are valid without a
                // tool call. Simple queries and greetings do not need to invoke
                // a model tool, and we do not force a tool for every trivial
                // read. If the Planner chose not to call a tool, its plain
                // answer is returned.
                return Ok(ChatOutcome {
                    answer: strip_tool_calls_json(&answer).trim().to_string(),
                    tool_results,
                });
            }

            messages.push(ModelMessage::new(ModelRole::Assistant, &answer));
            for call in calls {
                let result = self.run_tool_as("planner", &call);
                let content = match result {
                    Ok(result) => {
                        let content = format!("tool {} result:\n{}", result.tool, result.text);
                        tool_results.push(result);
                        content
                    }
                    Err(error) => format!("tool {} error: {}", call.name, error),
                };
                messages.push(ModelMessage::new(ModelRole::User, content));
                messages.push(ModelMessage::new(
                    ModelRole::User,
                    "The tool result above is complete and authoritative. Do not call another tool. Answer the original user question now using only the returned evidence. If the evidence does not contain the requested metric, say that it is unavailable.",
                ));
            }
            if turn + 1 == MAX_TOOL_TURNS {
                self.record_audit("planner", "tool_loop", "chat", "turn cap reached");
                return Err(AgentError::Format(
                    "tool-call turn cap reached before a grounded answer".into(),
                ));
            }
            self.report(
                GraphPhase::Planning,
                &["facade", "coordinator", "planner", "gateway"],
            );
            answer = self.planner.chat_with(messages.clone(), None)?;
        }

        Ok(ChatOutcome {
            answer: strip_tool_calls_json(&answer).trim().to_string(),
            tool_results,
        })
    }

    pub fn local_context(&self) -> Option<String> {
        let summary = self.last_scan_summary.read().expect("scan lock").clone();
        let summary = summary?;
        // Machine state is only attached when the chat role has a model and
        // that provider may see system-config data. Revoking consent for the
        // assigned provider stops machine state from leaving the machine.
        let (provider, _) = self.gateway.router().assignment("chat")?;
        let consent_ok = self
            .gateway
            .router()
            .consent_for(&provider)
            .map(|c| c.is_active_for(DataClassification::SystemConfig))
            .unwrap_or(false);
        if !consent_ok {
            return None;
        }
        let mut context = summary;
        let graph = self.graph.read().expect("graph lock");
        let index = resource_index(&graph);
        if !index.is_empty() {
            context.push('\n');
            context.push_str(&index);
        }
        Some(context)
    }

    pub fn run_tool(&self, name: &str, args: &str) -> Result<crate::tools::ToolResult, ToolError> {
        let call = ToolCallRequest {
            name: name.to_string(),
            arguments: args.to_string(),
        };
        self.run_tool_as("user", &call)
    }

    pub(crate) fn run_tool_as(
        &self,
        actor: &str,
        call: &ToolCallRequest,
    ) -> Result<crate::tools::ToolResult, ToolError> {
        let operation =
            operation_for_tool(&call.name).ok_or_else(|| ToolError::Unknown(call.name.clone()))?;
        // Specialist tools route to the owning specialist's resource
        // (message-protocol §8.1); generic read-only tools route to the graph.
        let is_wifi_tool = matches!(
            call.name.as_str(),
            "wifi.observe_device"
                | "wifi.diagnose_fault"
                | "wifi.stage_driver"
                | "wifi.request_reset"
        );
        let is_storage_tool = matches!(
            call.name.as_str(),
            "storage.observe_storage" | "storage.diagnose_fault"
        );
        let is_network_tool = matches!(
            call.name.as_str(),
            "network.observe_network" | "network.diagnose_fault"
        );
        let is_drivers_tool = matches!(
            call.name.as_str(),
            "drivers.observe_device" | "drivers.diagnose_fault"
        );
        let is_graphics_tool = matches!(
            call.name.as_str(),
            "graphics.observe_graphics" | "graphics.diagnose_fault"
        );
        let is_memory_tool = matches!(
            call.name.as_str(),
            "memory.observe_memory" | "memory.diagnose_fault"
        );
        let is_processes_tool = matches!(
            call.name.as_str(),
            "processes.observe_process" | "processes.diagnose_fault"
        );
        let is_power_tool = matches!(
            call.name.as_str(),
            "power.observe_thermal" | "power.diagnose_fault"
        );
        let is_security_tool = matches!(
            call.name.as_str(),
            "security.observe_security" | "security.diagnose_fault"
        );
        let is_boot_tool = matches!(
            call.name.as_str(),
            "boot.observe_boot" | "boot.diagnose_fault"
        );
        let is_packages_tool = matches!(
            call.name.as_str(),
            "packages.observe_package" | "packages.diagnose_fault"
        );
        let resource = if is_wifi_tool {
            let device = self
                .wifi_specialist
                .as_ref()
                .ok_or_else(|| ToolError::Permission("no wi-fi specialist instantiated".into()))?
                .device
                .clone();
            ResourceId(device.0)
        } else if is_storage_tool {
            ResourceId("storage:domain".into())
        } else if is_network_tool {
            ResourceId("network:domain".into())
        } else if is_drivers_tool {
            ResourceId("drivers:domain".into())
        } else if is_power_tool {
            ResourceId("power:domain".into())
        } else if is_graphics_tool {
            ResourceId("graphics:domain".into())
        } else if is_memory_tool {
            ResourceId("memory:domain".into())
        } else if is_processes_tool {
            ResourceId("processes:domain".into())
        } else if is_security_tool {
            ResourceId("security:domain".into())
        } else if is_boot_tool {
            ResourceId("boot:domain".into())
        } else if is_packages_tool {
            ResourceId("packages:domain".into())
        } else {
            ResourceId("system:graph".into())
        };
        let principal = self.session_principal.clone();
        let client = self.broker.client(principal.clone());
        // Static session tokens issued at session start (capability-model §6.3).
        let token = self
            .session_tokens
            .iter()
            .find(|token| {
                token.capability.operation == operation && token.capability.resource == resource
            })
            .cloned()
            .ok_or_else(|| {
                ToolError::Permission(format!("no session token for {operation:?} on {resource}"))
            })?;
        let active_id = match resource.0.as_str() {
            // Graph queries are served through the tool registry.
            "system:graph" => "tools".to_string(),
            other => other.to_string(),
        };
        let owner_id = {
            let graph = self.graph.read().expect("graph lock");
            graph
                .get_owner(&NodeId(resource.0.clone()))
                .map(|owner| owner.node_id.0)
        };
        let mut active_ids = vec![
            "coordinator".to_string(),
            "broker".to_string(),
            "tools".to_string(),
            active_id,
        ];
        if let Some(owner) = owner_id {
            if !active_ids.contains(&owner) {
                active_ids.push(owner);
            }
        }
        let active_refs: Vec<&str> = active_ids.iter().map(String::as_str).collect();
        self.report(GraphPhase::Gathering, &active_refs);
        let parameters = tool_parameters(operation, &call.arguments);
        let mut request = crate::protocol::ToolRequest::new(
            principal,
            resource,
            operation,
            call.name.clone(),
            token,
            parameters,
            uuid::Uuid::new_v4(),
            DataClassification::SystemConfig,
            30,
        );
        request.nonce = NEXT_TOOL_NONCE.fetch_add(1, Ordering::Relaxed);
        let protocol_result = client
            .request_tool(request)
            .map_err(|e| ToolError::Permission(e.to_string()))?;
        let result = protocol_tool_result(&call.name, protocol_result);
        match &result {
            Ok(tool_result) => self.record_audit(
                actor,
                "tool",
                &format!("{} {}", call.name, call.arguments),
                &format!("ok ({} chars)", tool_result.text.len()),
            ),
            Err(e) => self.record_audit(
                actor,
                "tool",
                &format!("{} {}", call.name, call.arguments),
                &format!("error: {e}"),
            ),
        }
        result
    }
}

pub(crate) fn operation_for_tool(name: &str) -> Option<Operation> {
    match name {
        "observe" => Some(Operation::Observe),
        "diagnose" => Some(Operation::Diagnose),
        "query" | "deps" | "impact" | "health" => Some(Operation::Query),
        "wifi.observe_device" => Some(Operation::Observe),
        "wifi.diagnose_fault" => Some(Operation::Diagnose),
        "wifi.stage_driver" => Some(Operation::Stage),
        "wifi.request_reset" => Some(Operation::Reset),
        "storage.observe_storage" => Some(Operation::Observe),
        "storage.diagnose_fault" => Some(Operation::Diagnose),
        "network.observe_network" => Some(Operation::Observe),
        "network.diagnose_fault" => Some(Operation::Diagnose),
        "drivers.observe_device" => Some(Operation::Observe),
        "drivers.diagnose_fault" => Some(Operation::Diagnose),
        "graphics.observe_graphics" => Some(Operation::Observe),
        "graphics.diagnose_fault" => Some(Operation::Diagnose),
        "memory.observe_memory" => Some(Operation::Observe),
        "memory.diagnose_fault" => Some(Operation::Diagnose),
        "processes.observe_process" => Some(Operation::Observe),
        "processes.diagnose_fault" => Some(Operation::Diagnose),
        "power.observe_thermal" => Some(Operation::Observe),
        "power.diagnose_fault" => Some(Operation::Diagnose),
        "security.observe_security" => Some(Operation::Observe),
        "security.diagnose_fault" => Some(Operation::Diagnose),
        "boot.observe_boot" => Some(Operation::Observe),
        "boot.diagnose_fault" => Some(Operation::Diagnose),
        "packages.observe_package" => Some(Operation::Observe),
        "packages.diagnose_fault" => Some(Operation::Diagnose),
        _ => None,
    }
}

pub(crate) fn tool_parameters(operation: Operation, args: &str) -> crate::protocol::ToolParameters {
    match operation {
        Operation::Observe => crate::protocol::ToolParameters::Observe {
            fields: vec![args.into()],
        },
        Operation::Diagnose => crate::protocol::ToolParameters::Diagnose {
            symptom: args.into(),
        },
        Operation::Query => crate::protocol::ToolParameters::Query { query: args.into() },
        Operation::Stage => crate::protocol::ToolParameters::Stage {
            change: serde_json::json!({ "module": args.trim() }),
        },
        Operation::Reset => crate::protocol::ToolParameters::Reset {
            to_known_good: true,
        },
        _ => unreachable!("operation mapping is exhaustive"),
    }
}

pub(crate) fn tool_arguments(parameters: &crate::protocol::ToolParameters) -> String {
    match parameters {
        crate::protocol::ToolParameters::Observe { fields } => fields.join(" "),
        crate::protocol::ToolParameters::Diagnose { symptom } => symptom.clone(),
        crate::protocol::ToolParameters::Query { query } => query.clone(),
        _ => panic!("read-only specialist received a mutating parameter"),
    }
}
pub(crate) fn protocol_tool_result(
    name: &str,
    result: crate::protocol::ToolResult,
) -> Result<crate::tools::ToolResult, ToolError> {
    if result.status != crate::protocol::ToolStatus::Success {
        let message = result
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "broker denied tool".into());
        let message = message.strip_prefix("denied: ").unwrap_or(&message);
        if let Some(target) = message.strip_prefix("nothing matches: ") {
            return Err(ToolError::NotFound(target.into()));
        }
        return Err(ToolError::Permission(message.into()));
    }
    let text = match result.data {
        Some(crate::protocol::ToolData::QueryResult { data }) => data
            .get("text")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::Usage(format!("tool {name} returned malformed data")))?
            .to_string(),
        Some(crate::protocol::ToolData::DeviceState { state, metrics }) => {
            let mut parts = vec![format!("state={state:?}")];
            let mut metrics: Vec<(String, String)> = metrics.into_iter().collect();
            metrics.sort();
            for (k, v) in metrics {
                parts.push(format!("{k}={}", quote_value(&v)));
            }
            parts.join(" ")
        }
        Some(crate::protocol::ToolData::Diagnosis {
            findings,
            confidence,
        }) => format!(
            "confidence={confidence} findings=[{}]",
            findings.join(" | ")
        ),
        _ => {
            return Err(ToolError::Usage(format!(
                "tool {name} returned no result data"
            )));
        }
    };
    Ok(crate::tools::ToolResult {
        tool: Box::leak(name.to_string().into_boxed_str()),
        text,
    })
}

/// Quote a metric value for the flattened `k=v k=v` protocol text so values
/// with spaces (command lines, mountpoints, labels) survive as one field.
pub(crate) fn quote_value(value: &str) -> String {
    if value.is_empty() || value.contains(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}
pub(crate) fn required_specialist_calls(messages: &[ModelMessage]) -> Vec<ToolCallRequest> {
    let Some(prompt) = messages
        .iter()
        .rev()
        .find(|message| message.role == ModelRole::User)
        .map(|message| message.content.to_ascii_lowercase())
    else {
        return Vec::new();
    };
    let mut calls = Vec::new();
    let add = |calls: &mut Vec<ToolCallRequest>, name: &str, arguments: &str| {
        calls.push(ToolCallRequest {
            name: name.to_string(),
            arguments: arguments.to_string(),
        });
    };
    if prompt.contains("cpu") || prompt.contains("process") || prompt.contains("service") {
        add(&mut calls, "processes.observe_process", "all");
    }
    if prompt.contains("service") {
        add(&mut calls, "query", "service");
    }
    if prompt.contains("ram") || prompt.contains("memory") || prompt.contains("swap") {
        add(&mut calls, "memory.observe_memory", "all");
    }
    if prompt.contains("disk") || prompt.contains("storage") {
        add(&mut calls, "storage.observe_storage", "all");
    }
    if prompt.contains("network") || prompt.contains("wifi") || prompt.contains("internet") {
        add(&mut calls, "network.observe_network", "all");
    }
    calls
}
