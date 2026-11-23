//! agent --- the loop that drains the queue through a provider and tools.
//!
//! One agent, not many. The loop dispatches the next queued node, asks the
//! model, and either finishes the node or executes the tool calls the model
//! requests and asks again. Tool calls run under the node; the session records
//! every result, so a file edit is revertible from the session. The model is
//! chosen per request; the queue is the subject the loop serves.

use crate::spawn::{Spawned, SubagentPool};
use crate::tools::ToolRegistry;
use pearl_core::{Queue, TaskId};
use pearl_provider::{ChatMessage, CompletionRequest, Provider, ProviderError, Role};
use pearl_session::{Role as SessionRole, SessionStore, StoreError};
use pearl_tools::sandbox::Policy;
use serde_json::Value;

/// A bound on tool rounds, so a model that never stops calling tools cannot
/// spin forever. The loop finishes the node in error when it trips.
const MAX_ROUNDS: usize = 64;

/// An error from the provider, the session, or the queue.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),
    #[error("session: {0}")]
    Session(#[from] StoreError),
    #[error("queue: {0}")]
    Queue(String),
}

impl From<pearl_core::QueueError> for AgentError {
    fn from(e: pearl_core::QueueError) -> Self {
        Self::Queue(e.to_string())
    }
}

/// The loop: a queue, a provider, a session, and optionally a set of tools.
pub struct Agent<P: Provider> {
    pub queue: Queue,
    provider: P,
    store: SessionStore,
    model: String,
    tools: Option<ToolRegistry>,
}

impl<P: Provider> Agent<P> {
    pub fn new(queue: Queue, provider: P, store: SessionStore, model: impl Into<String>) -> Self {
        Self { queue, provider, store, model: model.into(), tools: None }
    }

    /// Give the agent hands: a workspace and a sandbox policy for its tools.
    pub fn with_tools(mut self, root: impl AsRef<std::path::Path>, policy: Policy) -> Self {
        self.tools = Some(ToolRegistry::new(root, policy));
        self
    }

    /// The session the loop records into.
    pub fn session(&self) -> &SessionStore {
        &self.store
    }

    /// The provider the loop asks, for callers that inspect or settle it.
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Run one task to completion: dispatch, ask the model, execute any tool
    /// calls and ask again, then finish. Returns the task id, or `None` when
    /// the queue has nothing dispatchable.
    pub async fn step(&mut self) -> Result<Option<TaskId>, AgentError> {
        let Some(id) = self.queue.next_queued() else {
            return Ok(None);
        };
        let title = self.queue.get(id).map(|n| n.title.clone()).unwrap_or_default();
        let steering: Vec<String> = self
            .queue
            .get(id)
            .map(|n| n.steering.iter().map(|s| s.text.clone()).collect())
            .unwrap_or_default();

        self.queue.start(id)?;
        for s in steering {
            self.store.append(SessionRole::User, s)?;
        }
        self.queue.drain_steering(id)?;
        self.store.append(SessionRole::User, title)?;

        // The wire conversation lives in memory for the tool rounds; the
        // session keeps the durable text record of everything that happened.
        let mut messages: Vec<ChatMessage> = self
            .store
            .read()?
            .iter()
            .map(|e| ChatMessage::new(provider_role(e.role), e.content.clone()))
            .collect();
        let specs = self.tools.as_ref().map(|t| t.specs()).unwrap_or_default();

        let mut rounds = 0;
        loop {
            rounds += 1;
            if rounds > MAX_ROUNDS {
                self.queue.fail(id, "too many tool rounds")?;
                return Ok(Some(id));
            }
            let request = CompletionRequest {
                model: self.model.clone(),
                messages: messages.clone(),
                stream: false,
                tools: specs.clone(),
            };
            let completion = self.provider.complete(request).await?;

            if completion.wants_tools() {
                if let Some(tools) = &mut self.tools {
                    messages.push(ChatMessage::assistant_calls(completion.tool_calls.clone()));
                    self.store.append(
                        SessionRole::Assistant,
                        format!("[calling {} tool(s)]", completion.tool_calls.len()),
                    )?;
                    for call in &completion.tool_calls {
                        let args: Value =
                            serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                        let output = tools.call(&call.function.name, args).await;
                        self.store.append(SessionRole::Tool, output.content.clone())?;
                        messages.push(ChatMessage::tool_result(call.id.clone(), output.content));
                    }
                    continue;
                }
            }

            self.store.append(SessionRole::Assistant, completion.content.clone())?;
            self.queue.complete(id, completion.content)?;
            return Ok(Some(id));
        }
    }

