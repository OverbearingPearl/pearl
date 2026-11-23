//! provider --- one interface, not one adapter per API.
//!
//! Every backend speaks the same trait. The first implementation is
//! OpenAI-compatible (DeepSeek); a new provider is a new wire under the same
//! interface, not a new shape the loop has to learn.

use thiserror::Error;

use crate::types::{Completion, CompletionRequest};

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("transport: {0}")]
    Transport(String),
    #[error("the response had no usable choice")]
    EmptyResponse,
    #[error("decode: {0}")]
    Decode(String),
}

/// A model backend. The loop calls `complete`; it does not know which wire is
/// underneath. Async by trait so a mock can stand in for the network in tests.
pub trait Provider {
    fn complete(
        &self,
        request: CompletionRequest,
    ) -> impl std::future::Future<Output = Result<Completion, ProviderError>> + Send;
}
