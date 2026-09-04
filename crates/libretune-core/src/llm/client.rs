//! [`LlmClient`] — owns the provider and is the single entry point the agent
//! orchestrator and Tauri commands use.

use crate::llm::provider::{build_provider, Provider, ProviderConfig};
use crate::llm::types::{ChatRequest, ChatResponse, LlmError};

/// The top-level LLM client. Construct it from user settings via
/// [`LlmClient::new`], then call [`LlmClient::chat`] per agent turn.
pub struct LlmClient {
    provider: Box<dyn Provider>,
}

impl LlmClient {
    /// Build a client from a [`ProviderConfig`] (typically read from settings).
    pub fn new(cfg: &ProviderConfig) -> Result<Self, LlmError> {
        Ok(Self {
            provider: build_provider(cfg)?,
        })
    }

    /// The provider name (e.g. "openai"), for logging / UI display.
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Send a chat-completion request.
    pub async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        self.provider.chat(req).await
    }

    /// Test-only constructor that wraps an arbitrary [`Provider`] (typically
    /// a mock/recording one) instead of building one from a
    /// [`ProviderConfig`]. Lets tests elsewhere in the crate (e.g.
    /// `agent::orchestrator`) observe exactly what gets sent to the model on
    /// each round of a turn, without a live provider.
    #[cfg(test)]
    pub(crate) fn from_provider(provider: Box<dyn Provider>) -> Self {
        Self { provider }
    }
}
