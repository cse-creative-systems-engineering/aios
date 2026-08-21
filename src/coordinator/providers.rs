use super::*;

impl Coordinator {
    /// Add a provider to the live registry and persist it to config. The
    /// backend (HTTP or local) is built immediately so the provider is
    /// usable without a restart.
    pub fn add_provider(
        &mut self,
        id: String,
        kind: String,
        tier: String,
        endpoint: Option<String>,
        model: Option<String>,
        api_key: Option<String>,
        http_timeout_ms: Option<u64>,
    ) -> Result<(), String> {
        let provider = crate::config::ProviderConfig {
            id: id.clone(),
            kind: kind.clone(),
            tier,
            model: model.clone(),
            endpoint: endpoint.clone(),
            api_key,
            api_key_env: None,
            capabilities: None,
            http_timeout_ms: http_timeout_ms.unwrap_or(10_000),
        };
        provider.validate_pub().map_err(|e| e.to_string())?;
        if self.config.provider(&id).is_some() {
            return Err(format!("provider '{id}' already exists"));
        }

        // Build and register the backend now so the provider is routable.
        let tier_parsed = provider.tier().map_err(|e| e.to_string())?;
        let capabilities = provider.capabilities().map_err(|e| e.to_string())?;
        let provider_id = ProviderId::new(&id);
        let model_name = model.clone().unwrap_or_else(|| id.clone());
        let model_id = ModelId::new(&model_name);
        match kind.as_str() {
            "openai-compatible" => {
                let endpoint = endpoint.clone().ok_or_else(|| {
                    format!("provider '{id}' (openai-compatible) needs an endpoint")
                })?;
                let api_key = provider.effective_api_key().map_err(|e| e.to_string())?;
                let backend = HttpBackend::new(
                    provider_id.clone(),
                    model_name,
                    endpoint,
                    api_key,
                    tier_parsed,
                    provider.http_timeout_ms,
                );
                let entry = ModelEntry::new(model_id, provider_id.clone(), tier_parsed, capabilities);
                self.registry
                    .write()
                    .expect("registry lock")
                    .register(entry)
                    .map_err(|e| e.to_string())?;
                self.gateway.register_backend(Arc::new(backend));
            }
            other => {
                return Err(format!(
                    "provider kind '{other}' cannot be added at runtime (only openai-compatible)"
                ));
            }
        }

        // Grant the session's machine-state consent scope so the provider is
        // immediately usable for chat, matching boot behavior.
        let _ = self.gateway.router().grant_consent(crate::model::ConsentRecord::new(
            provider_id,
            vec![DataClassification::SystemConfig],
        ));

        self.config.provider.push(provider);
        self.persist_config()?;
        // Discover the model list now so the settings panel can populate the
        // role dropdowns. A failure is kept in the catalogue and shown there;
        // it does not fail the add itself.
        let _ = self.refresh_catalogue(&id);
        Ok(())
    }

    /// Remove a provider from the live registry and config. Role assignments
    /// referencing it are dropped; those roles report as unassigned until the
    /// user picks something else.
    pub fn remove_provider(&mut self, id: &str) -> Result<(), String> {
        let before = self.config.provider.len();
        self.config.provider.retain(|p| p.id != id);
        if self.config.provider.len() == before {
            return Err(format!("provider '{id}' is not configured"));
        }
        if let Some(roles) = &mut self.config.roles {
            if roles.surface.as_ref().is_some_and(|a| a.provider == id) {
                roles.surface = None;
            }
            if roles.chat.as_ref().is_some_and(|a| a.provider == id) {
                roles.chat = None;
            }
            if roles
                .verification
                .as_ref()
                .is_some_and(|a| a.provider == id)
            {
                roles.verification = None;
            }
            roles
                .specialists
                .retain(|_, a| a.provider != id);
        }
        let provider_id = ProviderId::new(id);
        for descriptor in assignable_roles() {
            if self
                .gateway
                .router()
                .assignment(&descriptor.id)
                .is_some_and(|(p, _)| p == provider_id)
            {
                self.gateway.router().clear_assignment(&descriptor.id);
            }
        }
        self.catalogue
            .write()
            .expect("catalogue lock")
            .remove(id);
        self.persist_config()
    }

