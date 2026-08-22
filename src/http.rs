use crate::model::{
    FinishReason, GenerationError, GenerationRequest, GenerationResponse, ModelBackend, ModelId,
    ModelRole, ProviderId, ProviderTier,
};
use serde_json::{Value, json};

pub struct HttpBackend {
    provider: ProviderId,
    model: String,
    endpoint: String,
    api_key: Option<String>,
    tier: ProviderTier,
    http_timeout_ms: u64,
}

impl HttpBackend {
    pub fn new(
        provider: ProviderId,
        model: String,
        endpoint: String,
        api_key: Option<String>,
        tier: ProviderTier,
        http_timeout_ms: u64,
    ) -> Self {
        Self {
            provider,
            model,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key,
            tier,
            http_timeout_ms,
        }
    }

    pub fn model_id(&self) -> ModelId {
        ModelId::new(self.model.clone())
    }

    pub fn tier(&self) -> ProviderTier {
        self.tier
    }

    fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.endpoint)
    }

    fn request_body(&self, request: &GenerationRequest) -> Value {
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    ModelRole::System => "system",
                    ModelRole::User => "user",
                    ModelRole::Assistant => "assistant",
                };
                json!({ "role": role, "content": message.content })
            })
            .collect();
        // serde_json widens floats to f64, which turns 0.3f32 into
        // 0.30000001192092896; some providers reject that as invalid. Round
        // in f64 space so the wire value matches what was configured.
        let temperature = (f64::from(request.temperature) * 1000.0).round() / 1000.0;
        let mut body = json!({
            "model": request.model.clone().unwrap_or_else(|| self.model.clone()),
            "messages": messages,
            "max_tokens": request.max_tokens,
            "temperature": temperature,
        });
        if request.messages.iter().any(|message| {
            message.role == ModelRole::System
                && message
                    .content
                    .contains("Read-only machine tools are available")
        }) {
            body["tools"] = json!([
                function_tool("observe", "Observe one discovered node", "target"),
                function_tool("diagnose", "Diagnose one discovered node", "target"),
                function_tool("query", "Query discovered nodes", "query"),
                function_tool("deps", "Query node dependencies", "target"),
                function_tool("impact", "Query node impact relationships", "target"),
                function_tool_no_args("health", "Summarize graph health"),
                function_tool(
                    "wifi_observe_device",
                    "Observe the Wi-Fi specialist device",
                    "target"
                ),
                function_tool("wifi_diagnose_fault", "Diagnose a Wi-Fi fault", "target"),
                function_tool("storage_observe_storage", "Observe storage and filesystem state", "target"),
                function_tool("storage_diagnose_fault", "Diagnose storage faults", "target"),
                function_tool("network_observe_network", "Observe network state", "target"),
                function_tool("network_diagnose_fault", "Diagnose network faults", "target"),
                function_tool("drivers_observe_device", "Observe device and driver state", "target"),
                function_tool("drivers_diagnose_fault", "Diagnose device and driver faults", "target"),
                function_tool("graphics_observe_graphics", "Observe graphics state", "target"),
                function_tool("graphics_diagnose_fault", "Diagnose graphics faults", "target"),
                function_tool("memory_observe_memory", "Observe memory and swap state", "target"),
                function_tool("memory_diagnose_fault", "Diagnose memory faults", "target"),
                function_tool("processes_observe_process", "Observe system and per-process CPU state", "target"),
                function_tool("processes_diagnose_fault", "Diagnose process faults", "target"),
                function_tool("power_observe_thermal", "Observe thermal and power state", "target"),
                function_tool("power_diagnose_fault", "Diagnose thermal and power faults", "target"),
                function_tool("security_observe_security", "Observe security state", "target"),
                function_tool("security_diagnose_fault", "Diagnose security faults", "target"),
                function_tool("packages_observe_package", "Observe package state", "target"),
                function_tool("packages_diagnose_fault", "Diagnose package faults", "target"),
                function_tool("boot_observe_boot", "Observe boot and recovery state", "target"),
                function_tool("boot_diagnose_fault", "Diagnose boot and recovery faults", "target"),
            ]);
            body["tool_choice"] = json!("auto");
        }
        if let Some(seed) = request.seed {
            body["seed"] = json!(seed);
        }
        body
    }

    fn parse_response(&self, body: &str) -> Result<GenerationResponse, GenerationError> {
        let parsed: Value = serde_json::from_str(body)
            .map_err(|e| GenerationError::new(format!("bad response JSON: {e}"), false))?;
        let choices = parsed
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| GenerationError::new("response has no choices", false))?;
        let choice = choices
            .first()
            .ok_or_else(|| GenerationError::new("response has zero choices", false))?;
        let message = choice
            .get("message")
            .ok_or_else(|| GenerationError::new("choice has no message", false))?;
        let text = match message.get("content").and_then(Value::as_str) {
            Some(content) => content.to_string(),
            None => message
                .get("tool_calls")
                .map(|calls| json!({ "tool_calls": calls }).to_string())
                .ok_or_else(|| {
                    GenerationError::new(
                        "model returned no visible content; reasoning models can spend their \
                         whole token budget thinking — assign a non-reasoning model to this role",
                        false,
                    )
                })?,
        };
        let finish_reason = match choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .unwrap_or("stop")
        {
            "length" => FinishReason::Length,
            "content_filter" => FinishReason::Error,
            _ => FinishReason::Stop,
        };
        let usage = parsed.get("usage").and_then(Value::as_object);
        let tokens_used = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(Value::as_u64)
            .map(|n| n as u32)
            .or_else(|| {
                usage
                    .and_then(|u| u.get("completion_tokens"))
                    .and_then(Value::as_u64)
                    .map(|n| n as u32)
            })
            .unwrap_or(0);
        Ok(GenerationResponse {
            text,
            tokens_used,
            finish_reason,
            latency_ms: 0,
        })
    }

    fn agent(&self, timeout_ms: u64) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
    }
}

