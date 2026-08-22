use crate::model::{
    FinishReason, GenerationError, GenerationRequest, GenerationResponse, ModelBackend, ModelId,
    ModelMessage, ModelRole, ProviderId,
};
use llama_cpp_2::TokenToStringError;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use std::num::NonZeroU32;
use std::path::Path;

#[derive(Debug)]
pub enum LocalModelError {
    Backend(String),
    Load { path: String, reason: String },
    Template(String),
    Tokenize(String),
    Decode(String),
    Context(String),
    NoMessages,
}

impl std::fmt::Display for LocalModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalModelError::Backend(reason) => write!(f, "llama backend init failed: {reason}"),
            LocalModelError::Load { path, reason } => {
                write!(f, "failed to load model from {path}: {reason}")
            }
            LocalModelError::Template(reason) => write!(f, "chat template failed: {reason}"),
            LocalModelError::Tokenize(reason) => write!(f, "tokenization failed: {reason}"),
            LocalModelError::Decode(reason) => write!(f, "decode failed: {reason}"),
            LocalModelError::Context(reason) => write!(f, "context setup failed: {reason}"),
            LocalModelError::NoMessages => write!(f, "request has no messages"),
        }
    }
}

impl std::error::Error for LocalModelError {}

pub struct LocalLlama {
    provider: ProviderId,
    model_id: ModelId,
    backend: LlamaBackend,
    model: LlamaModel,
    n_ctx: u32,
    n_threads: i32,
}

fn chat_messages(messages: &[ModelMessage]) -> Result<Vec<LlamaChatMessage>, LocalModelError> {
    if messages.is_empty() {
        return Err(LocalModelError::NoMessages);
    }
    messages
        .iter()
        .map(|message| {
            let role = match message.role {
                ModelRole::System => "system",
                ModelRole::User => "user",
                ModelRole::Assistant => "assistant",
            };
            LlamaChatMessage::new(role.into(), message.content.clone())
        })
        .collect::<Result<Vec<LlamaChatMessage>, _>>()
        .map_err(|e| LocalModelError::Template(e.to_string()))
}

