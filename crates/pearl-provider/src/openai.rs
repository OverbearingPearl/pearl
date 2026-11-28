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

/// One client for the process. A client owns the connection pool and the TLS
/// configuration, so building one per request pays a handshake on every round
/// trip and leaves the pool behind for the allocator to keep. The endpoint is
/// still read per request -- the model can change under a running window --
/// but the thing that carries it does not have to be rebuilt to do that.
fn shared_client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new).clone()
}

impl OpenAiProvider {
    /// DeepSeek's public endpoint.
    pub fn deepseek(api_key: impl Into<String>) -> Self {
        Self::new("https://api.deepseek.com", api_key)
    }

    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        // A copied address or key arrives with the line break the terminal
        // appended, and reqwest reports that as the same opaque builder error
        // it uses for every malformed request. The break is never part of
        // either value, so it is trimmed here rather than refused.
        Self {
            base_url: base_url.into().trim().trim_end_matches('/').to_string(),
            api_key: api_key.into().trim().to_string(),
            client: shared_client(),
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
        let endpoint = self.endpoint();
        // Name the part that is wrong. reqwest turns a malformed address, a
        // header value it cannot carry, and a body it cannot serialize into
        // the same `builder error`, which says nothing about what to change.
        let url = reqwest::Url::parse(&endpoint).map_err(|e| {
            ProviderError::Transport(format!(
                "the address {endpoint:?} is not a URL a request can go to ({e}); write it with https:// and no line break"
            ))
        })?;
        let mut call = self.client.post(url);
        if !self.api_key.is_empty() {
            let house = crate::auth::provider_name(&crate::auth::host_of(&self.base_url));
            let header = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|_| {
                    ProviderError::Transport(format!(
                        "the key for {house} cannot travel in an HTTP header; it holds a line break or a character outside ASCII, so export it as one line"
                    ))
                })?;
            call = call.header(reqwest::header::AUTHORIZATION, header);
        }
        let response = call
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
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
        if let Some(m) = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return m.to_string();
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        String::from("no explanation given")
    } else {
        trimmed.chars().take(400).collect()
    }
}