fn function_tool(name: &str, description: &str, argument: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {
                    argument: { "type": "string" }
                },
                "required": [argument],
                "additionalProperties": false
            }
        }
    })
}

fn function_tool_no_args(name: &str, description: &str) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }
        }
    })
}

impl ModelBackend for HttpBackend {
    fn provider_id(&self) -> &ProviderId {
        &self.provider
    }

    fn tier(&self) -> crate::model::ProviderTier {
        self.tier
    }

    fn is_healthy(&self) -> bool {
        // Some OpenAI-compatible gateways, including free-model routes, serve
        // chat completions but do not expose a usable GET /models endpoint.
        // Eligibility is therefore established by the real generation call;
        // transport and HTTP failures are still returned and recorded there.
        !self.endpoint.trim().is_empty()
    }

    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, GenerationError> {
        let agent = self.agent(self.http_timeout_ms);
        let body = self.request_body(request).to_string();
        let mut http = agent
            .post(&self.chat_completions_url())
            .set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            http = http.set("Authorization", &format!("Bearer {key}"));
        }
        let mut response_body = String::new();
        let result = match http.send_string(&body) {
            Ok(response) => {
                let text = response
                    .into_string()
                    .map_err(|e| GenerationError::new(format!("read response: {e}"), true))?;
                self.parse_response(&text)
            }
            Err(ureq::Error::Status(code, response)) => {
                let status_code: u16 = code;
                response_body = response.into_string().unwrap_or_default();
                Err(Self::status_error(status_code, response_body.clone()))
            }
            Err(ureq::Error::Transport(transport)) => Err(GenerationError::new(
                format!("transport error: {transport}"),
                true,
            )),
        };
        if let Err(error) = &result {
            // Dump exactly what we sent and what came back so provider-side
            // rejections can be replayed and diagnosed. Goes to stderr, which
            // is visible in the terminal while running tauri:dev.
            eprintln!(
                "Aios [{}] generation failed: {error}\nAios request body:\n{body}\nAios response body:\n{response_body}",
                self.provider
            );
        }
        result
    }
}