    /// Store a credential for a provider. Write-only: the key is persisted
    /// to config and never returned to the frontend. The model catalogue is
    /// refreshed so the dropdowns reflect what the key can actually see.
    pub fn set_provider_credential(&mut self, id: &str, api_key: String) -> Result<(), String> {
        let provider = self
            .config
            .provider_mut(id)
            .ok_or_else(|| format!("provider '{id}' is not configured"))?;
        provider.api_key_env = None;
        provider.api_key = Some(api_key);
        self.persist_config()?;
        let _ = self.refresh_catalogue(id);
        Ok(())
    }

    /// Refresh one provider's model catalogue and remember the outcome,
    /// including the failure reason. Returns the models on success.
    pub fn refresh_catalogue(&mut self, provider_id: &str) -> Result<Vec<DiscoveredModel>, String> {
        let result = self.fetch_models(provider_id);
        let entry = ProviderCatalogue::from_result(result.clone());
        self.catalogue
            .write()
            .expect("catalogue lock")
            .insert(provider_id.to_string(), entry);
        result
    }

    /// Fetch the live model list from a configured provider's /models
    /// endpoint, caching the outcome (including failures) for the settings
    /// panel. The provider must already be configured with an API key.
    pub fn discover_models(&mut self, provider_id: &str) -> Result<Vec<DiscoveredModel>, String> {
        self.refresh_catalogue(provider_id)
    }

    /// Raw /models fetch with no caching. Callers that need the result to be
    /// remembered should go through `refresh_catalogue`.
    fn fetch_models(&self, provider_id: &str) -> Result<Vec<DiscoveredModel>, String> {
        let provider = self
            .config
            .provider(provider_id)
            .ok_or_else(|| format!("provider '{provider_id}' is not configured"))?;
        let endpoint = provider
            .endpoint
            .as_ref()
            .ok_or_else(|| format!("provider '{provider_id}' has no endpoint"))?;
        let api_key = provider
            .effective_api_key()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("provider '{provider_id}' has no API key"))?;
        let url = format!("{}/models", endpoint.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_millis(provider.http_timeout_ms))
            .build();
        let mut req = agent.get(&url);
        req = req.set("Authorization", &format!("Bearer {api_key}"));
        let response = req
            .call()
            .map_err(|e| format!("models request failed: {e}"))?;
        let body: Value = response
            .into_string()
            .map_err(|e| format!("cannot read models response: {e}"))
            .and_then(|text| {
                serde_json::from_str(&text)
                    .map_err(|e| format!("cannot parse models response: {e}"))
            })?;
        let data = body
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| "models response has no 'data' array".to_string())?;
        let models = data
            .iter()
            .map(|entry| DiscoveredModel {
                id: entry
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                name: entry
                    .get("name")
                    .and_then(Value::as_str)
                    .map(String::from),
            })
            .collect();
        Ok(models)
    }
}

/// A pre-configured OpenAI-compatible provider the user can pick from in the
/// settings panel. The endpoint is filled in automatically; the user only
/// enters an API key.
pub struct CatalogProvider {
    pub id: &'static str,
    pub label: &'static str,
    pub endpoint: &'static str,
    pub kind: &'static str,
    pub tier: &'static str,
}

/// The provider catalog. These are the most common OpenAI-compatible
/// providers. Adding a provider is a config change, not a code change, once
/// the panel supports arbitrary endpoints.
pub const PROVIDER_CATALOG: &[CatalogProvider] = &[
    CatalogProvider {
        id: "openrouter",
        label: "OpenRouter",
        endpoint: "https://openrouter.ai/api/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "openai",
        label: "OpenAI",
        endpoint: "https://api.openai.com/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "anthropic",
        label: "Anthropic",
        endpoint: "https://api.anthropic.com/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "groq",
        label: "Groq",
        endpoint: "https://api.groq.com/openai/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "together",
        label: "Together AI",
        endpoint: "https://api.together.xyz/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "fireworks",
        label: "Fireworks AI",
        endpoint: "https://api.fireworks.ai/inference/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "mistral",
        label: "Mistral AI",
        endpoint: "https://api.mistral.ai/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "deepinfra",
        label: "DeepInfra",
        endpoint: "https://api.deepinfra.com/v1/openai",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "novita",
        label: "Novita AI",
        endpoint: "https://api.novita.ai/v3/openai",
        kind: "openai-compatible",
        tier: "internet",
    },
    CatalogProvider {
        id: "hyperbolic",
        label: "Hyperbolic",
        endpoint: "https://api.hyperbolic.xyz/v1",
        kind: "openai-compatible",
        tier: "internet",
    },
];
/// A model discovered live from a provider's /models endpoint.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub id: String,
    pub name: Option<String>,
}
