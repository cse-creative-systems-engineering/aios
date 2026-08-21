use crate::model::{ModelCapability, ProviderTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiosConfig {
    pub model: Option<ModelConfig>,
    #[serde(default)]
    pub provider: Vec<ProviderConfig>,
    #[serde(default)]
    pub shell: Option<ShellConfig>,
    #[serde(default)]
    pub roles: Option<RolesConfig>,
}

impl Default for AiosConfig {
    fn default() -> Self {
        Self {
            model: None,
            provider: Vec::new(),
            shell: None,
            roles: None,
        }
    }
}

impl AiosConfig {
    pub fn default_path() -> PathBuf {
        dirs_home().join(".aios").join("config.toml")
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = match std::env::var("AIOS_CONFIG") {
            Ok(p) => PathBuf::from(p),
            Err(_) => Self::default_path(),
        };
        Self::load_from(&path)
    }

    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        let source = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(ConfigError::Io {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                });
            }
        };
        let config: AiosConfig = toml::from_str(&source).map_err(|e| ConfigError::Parse {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut seen = std::collections::HashSet::new();
        for provider in &self.provider {
            if !seen.insert(provider.id.clone()) {
                return Err(ConfigError::DuplicateProvider(provider.id.clone()));
            }
            provider.validate()?;
        }
        if let Some(roles) = &self.roles {
            roles.validate(&self.provider)?;
        }
        Ok(())
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.provider.iter().find(|p| p.id == id)
    }

    pub fn provider_mut(&mut self, id: &str) -> Option<&mut ProviderConfig> {
        self.provider.iter_mut().find(|p| p.id == id)
    }

    /// Persist the config to `path`. Used by the settings panel so provider
    /// and role changes survive a restart. The write is atomic (temp file +
    /// rename) so a crash mid-write cannot leave a truncated config.
    pub fn save_to(&self, path: &PathBuf) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self).map_err(|e| ConfigError::Parse {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| ConfigError::Io {
            path: tmp.display().to_string(),
            reason: e.to_string(),
        })?;
        std::fs::rename(&tmp, path).map_err(|e| ConfigError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    /// Path of the config file this instance was loaded from (or the default
    /// path for a fresh config).
    pub fn source_path(&self) -> PathBuf {
        match std::env::var("AIOS_CONFIG") {
            Ok(p) => PathBuf::from(p),
            Err(_) => Self::default_path(),
        }
    }
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelConfig {
    pub path: Option<PathBuf>,
    #[serde(default = "default_ctx")]
    pub n_ctx: u32,
    #[serde(default = "default_threads")]
    pub n_threads: i32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            path: None,
            n_ctx: default_ctx(),
            n_threads: default_threads(),
            max_tokens: default_max_tokens(),
        }
    }
}

const fn default_ctx() -> u32 {
    4096
}

const fn default_threads() -> i32 {
    4
}

const fn default_max_tokens() -> u32 {
    1024
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: String,
    pub tier: String,
    #[serde(default)]
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default = "default_http_timeout_ms")]
    pub http_timeout_ms: u64,
}

const fn default_http_timeout_ms() -> u64 {
    10_000
}

impl ProviderConfig {
    pub fn local_default() -> Self {
        Self {
            id: "local".into(),
            kind: "local".into(),
            tier: "local".into(),
            model: None,
            endpoint: None,
            api_key: None,
            api_key_env: None,
            capabilities: None,
            http_timeout_ms: default_http_timeout_ms(),
        }
    }

    pub fn validate_pub(&self) -> Result<(), ConfigError> {
        self.validate()
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match self.kind.as_str() {
            "local" => {
                if self.endpoint.is_some() {
                    return Err(ConfigError::InvalidProvider(format!(
                        "provider '{}' is kind 'local' and must not set endpoint",
                        self.id
                    )));
                }
                if self.api_key.is_some() || self.api_key_env.is_some() {
                    return Err(ConfigError::InvalidProvider(format!(
                        "provider '{}' is kind 'local' and must not set an api key",
                        self.id
                    )));
                }
            }
            "openai-compatible" => {
                if self.endpoint.is_none() {
                    return Err(ConfigError::InvalidProvider(format!(
                        "provider '{}' (openai-compatible) needs an endpoint",
                        self.id
                    )));
                }
                // No model requirement: models come from live /models
                // discovery, and roles pick from that list in the settings
                // panel. The optional `model` field is only a fallback id for
                // registry seeding.
                let both_keys = self.api_key.is_some() && self.api_key_env.is_some();
                if both_keys {
                    return Err(ConfigError::InvalidProvider(format!(
                        "provider '{}' sets both api_key and api_key_env; use one",
                        self.id
                    )));
                }
            }
            other => {
                return Err(ConfigError::InvalidProvider(format!(
                    "provider '{}' has unknown kind '{other}'",
                    self.id
                )));
            }
        }
        parse_tier(&self.tier).map_err(|e| ConfigError::InvalidProvider(format!("{}: {e}", self.id)))?;
        Ok(())
    }