impl LocalLlama {
    pub fn load(
        provider: ProviderId,
        model_id: ModelId,
        path: impl AsRef<Path>,
        n_ctx: u32,
        n_threads: i32,
    ) -> Result<Self, LocalModelError> {
        let mut backend =
            LlamaBackend::init().map_err(|e| LocalModelError::Backend(e.to_string()))?;
        backend.void_logs();
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, path.as_ref(), &params).map_err(|e| {
            LocalModelError::Load {
                path: path.as_ref().display().to_string(),
                reason: e.to_string(),
            }
        })?;
        Ok(Self {
            provider,
            model_id,
            backend,
            model,
            n_ctx,
            n_threads,
        })
    }

    pub fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn token_to_bytes(&self, token: LlamaToken) -> Result<Vec<u8>, LocalModelError> {
        let mut size = 8;
        loop {
            match self.model.token_to_piece_bytes(token, size, false, None) {
                Ok(bytes) => return Ok(bytes),
                Err(TokenToStringError::InsufficientBufferSpace(needed)) => {
                    size = usize::try_from(-needed).expect("needed size fits in usize");
                }
                Err(e) => return Err(LocalModelError::Tokenize(e.to_string())),
            }
        }
    }

    fn build_prompt(&self, messages: &[ModelMessage]) -> Result<String, LocalModelError> {
        let chat = chat_messages(messages)?;
        let template = self
            .model
            .chat_template(None)
            .map_err(|e| LocalModelError::Template(e.to_string()))?;
        self.model
            .apply_chat_template(&template, &chat, true)
            .map_err(|e| LocalModelError::Template(e.to_string()))
    }

    pub fn generate_inner(
        &self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, LocalModelError> {
        let prompt = self.build_prompt(&request.messages)?;
        let tokens = self
            .model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| LocalModelError::Tokenize(e.to_string()))?;

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.n_ctx))
            .with_n_batch(512)
            .with_n_threads(self.n_threads);
        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| LocalModelError::Context(e.to_string()))?;

        let prompt_len = tokens.len() as i32;
        let budget = (self.n_ctx as i32 - prompt_len - 4).max(0);
        if budget <= 0 {
            return Err(LocalModelError::Context(format!(
                "prompt ({prompt_len} tokens) exceeds context window ({} tokens)",
                self.n_ctx
            )));
        }
        let max_new = request.max_tokens.min(budget as u32) as usize;
        let batch_size = 512usize;
        let mut batch = LlamaBatch::new(batch_size, 1);
        for (chunk_index, chunk) in tokens.chunks(batch_size).enumerate() {
            batch.clear();
            let offset = chunk_index * batch_size;
            for (index, token) in chunk.iter().enumerate() {
                let is_last = offset + index == tokens.len() - 1;
                batch
                    .add(*token, (offset + index) as i32, &[0], is_last)
                    .map_err(|e| LocalModelError::Context(e.to_string()))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| LocalModelError::Decode(e.to_string()))?;
        }

        let mut samplers = vec![LlamaSampler::top_k(40)];
        if request.temperature > 0.0 {
            samplers.push(LlamaSampler::temp(request.temperature));
            samplers.push(LlamaSampler::dist(request.seed.unwrap_or(0) as u32));
        } else {
            samplers.push(LlamaSampler::greedy());
        }
        let mut sampler = LlamaSampler::chain_simple(samplers);

        let mut out = String::new();
        let mut tokens_used = tokens.len() as u32;
        let mut finish_reason = FinishReason::Length;
        let mut pos = tokens.len();
        let mut idx = batch.n_tokens() - 1;
        for _ in 0..max_new {
            let token = sampler.sample(&mut ctx, idx);
            tokens_used += 1;
            if self.model.is_eog_token(token) {
                finish_reason = FinishReason::Stop;
                break;
            }
            let piece = self
                .token_to_bytes(token)
                .map_err(|e| LocalModelError::Tokenize(e.to_string()))?;
            out.push_str(&String::from_utf8_lossy(&piece));
            sampler.accept(token);
            batch.clear();
            batch
                .add(token, pos as i32, &[0], true)
                .map_err(|e| LocalModelError::Context(e.to_string()))?;
            ctx.decode(&mut batch)
                .map_err(|e| LocalModelError::Decode(e.to_string()))?;
            pos += 1;
            idx = 0;
        }

        Ok(GenerationResponse {
            text: out,
            tokens_used,
            finish_reason,
            latency_ms: 0,
        })
    }
}

impl ModelBackend for LocalLlama {
    fn provider_id(&self) -> &ProviderId {
        &self.provider
    }

    fn tier(&self) -> crate::model::ProviderTier {
        crate::model::ProviderTier::Local
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, GenerationError> {
        self.generate_inner(request)
            .map_err(|e| GenerationError::new(e.to_string(), false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GenerationRequest {
        GenerationRequest {
            task_id: uuid::Uuid::new_v4(),
            messages: vec![
                ModelMessage::new(ModelRole::System, "You are a concise assistant."),
                ModelMessage::new(ModelRole::User, "Say hello in one word."),
            ],
            max_tokens: 32,
            temperature: 0.0,
            seed: Some(0),
            model: None,
        reasoning_disabled: false,
        }
    }

    #[test]
    fn chat_messages_rejects_empty() {
        assert!(matches!(
            chat_messages(&[]),
            Err(LocalModelError::NoMessages)
        ));
    }

    #[test]
    fn chat_messages_maps_roles() {
        let messages = vec![
            ModelMessage::new(ModelRole::System, "sys"),
            ModelMessage::new(ModelRole::User, "usr"),
            ModelMessage::new(ModelRole::Assistant, "ast"),
        ];
        let chat = chat_messages(&messages).expect("chat messages");
        assert_eq!(chat.len(), 3);
    }

    #[test]
    #[ignore = "requires a real GGUF model; set AIOS_MODEL_PATH"]
    fn loads_and_generates_real_model() {
        let path = std::env::var("AIOS_MODEL_PATH").expect("AIOS_MODEL_PATH must be set");
        let llama = LocalLlama::load(
            ProviderId::local(),
            ModelId::new("local-qwen"),
            &path,
            2048,
            4,
        )
        .expect("load model");
        assert_eq!(llama.provider_id(), &ProviderId::local());
        let response = llama.generate(&request()).expect("generate");
        eprintln!("real model response: {:?}", response.text);
        assert!(!response.text.trim().is_empty());
        assert!(response.tokens_used > 0);
    }
}
