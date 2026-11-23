//! agent --- the loop that moves the queue, one task at a time.
//!
//! The queue decides what runs and when; the agent carries each node across
//! the wire and writes the answer down. Steering is drained at the turn
//! boundary, never mid-token, and every exchange is persisted before the next.

use pearl_core::{Event, Queue};
use pearl_provider::{ChatMessage, CompletionRequest, Provider, ProviderError, Role};
use pearl_session::{Role as EntryRole, SessionStore, StoreError};
use pearl_tools::Workspace;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),
    #[error("session: {0}")]
    Session(#[from] StoreError),
    #[error("queue: {0}")]
    Queue(String),
}

/// The loop: a queue, a provider, and the session that remembers the exchange.
pub struct Agent<P: Provider> {
    pub queue: Queue,
    pub provider: P,
    pub store: SessionStore,
    pub model: String,
    /// The revertible workspace the agent's edits land in, when it has one.
    pub workspace: Option<Workspace>,
}

impl<P: Provider> Agent<P> {
    pub fn new(queue: Queue, provider: P, store: SessionStore, model: impl Into<String>) -> Self {
        Self { queue, provider, store, model: model.into(), workspace: None }
    }

    /// Give the agent a workspace rooted at `root`; its edits are revertible.
    pub fn with_workspace(mut self, root: impl AsRef<std::path::Path>) -> Self {
        self.workspace = Some(Workspace::new(root));
        self
    }

    /// Carry one queued node across the wire. Returns the events the queue
    /// emitted, or `None` when nothing is ready to run.
    pub async fn step(&mut self) -> Result<Option<Vec<Event>>, AgentError> {
        let Some(id) = self.queue.next_queued() else { return Ok(None) };
        let node = self.queue.get(id);
        let title = node.map(|n| n.title.clone()).unwrap_or_default();
        let steering: Vec<String> =
            node.map(|n| n.steering.iter().map(|s| s.text.clone()).collect()).unwrap_or_default();

        // Start the turn, then drain steering into the session at the boundary.
        self.queue.start(id).map_err(|e| AgentError::Queue(e.to_string()))?;
        for text in steering {
            self.store.append(EntryRole::User, text)?;
        }
        self.queue.drain_steering(id).map_err(|e| AgentError::Queue(e.to_string()))?;
        self.store.append(EntryRole::User, title)?;

        let messages = self
            .store
            .read()?
            .iter()
            .map(|e| ChatMessage::new(map_role(e.role), e.content.clone()))
            .collect();
        let request = CompletionRequest { model: self.model.clone(), messages, stream: false };
        let completion = self.provider.complete(request).await?;
        self.store.append(EntryRole::Assistant, completion.content.clone())?;

        let events =
            self.queue.complete(id, completion.content).map_err(|e| AgentError::Queue(e.to_string()))?;
        Ok(Some(events))
    }

    /// Run until no node is ready. Returns the total events emitted.
    pub async fn run(&mut self) -> Result<Vec<Event>, AgentError> {
        let mut all = Vec::new();
        while let Some(events) = self.step().await? {
            all.extend(events);
        }
        Ok(all)
    }
}

fn map_role(role: EntryRole) -> Role {
    match role {
        EntryRole::System => Role::System,
        EntryRole::User => Role::User,
        EntryRole::Assistant => Role::Assistant,
        EntryRole::Tool => Role::Tool,
    }
}
