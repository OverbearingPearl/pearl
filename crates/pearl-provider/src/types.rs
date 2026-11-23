//! types --- the request and response shapes on the wire.

use serde::{Deserialize, Serialize};

/// The role of a chat message, as the OpenAI-compatible wire names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self { role, content: content.into() }
    }
}

/// A completion request. The model is chosen per request, not stored on the
/// client, so a task can pick its own model.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
}

/// A finished completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub content: String,
    pub model: String,
}

/// The subset of the response JSON we read. Kept separate from the public
/// `Completion` so the wire shape can be tested without a network.
#[derive(Debug, Deserialize)]
pub(crate) struct WireResponse {
    pub model: String,
    pub choices: Vec<WireChoice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WireChoice {
    pub message: WireMessage,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WireMessage {
    pub content: String,
}

impl WireResponse {
    pub fn into_completion(self) -> Option<Completion> {
        let choice = self.choices.into_iter().next()?;
        Some(Completion { content: choice.message.content, model: self.model })
    }
}