    /// A side question: ask the model against the session as read-only
    /// context and return the answer. Nothing enters the queue or the log;
    /// the aside lives on the surface, not in the record.
    pub async fn btw(&mut self, question: &str) -> Result<String, AgentError> {
        let mut messages: Vec<ChatMessage> = self
            .store
            .read()?
            .iter()
            .map(|e| ChatMessage::new(provider_role(e.role), e.content.clone()))
            .collect();
        messages.push(ChatMessage::new(Role::User, question.to_string()));
        let request = CompletionRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            tools: vec![],
        };
        let completion = self.provider.complete(request).await?;
        Ok(completion.content)
    }

    /// Drain the queue, running every dispatchable task in order.
    pub async fn run(&mut self) -> Result<usize, AgentError> {
        let mut count = 0;
        while let Some(_id) = self.step().await? {
            count += 1;
        }
        Ok(count)
    }

    /// Fan a running node out into child tasks, each with its own derived
    /// session. The children are the parallel execution; the human keeps one
    /// focus on the parent. The parent finishes only when every child settles.
    pub fn spawn(
        &mut self,
        parent: TaskId,
        titles: &[impl AsRef<str>],
    ) -> Result<Vec<Spawned>, AgentError> {
        let pool = SubagentPool::new(self.store.dir());
        Ok(pool.spawn(&mut self.queue, &self.store, parent, titles)?)
    }

    /// Run one subagent to completion in its derived session, then settle it.
    /// The session is separate from the parent's, so the subagent's context is
    /// its own; only the settled result flows back to the queue.
    pub async fn run_subagent(&mut self, child: TaskId, mut store: SessionStore) -> Result<(), AgentError> {
        let title = self.queue.get(child).map(|n| n.title.clone()).unwrap_or_default();
        self.queue.start(child)?;
        store.append(SessionRole::User, title)?;
        let mut messages: Vec<ChatMessage> = store
            .read()?
            .iter()
            .map(|e| ChatMessage::new(provider_role(e.role), e.content.clone()))
            .collect();
        let specs = self.tools.as_ref().map(|t| t.specs()).unwrap_or_default();
        let mut rounds = 0;
        loop {
            rounds += 1;
            if rounds > MAX_ROUNDS {
                self.queue.fail(child, "too many tool rounds")?;
                return Ok(());
            }
            let request = CompletionRequest {
                model: self.model.clone(),
                messages: messages.clone(),
                stream: false,
                tools: specs.clone(),
            };
            let completion = self.provider.complete(request).await?;
            if completion.wants_tools() {
                if let Some(tools) = &mut self.tools {
                    messages.push(ChatMessage::assistant_calls(completion.tool_calls.clone()));
                    store.append(
                        SessionRole::Assistant,
                        format!("[calling {} tool(s)]", completion.tool_calls.len()),
                    )?;
                    for call in &completion.tool_calls {
                        let args: Value =
                            serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                        let output = tools.call(&call.function.name, args).await;
                        store.append(SessionRole::Tool, output.content.clone())?;
                        messages.push(ChatMessage::tool_result(call.id.clone(), output.content));
                    }
                    continue;
                }
            }
            store.append(SessionRole::Assistant, completion.content.clone())?;
            self.queue.complete(child, completion.content)?;
            return Ok(());
        }
    }
}

fn provider_role(role: SessionRole) -> Role {
    match role {
        SessionRole::System => Role::System,
        SessionRole::User => Role::User,
        SessionRole::Assistant => Role::Assistant,
        SessionRole::Tool => Role::Tool,
    }
}
