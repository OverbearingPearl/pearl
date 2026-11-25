//! tool --- the one shape every tool the loop can call takes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("the anchor text was not found")]
    AnchorNotFound,
    #[error("the anchor text matched more than once")]
    AnchorAmbiguous,
    #[error("{0}")]
    Rejected(String),
}

/// What a tool reports back to the loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
    /// What the environment did that the human has to read even after the
    /// answer lands -- a write the sandbox refused. The model's copy stays in
    /// `content`; this one is for the report.
    pub notice: Option<String>,
}

impl ToolOutput {
    pub fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false, notice: None }
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true, notice: None }
    }
}

/// A tool the agent can call. The arguments are JSON so the wire shape is the
/// same for every tool; the loop does not learn a new shape per tool.
pub trait Tool {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn call(
        &mut self,
        args: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<ToolOutput, ToolError>> + Send;
}