impl HttpBackend {
    /// Build the error for a non-2xx response. HTTP-level client errors stay
    /// non-recoverable, but some gateways report *upstream* provider
    /// failures as HTTP 400 while naming the failed provider in an `error.metadata`
    /// object (OpenRouter does this). The request itself was valid there, so
    /// those are transient and worth retrying.
    fn status_error(code: u16, body: String) -> GenerationError {
        let upstream_failure = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|parsed| parsed.get("error").and_then(|e| e.get("metadata")).cloned())
            .is_some();
        let snippet: String = body.chars().take(240).collect();
        let detail = if snippet.is_empty() {
            String::new()
        } else {
            format!(": {snippet}")
        };
        let recoverable = code >= 500 || code == 429 || upstream_failure;
        GenerationError::new(
            format!("provider returned HTTP {code}{detail}"),
            recoverable,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentRole, ModelMessage, ModelTask};
    use crate::protocol::DataClassification;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn request() -> GenerationRequest {
        GenerationRequest {
            task_id: uuid::Uuid::new_v4(),
            messages: vec![
                ModelMessage::new(ModelRole::System, "you are aios"),
                ModelMessage::new(ModelRole::User, "is my wifi ok?"),
            ],
            max_tokens: 64,
            temperature: 0.2,
            seed: None,
            model: None,
        }
    }

    struct TestServer {
        port: u16,
    }