    pub fn tier(&self) -> Result<ProviderTier, ConfigError> {
        parse_tier(&self.tier)
    }

    pub fn effective_api_key(&self) -> Result<Option<String>, ConfigError> {
        if let Some(env_name) = &self.api_key_env {
            match std::env::var(env_name) {
                Ok(key) if !key.trim().is_empty() => return Ok(Some(key)),
                Ok(_) => {
                    return Err(ConfigError::InvalidProvider(format!(
                        "environment variable '{env_name}' is empty"
                    )));
                }
                Err(_) => {
                    return Err(ConfigError::InvalidProvider(format!(
                        "environment variable '{env_name}' is not set"
                    )));
                }
            }
        }
        Ok(self.api_key.clone())
    }

    pub fn capabilities(&self) -> Result<Vec<ModelCapability>, ConfigError> {
        match &self.capabilities {
            Some(list) => list
                .iter()
                .map(|name| parse_capability(name))
                .collect::<Result<Vec<_>, _>>(),
            None => Ok(vec![
                ModelCapability::TextGeneration,
                ModelCapability::Reasoning,
            ]),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShellConfig {
    #[serde(default = "default_history_len")]
    pub history_len: usize,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

const fn default_history_len() -> usize {
    20
}

/// Explicit provider/model assignment for one agent role. Both fields are
/// required: an assignment that names a provider but not a model (or vice
/// versa) is ambiguous and rejected at load (ADR-0003).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RoleAssignment {
    pub provider: String,
    pub model: String,
}

/// The `[roles]` config section. Each entry pins one role to one
/// provider/model pair chosen in the settings panel. Unlisted roles are
/// simply unassigned; nothing routes until the user picks a model.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RolesConfig {
    /// Generative surface composition (the canvas widget model).
    pub surface: Option<RoleAssignment>,
    /// Conversational chat, served by the planner agent.
    pub chat: Option<RoleAssignment>,
    /// Plan verification before execution.
    #[serde(default)]
    pub verification: Option<RoleAssignment>,
    /// Per-specialist assignments keyed by domain ("wifi", "storage", ...).
    /// Specialists are pure Rust tools today; these slots are where their
    /// model calls will land.
    #[serde(default)]
    pub specialists: HashMap<String, RoleAssignment>,
}

impl RolesConfig {
    fn validate(&self, providers: &[ProviderConfig]) -> Result<(), ConfigError> {
        let named = [
            ("surface", &self.surface),
            ("chat", &self.chat),
            ("verification", &self.verification),
        ];
        for (role, assignment) in named {
            Self::check_provider(providers, role, assignment.as_ref())?;
        }
        for (domain, assignment) in &self.specialists {
            Self::check_provider(providers, &format!("specialist:{domain}"), Some(assignment))?;
        }
        Ok(())
    }

    fn check_provider(
        providers: &[ProviderConfig],
        role: &str,
        assignment: Option<&RoleAssignment>,
    ) -> Result<(), ConfigError> {
        let Some(assignment) = assignment else {
            return Ok(());
        };
        providers
            .iter()
            .find(|p| p.id == assignment.provider)
            .map(|_| ())
            .ok_or_else(|| {
                ConfigError::InvalidRole(format!(
                    "roles.{role}: provider '{}' is not configured",
                    assignment.provider
                ))
            })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io { path: String, reason: String },
    Parse { path: String, reason: String },
    DuplicateProvider(String),
    InvalidProvider(String),
    InvalidRole(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, reason } => {
                write!(f, "cannot read config {path}: {reason}")
            }
            ConfigError::Parse { path, reason } => {
                write!(f, "config {path} is not valid TOML: {reason}")
            }
            ConfigError::DuplicateProvider(id) => {
                write!(f, "provider declared twice in config: {id}")
            }
            ConfigError::InvalidProvider(reason) => write!(f, "invalid provider: {reason}"),
            ConfigError::InvalidRole(reason) => write!(f, "invalid role assignment: {reason}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn parse_tier(tier: &str) -> Result<ProviderTier, ConfigError> {
    match tier {
        "local" => Ok(ProviderTier::Local),
        "lan" => Ok(ProviderTier::Lan),
        "internet" => Ok(ProviderTier::Internet),
        other => Err(ConfigError::InvalidProvider(format!(
            "unknown tier '{other}' (expected local, lan, or internet)"
        ))),
    }
}

fn parse_capability(name: &str) -> Result<ModelCapability, ConfigError> {
    match name {
        "text-generation" => Ok(ModelCapability::TextGeneration),
        "tool-use" => Ok(ModelCapability::ToolUse),
        "code-generation" => Ok(ModelCapability::CodeGeneration),
        "reasoning" => Ok(ModelCapability::Reasoning),
        "multimodal" => Ok(ModelCapability::Multimodal),
        other => Err(ConfigError::InvalidProvider(format!(
            "unknown capability '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(dir: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        let mut file = std::fs::File::create(&path).expect("create");
        file.write_all(body.as_bytes()).expect("write");
        path
    }

    #[test]
    fn missing_config_is_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.toml");
        let config = AiosConfig::load_from(&path).expect("load");
        assert!(config.provider.is_empty());
        assert!(config.model.is_none());
    }

    #[test]
    fn parses_full_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[model]
path = "models/qwen.gguf"
n_ctx = 8192
n_threads = 8
max_tokens = 512

[[provider]]
id = "local"
kind = "local"
tier = "local"

[[provider]]
id = "deepseek"
kind = "openai-compatible"
tier = "internet"
endpoint = "https://api.deepseek.com/v1"
model = "deepseek-chat"
api_key_env = "AIOS_DEEPSEEK_KEY"
capabilities = ["text-generation", "reasoning"]
"#;
        let path = write_config(&dir, body);
        let config = AiosConfig::load_from(&path).expect("load");
        let model = config.model.as_ref().expect("model");
        assert_eq!(model.n_ctx, 8192);
        assert_eq!(model.n_threads, 8);
        assert_eq!(model.max_tokens, 512);
        assert_eq!(config.provider.len(), 2);

        let local = config.provider("local").expect("local provider");
        assert_eq!(local.tier().expect("tier"), ProviderTier::Local);
        assert_eq!(
            local.capabilities().expect("caps"),
            vec![ModelCapability::TextGeneration, ModelCapability::Reasoning]
        );

        let deepseek = config.provider("deepseek").expect("deepseek provider");
        assert_eq!(deepseek.tier().expect("tier"), ProviderTier::Internet);
        assert_eq!(deepseek.api_key_env.as_deref(), Some("AIOS_DEEPSEEK_KEY"));
    }

    #[test]
    fn rejects_duplicate_provider() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "local"
kind = "local"
tier = "local"

[[provider]]
id = "local"
kind = "openai-compatible"
tier = "internet"
endpoint = "https://x.example/v1"
model = "m"
"#;
        let path = write_config(&dir, body);
        assert!(matches!(
            AiosConfig::load_from(&path),
            Err(ConfigError::DuplicateProvider(_))
        ));
    }

    #[test]
    fn rejects_unknown_kind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "x"
kind = "magic"
tier = "local"
"#;
        let path = write_config(&dir, body);
        assert!(matches!(
            AiosConfig::load_from(&path),
            Err(ConfigError::InvalidProvider(_))
        ));
    }

    #[test]
    fn rejects_bad_tier() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "x"
kind = "openai-compatible"
tier = "space"
endpoint = "https://x.example/v1"
model = "m"
"#;
        let path = write_config(&dir, body);
        assert!(matches!(
            AiosConfig::load_from(&path),
            Err(ConfigError::InvalidProvider(_))
        ));
    }

    #[test]
    fn rejects_http_provider_without_endpoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "x"
kind = "openai-compatible"
tier = "internet"
model = "m"
"#;
        let path = write_config(&dir, body);
        assert!(matches!(
            AiosConfig::load_from(&path),
            Err(ConfigError::InvalidProvider(_))
        ));
    }

    #[test]
    fn rejects_local_provider_with_endpoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "local"
kind = "local"
tier = "local"
endpoint = "https://x.example/v1"
"#;
        let path = write_config(&dir, body);
        assert!(matches!(
            AiosConfig::load_from(&path),
            Err(ConfigError::InvalidProvider(_))
        ));
    }

    #[test]
    fn api_key_resolved_from_env() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = r#"
[[provider]]
id = "ling"
kind = "openai-compatible"
tier = "internet"
endpoint = "https://ling.example/v1"
model = "ling-1"
api_key_env = "AIOS_TEST_LING_KEY"
"#;
        let path = write_config(&dir, body);
        let config = AiosConfig::load_from(&path).expect("load");
        let provider = config.provider("ling").expect("provider");

        unsafe {
            std::env::set_var("AIOS_TEST_LING_KEY", "sekrit");
        }
        assert_eq!(
            provider.effective_api_key().expect("key"),
            Some("sekrit".to_string())
        );
        unsafe {
            std::env::remove_var("AIOS_TEST_LING_KEY");
        }

        assert!(matches!(
            provider.effective_api_key(),
            Err(ConfigError::InvalidProvider(_))
        ));
    }
}
