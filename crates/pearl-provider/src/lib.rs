//! pearl-provider --- one provider interface over an OpenAI-compatible wire.
//!
//! The model is chosen per request and the key resolved at use. DeepSeek is the
//! first endpoint; any OpenAI-compatible server is the same code with a
//! different base URL.

pub mod openai;
pub mod provider;
pub mod types;

pub use openai::OpenAiProvider;
pub use provider::{Provider, ProviderError};
pub use types::{
    ChatMessage, Completion, CompletionRequest, Function, FunctionSpec, Role, ToolCall, ToolSpec,
    Usage,
};