    fn spawn_server(response_body: &'static str, status_line: &'static str) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                let _ = reader.read_line(&mut request_line);
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).expect("line") == 0 || line.trim().is_empty() {
                        break;
                    }
                    headers.push_str(&line);
                }
                if let Some(len) = headers.lines().find_map(|l| {
                    let lower = l.trim().to_lowercase();
                    lower
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().to_string())
                }) {
                    if let Ok(len) = len.parse::<usize>() {
                        let mut buf = vec![0u8; len];
                        let _ = reader.read_exact(&mut buf);
                    }
                }
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\n\r\n{response_body}"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        TestServer { port }
    }

    fn backend(server: &TestServer) -> HttpBackend {
        HttpBackend::new(
            ProviderId::new("test-provider"),
            "test-model".into(),
            format!("http://127.0.0.1:{}", server.port),
            None,
            ProviderTier::Internet,
            5000,
        )
    }

    #[test]
    fn upstream_provider_400_is_recoverable() {
        let body = r#"{"error":{"message":"Provider returned error","code":400,"metadata":{"raw":"ERROR","provider_name":"Stealth","is_byok":false}}}"#;
        assert!(HttpBackend::status_error(400, body.to_string()).recoverable);
        // A real request problem stays non-recoverable.
        let client_error =
            r#"{"error":{"message":"no endpoints found matching your request"}}"#;
        assert!(!HttpBackend::status_error(400, client_error.to_string()).recoverable);
        assert!(HttpBackend::status_error(502, String::new()).recoverable);
        assert!(!HttpBackend::status_error(401, String::new()).recoverable);
    }

    #[test]
    fn request_body_has_expected_shape() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        let body = backend.request_body(&request());
        assert_eq!(body["model"], "m");
        assert_eq!(body["max_tokens"], 64);
        assert_eq!(body["messages"].as_array().expect("array").len(), 2);
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body.get("seed").is_none());

        let mut with_tools = request();
        with_tools.messages[0].content = "Read-only machine tools are available".into();
        let body = backend.request_body(&with_tools);
        assert_eq!(body["tools"][0]["function"]["name"], "observe");
        assert_eq!(body["tool_choice"], "auto");

        let mut with_seed = request();
        with_seed.seed = Some(7);
        let body = backend.request_body(&with_seed);
        assert_eq!(body["seed"], 7);
    }

    #[test]
    fn temperature_is_rounded_for_the_wire() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        // serde_json widens f32 to f64; 0.3f32 must not reach the wire as
        // 0.30000001192092896 (providers reject that).
        let mut request = request();
        request.temperature = 0.3;
        let body = backend.request_body(&request).to_string();
        assert!(!body.contains("30000001192092896"));
        assert!(body.contains("\"temperature\":0.3"));

        request.temperature = 0.7;
        let body = backend.request_body(&request).to_string();
        assert!(body.contains("\"temperature\":0.7"));
    }

    #[test]
    fn parses_successful_response() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        let body = r#"{
            "choices": [
                {"message": {"content": "yes, wifi looks fine"}, "finish_reason": "stop"}
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 6, "total_tokens": 16}
        }"#;
        let response = backend.parse_response(body).expect("parse");
        assert_eq!(response.text, "yes, wifi looks fine");
        assert_eq!(response.tokens_used, 16);
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parses_length_finish_and_no_usage() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        let body = r#"{"choices": [{"message": {"content": "cut"}, "finish_reason": "length"}]}"#;
        let response = backend.parse_response(body).expect("parse");
        assert_eq!(response.finish_reason, FinishReason::Length);
        assert_eq!(response.tokens_used, 0);
    }

    #[test]
    fn parses_native_tool_calls_without_message_content() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        let body = r#"{"choices":[{"message":{"tool_calls":[{"function":{"name":"query","arguments":"{\"query\":\"memory\"}"}}]},"finish_reason":"tool_calls"}]}"#;
        let response = backend.parse_response(body).expect("tool call parses");
        assert!(response.text.contains("tool_calls"));
        assert!(response.text.contains("memory"));
    }

    #[test]
    fn empty_choices_is_error() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "https://x.example/v1".into(),
            None,
            ProviderTier::Internet,
            5000,
        );
        let err = backend
            .parse_response(r#"{"choices": []}"#)
            .expect_err("error");
        assert!(!err.recoverable);
    }

    #[test]
    fn generate_hits_endpoint_and_parses() {
        let server = spawn_server(
            r#"{"choices": [{"message": {"content": "hello from provider"}}]}"#,
            "HTTP/1.1 200 OK",
        );
        let backend = backend(&server);
        let response = backend.generate(&request()).expect("generate");
        assert_eq!(response.text, "hello from provider");
    }

    #[test]
    fn auth_header_sent_when_key_present() {
        let server = spawn_server(
            r#"{"choices": [{"message": {"content": "ok"}}]}"#,
            "HTTP/1.1 200 OK",
        );
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            format!("http://127.0.0.1:{}", server.port),
            Some("sk-test".into()),
            ProviderTier::Internet,
            5000,
        );
        let response = backend.generate(&request()).expect("generate");
        assert_eq!(response.text, "ok");
    }

    #[test]
    fn http_4xx_is_not_recoverable() {
        let server = spawn_server("{}", "HTTP/1.1 401 Unauthorized");
        let backend = backend(&server);
        let err = backend.generate(&request()).expect_err("error");
        assert!(!err.recoverable);
        assert!(err.message.contains("401"));
    }

    #[test]
    fn http_500_is_recoverable() {
        let server = spawn_server("{}", "HTTP/1.1 500 Internal Server Error");
        let backend = backend(&server);
        let err = backend.generate(&request()).expect_err("error");
        assert!(err.recoverable);
    }

    #[test]
    fn transport_failure_is_recoverable() {
        let backend = HttpBackend::new(
            ProviderId::new("p"),
            "m".into(),
            "http://127.0.0.1:1".into(),
            None,
            ProviderTier::Internet,
            500,
        );
        let err = backend.generate(&request()).expect_err("error");
        assert!(err.recoverable);
    }

    #[test]
    fn health_check_against_live_server() {
        let server = spawn_server("{}", "HTTP/1.1 200 OK");
        let backend = backend(&server);
        assert!(backend.is_healthy());
    }

    #[test]
    fn unused_model_task_wiring_compiles() {
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        assert_eq!(task.role, AgentRole::Planner);
    }
}
