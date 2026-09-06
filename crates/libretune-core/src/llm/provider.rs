//! The [`Provider`] trait and a factory that selects a concrete provider.

use crate::llm::types::{ChatRequest, ChatResponse, LlmError};
use async_trait::async_trait;
use std::time::Duration;

/// Total request timeout for the shared LLM HTTP client. Generous on
/// purpose: a legitimate streaming completion can run long, and this is a
/// ceiling on the whole request/response, not a per-chunk idle timeout.
const LLM_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// TCP connect timeout for the shared LLM HTTP client. Much shorter than
/// [`LLM_REQUEST_TIMEOUT`] since a healthy endpoint should accept a
/// connection almost immediately — a stalled connect attempt indicates a
/// dead/unreachable host, not a slow-but-working one.
const LLM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Builds the `reqwest::Client` shared by every provider, with an explicit
/// request/connect timeout so a stalled endpoint can't hang an agent turn
/// forever (see [`build_http_client`] for the production timeouts; tests use
/// [`build_http_client_with_timeouts`] directly with short overrides so they
/// don't have to wait out the real ones).
fn build_http_client() -> Result<reqwest::Client, LlmError> {
    build_http_client_with_timeouts(LLM_REQUEST_TIMEOUT, LLM_CONNECT_TIMEOUT)
}

fn build_http_client_with_timeouts(
    request_timeout: Duration,
    connect_timeout: Duration,
) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .user_agent("LibreTune/0.1 (AI-Assistant)")
        .timeout(request_timeout)
        .connect_timeout(connect_timeout)
        .build()
        .map_err(|e| LlmError::Config(format!("failed to build HTTP client: {e}")))
}

/// All configuration needed to talk to one provider, in a provider-agnostic
/// form. This is what gets stored in user settings.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    /// Which provider protocol to use: "openai", "anthropic", "google".
    pub provider: String,
    /// Base URL (e.g. `https://api.openai.com/v1` for OpenAI,
    /// `http://localhost:11434/v1` for a local Ollama exposing an
    /// OpenAI-compatible endpoint).
    pub base_url: String,
    /// API key / bearer token. Empty for local no-auth providers.
    pub api_key: String,
    /// Model identifier (e.g. `gpt-4o`, `claude-3-5-sonnet-...`, `gemini-1.5-pro`).
    pub model: String,
}

/// A chat-completion provider. Implementations translate [`ChatRequest`] to
/// their wire format, call the endpoint, and parse the response back into
/// [`ChatResponse`].
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (matches [`ProviderConfig::provider`]).
    fn name(&self) -> &str;

    /// Send a chat-completion request.
    async fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}

/// Construct the concrete provider for a [`ProviderConfig`].
///
/// Returns [`LlmError::Config`] for an unknown provider id. The factory owns
/// its own `reqwest::Client` (built once, reused) so callers don't have to
/// thread one through.
pub fn build_provider(cfg: &ProviderConfig) -> Result<Box<dyn Provider>, LlmError> {
    let client = build_http_client()?;

    match cfg.provider.to_lowercase().as_str() {
        "openai" | "" => Ok(Box::new(
            crate::llm::providers::openai::OpenAiProvider::new(
                client,
                cfg.base_url.clone(),
                cfg.api_key.clone(),
                cfg.model.clone(),
            ),
        )),
        "anthropic" | "claude" => Ok(Box::new(
            crate::llm::providers::anthropic::AnthropicProvider::new(
                client,
                cfg.base_url.clone(),
                cfg.api_key.clone(),
                cfg.model.clone(),
            ),
        )),
        "google" | "gemini" => Ok(Box::new(
            crate::llm::providers::google::GoogleProvider::new(
                client,
                cfg.base_url.clone(),
                cfg.api_key.clone(),
                cfg.model.clone(),
            ),
        )),
        other => Err(LlmError::Config(format!(
            "unknown provider '{other}' (expected: openai, anthropic, google)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_build_provider_succeeds_for_known_providers() {
        for provider in ["openai", "anthropic", "google"] {
            let cfg = ProviderConfig {
                provider: provider.to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: String::new(),
                model: "test-model".to_string(),
            };
            assert!(
                build_provider(&cfg).is_ok(),
                "expected provider '{provider}' to build successfully"
            );
        }
    }

    /// Exercises the exact `reqwest::ClientBuilder` chain `build_provider`
    /// uses (short-overridden so the test doesn't have to wait out the real
    /// 10s production connect timeout) against a non-routable address, which
    /// silently drops the connection attempt rather than refusing it — the
    /// only way to observe whether `connect_timeout` is actually wired in
    /// rather than the client hanging forever.
    #[tokio::test]
    async fn test_client_enforces_connect_timeout() {
        let client =
            build_http_client_with_timeouts(Duration::from_millis(500), Duration::from_millis(500))
                .expect("client should build");

        let start = Instant::now();
        // 10.255.255.1 is a non-routable address: the OS attempts the TCP
        // handshake and gets no response, so the connection genuinely hangs
        // until something (here, connect_timeout) gives up on it.
        let result = client.get("http://10.255.255.1/").send().await;
        let elapsed = start.elapsed();

        assert!(
            result.is_err(),
            "expected the connect timeout to abort the request"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "connect_timeout should bound the wait to well under 5s, took {elapsed:?}"
        );
    }
}
