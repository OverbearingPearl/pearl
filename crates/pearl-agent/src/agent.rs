//! agent --- the loop that drains the queue through a provider and tools.
//!
//! One agent, not many. The loop dispatches the next queued node, asks the
//! model, and either finishes the node or executes the tool calls the model
//! requests and asks again. Tool calls run under the node; the session records
//! every result, so a file edit is revertible from the session. The model is
//! chosen per request; the queue is the subject the loop serves.

use crate::spawn::{Spawned, SubagentPool};
use crate::summary::ModelSummary;
use crate::tools::ToolRegistry;
use pearl_core::{Queue, TaskId};
use pearl_provider::{ChatMessage, CompletionRequest, Provider, ProviderError, Role};
use pearl_session::{
    Budget, Entry, Meter, Role as SessionRole, SessionStore, StoreError,
};
use pearl_tools::sandbox::Policy;
use serde_json::Value;
use tokio::sync::watch;

/// A bound on tool rounds, so a model that never stops calling tools cannot
/// spin forever. The loop finishes the node in error when it trips.
const MAX_ROUNDS: usize = 64;

/// How long `serve` sleeps before looking at the queue again. Short enough
/// that a task placed by the surface starts before a human notices the wait.
const IDLE_POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// When history is folded, and how much of it survives.
///
/// The trigger is a fraction of the model's context window rather than a
/// message count, because the window is what runs out. The default asks for a
/// fold at three quarters: early enough that the request after it still has
/// room for tool results, late enough that folding does not spend a request on
/// a conversation that would have fit anyway.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactionPolicy {
    /// The context window of the model this loop talks to, in tokens.
    pub window: u32,
    /// The fraction of the window at which history is folded.
    pub trigger: f64,
    /// What a fold keeps and what it must reclaim.
    pub budget: Budget,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self { window: 128_000, trigger: 0.75, budget: Budget::default() }
    }
}

impl CompactionPolicy {
    /// A policy for one model, at the default fractions.
    pub fn for_window(window: u32) -> Self {
        Self { window, ..Self::default() }
    }

    /// The token count at which history is folded.
    pub fn threshold(&self) -> u32 {
        (f64::from(self.window) * self.trigger).round() as u32
    }
}

