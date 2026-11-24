//! openai --- the OpenAI-compatible wire; DeepSeek is the first endpoint.
//!
//! It sends the key it was given and takes the base URL: DeepSeek, a local
//! server, or any compatible endpoint is the same code. Finding the key at the
//! moment of the request is `Houses`' job, not this one's -- an empty key here
//! means the endpoint asked for none, and no Authorization header is sent.

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
pub fn parse_response(body: &str) -> Result<Completion, ProviderError> {
    let wire: WireResponse =
        serde_json::from_str(body).map_err(|e| ProviderError::Decode(e.to_string()))?;
    wire.into_completion().ok_or(ProviderError::EmptyResponse)
}

impl Provider for OpenAiProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let mut call = self.client.post(self.endpoint());
        if !self.api_key.is_empty() {
            call = call.bearer_auth(&self.api_key);
        }
        let response = call
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(ProviderError::Refused {
                status: status.as_u16(),
                message: refusal_message(&body),
            });
        }
        parse_response(&body)
    }
}

/// What the endpoint said, in its words. Most houses put it in
/// `error.message`; a proxy may put anything, so the body itself is the
/// fallback rather than a generic complaint of our own.
fn refusal_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(m) = v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
            return m.to_string();
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() { String::from("no explanation given") } else { trimmed.chars().take(400).collect() }
}
