//! openai --- the OpenAI-compatible wire; DeepSeek is the first endpoint.
//!
//! The client resolves its key at use, not at construction, and takes the base
//! URL: DeepSeek, a local server, or any compatible endpoint is the same code.

use crate::provider::{Provider, ProviderError};
use crate::types::{Completion, CompletionRequest, WireResponse};

/// An OpenAI-compatible endpoint.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// DeepSeek's public endpoint.
    pub fn deepseek(api_key: impl Into<String>) -> Self {
        Self::new("https://api.deepseek.com", api_key)
    }

    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

/// Parse a response body into a completion. Split out so the wire shape is
/// tested without a network.
pub(crate) fn parse_response(body: &str) -> Result<Completion, ProviderError> {
    let wire: WireResponse =
        serde_json::from_str(body).map_err(|e| ProviderError::Decode(e.to_string()))?;
    wire.into_completion().ok_or(ProviderError::EmptyResponse)
}

impl Provider for OpenAiProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        let body = response.text().await.map_err(|e| ProviderError::Transport(e.to_string()))?;
        parse_response(&body)
    }
}