/// An error from the provider, the session, or the queue.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("provider: {0}")]
    Provider(#[from] ProviderError),
    #[error("session: {0}")]
    Session(#[from] StoreError),
    #[error("queue: {0}")]
    Queue(String),
    #[error("workspace: {0}")]
    Workspace(String),
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
    compaction: CompactionPolicy,
    meter: Meter,
}

impl<P: Provider + Sync> Agent<P> {
    pub fn new(queue: Queue, provider: P, store: SessionStore, model: impl Into<String>) -> Self {
        Self {
            queue,
            provider,
            store,
            model: model.into(),
            tools: None,
            compaction: CompactionPolicy::default(),
            meter: Meter::new(),
        }
    }

    /// Fold history at a different window or different fractions.
    pub fn with_compaction(mut self, compaction: CompactionPolicy) -> Self {
        self.compaction = compaction;
        self
    }

    /// The meter that decides how full the context is.
    pub fn meter(&self) -> Meter {
        self.meter
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

    /// The conversation as it would go on the wire: everything the log holds
    /// except what a summary already stands in for.
    fn messages(&self) -> Result<Vec<ChatMessage>, StoreError> {
        Ok(messages_from(&pearl_session::view(&self.store.read()?)))
    }

    /// Fold history when the context is full enough to need it. Returns
    /// whether anything was folded.
    ///
    /// The summary is written back as a new entry that covers the range it
    /// replaced, so a resumed session shows the same view without paying for
    /// the same summary twice. The folded entries stay in the log.
    async fn compact(&mut self) -> Result<bool, AgentError> {
        let shown = pearl_session::view(&self.store.read()?);
        if !self.over_threshold(&shown) {
            return Ok(false);
        }
        let summarizer = ModelSummary::new(&self.provider, &self.model);
        let projection = pearl_session::project(
            &shown,
            &self.compaction.budget,
            &summarizer,
            &self.meter,
        )
        .await;
        let Some(covers) = projection.covered else {
            return Ok(false);
        };
        self.store.append_compaction(projection.summary, covers)?;
        Ok(true)
    }

    /// Whether the view is full enough to fold.
    fn over_threshold(&self, entries: &[Entry]) -> bool {
        let tokens = self.meter.total(entries.iter().map(|e| e.content.as_str()));
        tokens >= self.compaction.threshold()
    }

    /// Teach the meter what the request actually cost, so the next estimate is
    /// in the provider's scale rather than in a character count.
    fn learn(&mut self, messages: &[ChatMessage], billed: u32) {
        let estimated = self.meter.total(messages.iter().map(|m| m.content.as_str()));
        self.meter.observe(estimated, billed);
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

        // Fold before the node's first request, at a turn boundary: a tool
        // call and its result must never be separated by a cut.
        self.compact().await?;

        // The wire conversation lives in memory for the tool rounds; the
        // session keeps the durable text record of everything that happened.
        let mut messages = self.messages()?;
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
            // A provider that refuses ends this task, not the session: the
            // surface still has a queue and the human still has other work.
            let completion = match self.provider.complete(request).await {
                Ok(completion) => completion,
                Err(e) => {
                    self.queue.fail(id, format!("provider: {e}"))?;
                    return Ok(Some(id));
                }
            };
            self.learn(&messages, completion.usage.prompt_tokens);

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
        let mut messages = self.messages()?;
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
        while self.step().await?.is_some() {
            count += 1;
        }
        Ok(count)
    }

    /// Serve the queue until `stop` trips: run what has been placed, then wait
    /// for more instead of returning. This is the surface's engine -- the
    /// process keeps one queue in front of the human and behind it a runner
    /// that never exits because the queue happened to be empty.
    pub async fn serve(&mut self, mut stop: watch::Receiver<bool>) -> Result<usize, AgentError> {
        let mut count = 0;
        while !*stop.borrow() {
            if self.queue.next_queued().is_none() {
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = tokio::time::sleep(IDLE_POLL) => {}
                }
                continue;
            }
            if self.step().await?.is_some() {
                count += 1;
            }
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
        let workspace = self.tools.as_ref().map(|t| t.root().to_path_buf());
        Ok(pool.spawn(&mut self.queue, parent, titles, workspace.as_deref())?)
    }

    /// Run the children of a fan-out at the same time, each in its own
    /// workspace copy, and settle each one as its result arrives. The queue
    /// marks every child running before the fan-in, so the surface shows them
    /// all in flight while they are, and the human's focus never leaves the
    /// parent.
    pub async fn run_children(&mut self, spawned: &[Spawned]) -> Result<usize, AgentError> {
        let policy = self.tools.as_ref().map(|t| t.policy()).unwrap_or(Policy::ReadOnly);
        let mut titles = Vec::with_capacity(spawned.len());
        for s in spawned {
            titles.push(self.queue.get(s.child).map(|n| n.title.clone()).unwrap_or_default());
            self.queue.start(s.child)?;
        }

        let provider = &self.provider;
        let model = &self.model;
        let runs = spawned.iter().zip(titles).map(|(s, title)| async move {
            let settled = match SessionStore::open(&s.session) {
                Ok(store) => {
                    let tools = if s.workspace.as_os_str().is_empty() {
                        None
                    } else {
                        Some(ToolRegistry::new(&s.workspace, policy))
                    };
                    run_child(provider, model, title, store, tools).await
                }
                Err(e) => Err(e.to_string()),
            };
            (s.child, settled)
        });
        let outcomes = futures::future::join_all(runs).await;

        let mut settled = 0;
        for (child, result) in outcomes {
            match result {
                Ok(text) => {
                    self.queue.complete(child, text)?;
                }
                Err(why) => {
                    self.queue.fail(child, why)?;
                }
            }
            settled += 1;
        }
        Ok(settled)
    }

    /// Run one subagent to completion in its derived session, then settle it.
    /// The session is separate from the parent's, so the subagent's context is
    /// its own; only the settled result flows back to the queue.
    pub async fn run_subagent(&mut self, child: TaskId, mut store: SessionStore) -> Result<(), AgentError> {
        let title = self.queue.get(child).map(|n| n.title.clone()).unwrap_or_default();
        self.queue.start(child)?;
        store.append(SessionRole::User, title)?;
        let mut messages = messages_from(&pearl_session::view(&store.read()?));
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

/// Run one child to completion in its own session and workspace copy, and
/// return its final text. A child that fails returns its reason, which settles
/// the node as failed rather than touching its siblings' trees.
async fn run_child<P: Provider>(
    provider: &P,
    model: &str,
    title: String,
    mut store: SessionStore,
    mut tools: Option<ToolRegistry>,
) -> Result<String, String> {
    store.append(SessionRole::User, title).map_err(|e| e.to_string())?;
    let entries = store.read().map_err(|e| e.to_string())?;
    let mut messages = messages_from(&pearl_session::view(&entries));
    let specs = tools.as_ref().map(|t| t.specs()).unwrap_or_default();
    let mut rounds = 0;
    loop {
        rounds += 1;
        if rounds > MAX_ROUNDS {
            return Err(String::from("too many tool rounds"));
        }
        let request = CompletionRequest {
            model: model.to_string(),
            messages: messages.clone(),
            stream: false,
            tools: specs.clone(),
        };
        let completion = provider.complete(request).await.map_err(|e| e.to_string())?;
        if completion.wants_tools() {
            if let Some(hands) = &mut tools {
                messages.push(ChatMessage::assistant_calls(completion.tool_calls.clone()));
                store
                    .append(
                        SessionRole::Assistant,
                        format!("[calling {} tool(s)]", completion.tool_calls.len()),
                    )
                    .map_err(|e| e.to_string())?;
                for call in &completion.tool_calls {
                    let args: Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                    let output = hands.call(&call.function.name, args).await;
                    store
                        .append(SessionRole::Tool, output.content.clone())
                        .map_err(|e| e.to_string())?;
                    messages.push(ChatMessage::tool_result(call.id.clone(), output.content));
                }
                continue;
            }
        }
        store
            .append(SessionRole::Assistant, completion.content.clone())
            .map_err(|e| e.to_string())?;
        return Ok(completion.content);
    }
}

/// Map log entries onto the wire conversation.
fn messages_from(entries: &[Entry]) -> Vec<ChatMessage> {
    entries
        .iter()
        .map(|e| ChatMessage::new(provider_role(e.role), e.content.clone()))
        .collect()
}

fn provider_role(role: SessionRole) -> Role {
    match role {
        SessionRole::System => Role::System,
        SessionRole::User => Role::User,
        SessionRole::Assistant => Role::Assistant,
        SessionRole::Tool => Role::Tool,
        // A summary is context the model reads, not a turn it had.
        SessionRole::Compaction => Role::System,
    }
}
