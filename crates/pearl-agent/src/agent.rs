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
use pearl_core::{Gauge, NodeState, Queue, Settings, TaskId};
use pearl_provider::wire::{Width, refusal_width};
use pearl_provider::{ChatMessage, Completion, CompletionRequest, Provider, ProviderError, Role, ToolCall};
use pearl_session::{
    Budget, Entry, Meter, Role as SessionRole, SessionStore, StoreError,
};
use pearl_tools::sandbox::Policy;
use serde_json::Value;
use tokio::sync::watch;

/// A bound on tool rounds, so a model that never stops calling tools cannot
/// spin forever. The loop finishes the node in error when it trips.
/// The tool the loop performs itself: fan-out needs the queue and the pool.
const FANOUT: &str = "subagent";
/// The tool that lets a task stop and ask the human a question. The loop runs
/// it itself: the answer is the human's next message, not a workspace call.
const ASK: &str = "ask";

fn wants_ask(c: &pearl_provider::Completion) -> bool {
    c.tool_calls.iter().any(|t| t.function.name == ASK)
}

fn wants_fanout(c: &pearl_provider::Completion) -> bool {
    c.tool_calls.iter().any(|t| t.function.name == FANOUT)
}

const MAX_ROUNDS: usize = 64;

/// How long `serve` sleeps before looking at the queue again. Short enough
/// that a task placed by the surface starts before a human notices the wait.
const IDLE_POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// When history is folded, and how much of it survives.
///
/// The window and the fraction live in `Settings`, where the human can turn
/// them while the work runs; this holds only what a fold keeps and reclaims,
/// which is a property of the fold rather than a preference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactionPolicy {
    /// What a fold keeps and what it must reclaim.
    pub budget: Budget,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self { budget: Budget::default() }
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
    settings: Settings,
    gauge: Gauge,
    tools: Option<ToolRegistry>,
    compaction: CompactionPolicy,
    meter: Meter,
    /// The heading whose conversation is being written: the task in flight
    /// belongs to it, and every turn it asks for is stamped with it.
    topic: Option<String>,
}

impl<P: Provider + Sync> Agent<P> {
    pub fn new(queue: Queue, provider: P, store: SessionStore, model: impl Into<String>) -> Self {
        Self {
            queue,
            provider,
            store,
            settings: Settings::new(model),
            gauge: Gauge::new(),
            tools: None,
            compaction: CompactionPolicy::default(),
            topic: None,
            meter: Meter::new(),
        }
    }

    /// Fold history at a different window or different fractions.
    /// Share the live configuration with the surface, so a knob turned there
    /// is in force for the next request rather than the next process.
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Take the most recent shell op back, restoring the workspace. `None`
    /// when there is nothing to undo. This is the headline promise made
    /// reachable: the op id was always reported, and nothing could act on it.
    pub fn revert_last(&mut self) -> Result<Option<String>, AgentError> {
        match &mut self.tools {
            Some(tools) => tools
                .revert_last()
                .map_err(|e| AgentError::Workspace(e.to_string())),
            None => Ok(None),
        }
    }

    /// The live configuration this runner reads.
    pub fn settings(&self) -> Settings {
        self.settings.clone()
    }

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

    /// Keep the thinking beside the answer: in the log for the record, on the
    /// node for the screen. Neither is sent back to the endpoint.
    fn keep_thinking(&mut self, id: TaskId, done: &pearl_provider::Completion) -> Result<(), AgentError> {
        if let Some(thought) = &done.reasoning {
            self.store.append(SessionRole::Reasoning, thought.clone())?;
            let _ = self.queue.thought(id, thought.clone());
        }
        Ok(())
    }

    /// What this node's settled children found, one line each. A child that
    /// failed reports its failure: the parent decides what to do about a part
    /// that could not be done, rather than being stranded by it.
    fn findings(&self, parent: TaskId) -> Vec<String> {
        let Some(node) = self.queue.get(parent) else {
            return Vec::new();
        };
        node.children
            .iter()
            .filter_map(|c| self.queue.get(*c))
            .filter_map(|c| {
                let said = c.result.or(c.error)?;
                Some(format!("{}: {said}", c.title))
            })
            .collect()
    }

    /// The tools this loop offers: the workspace's, plus fan-out, which the
    /// loop performs itself because it needs the queue and the pool.
    pub fn all_specs(&self) -> Vec<pearl_provider::ToolSpec> {
        let mut specs = self.tools.as_ref().map(|t| t.specs()).unwrap_or_default();
        specs.push(pearl_provider::ToolSpec::new(
            ASK,
            "Stop and ask the human one question when the answer decides what to do next and guessing would waste the work. The task waits, visibly, until they answer; the answer arrives as the next message.",
            serde_json::json!({"type":"object","properties":{"question":{"type":"string"}},"required":["question"]}),
        ));
        specs.push(pearl_provider::ToolSpec::new(
            FANOUT,
            "Split this task into child tasks that run at the same time, each in its own copy of the workspace and its own session. Returns what each child found.",
            serde_json::json!({"type":"object","properties":{"titles":{"type":"array","items":{"type":"string"}}},"required":["titles"]}),
        ));
        specs
    }

    /// The names on offer, for a surface or a story that asks what is possible.
    pub fn tool_names(&self) -> Vec<String> {
        self.all_specs().iter().map(|s| s.function.name.clone()).collect()
    }

    /// Fan this node out and wait for the children, then report what each one
    /// found. A refusal is a tool result, not a dead task: the parent reads it
    /// and decides what to do next.
    async fn fanout(&mut self, parent: TaskId, arguments: &str) -> String {
        let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
        let titles: Vec<String> = args
            .get("titles")
            .and_then(|t| t.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if titles.is_empty() {
            return String::from("subagent: no titles given");
        }
        let spawned = match self.spawn(parent, &titles) {
            Ok(s) => s,
            Err(e) => return format!("subagent: {e}"),
        };
        if let Err(e) = self.run_children(&spawned).await {
            return format!("subagent: {e}");
        }
        spawned
            .iter()
            .map(|s| {
                let node = self.queue.get(s.child);
                let title = node.as_ref().map(|n| n.title.clone()).unwrap_or_default();
                let said = node
                    .and_then(|n| n.result.or(n.error))
                    .unwrap_or_else(|| String::from("(nothing said)"));
                format!("- {title}: {said}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Share the window's reading with the surface, so the fill and the folds
    /// are on the screen while the work runs.
    pub fn with_gauge(mut self, gauge: Gauge) -> Self {
        self.gauge = gauge;
        self
    }

    /// Share the ledger of takeable writes with the surface.
    pub fn with_ops(mut self, ops: pearl_tools::Ops) -> Self {
        self.tools = self.tools.take().map(|t| t.with_ops(ops));
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
        let entries = self.store.read()?;
        Ok(replay(&pearl_session::view_in(&entries, self.topic.as_deref())))
    }

    /// Fold history when the context is full enough to need it. Returns
    /// whether anything was folded.
    ///
    /// The summary is written back as a new entry that covers the range it
    /// replaced, so a resumed session shows the same view without paying for
    /// the same summary twice. The folded entries stay in the log.
    async fn compact(&mut self, id: TaskId) -> Result<bool, AgentError> {
        let entries = self.store.read()?;
        let shown = pearl_session::view_in(&entries, self.topic.as_deref());
        if !self.over_threshold(&shown) {
            return Ok(false);
        }
        let before = self.meter.total(shown.iter().map(|e| e.content.as_str()));
        let model = self.settings.model();
        let summarizer = ModelSummary::new(&self.provider, &model);
        let projection = pearl_session::project(
            &shown,
            &self.budget(),
            &summarizer,
            &self.meter,
        )
        .await;
        let Some(covers) = projection.covered else {
            return Ok(false);
        };
        self.store.append_compaction(projection.summary, covers)?;
        self.gauge.folded();
        let after = self.meter.total(
            pearl_session::view_in(&self.store.read()?, self.topic.as_deref())
                .iter()
                .map(|e| e.content.as_str()),
        );
        // The gauge says a fold happened; the task says what it cost, so the
        // human reading this task's report knows why the agent stopped quoting
        // its first turns. Once per fold, with the size before and after: a
        // count alone says how often, never whether it was worth doing, and a
        // running total hides the shape of a long session behind one number.
        let entries = covers.last.saturating_sub(covers.first) + 1;
        let _ = self.queue.notice(
            id,
            format!(
                "history: summarized {entries} older entries, {} → {}, to fit the window",
                Self::thousands_impl(before),
                Self::thousands_impl(after),
            ),
        );
        Ok(true)
    }

    /// A token count as the header writes it: `12.4k`, or `840` under a
    /// thousand, because a size change is read at a glance and `12413` is
    /// arithmetic at a glance.
    fn thousands_impl(tokens: u32) -> String {
        if tokens < 1000 {
            tokens.to_string()
        } else {
            format!("{:.1}k", f64::from(tokens) / 1000.0)
        }
    }

    /// The fold's budget, scaled to the window in force. A tail of eight
    /// thousand tokens is right for a 128k window and impossible for a 2k one:
    /// without this, lowering the window could never reclaim anything.
    fn budget(&self) -> Budget {
        let fold_at = self.settings.fold_at();
        let mut budget = self.compaction.budget;
        budget.tail_tokens = budget.tail_tokens.min(fold_at / 2);
        budget.min_fold_tokens = budget.min_fold_tokens.min(fold_at / 4);
        budget
    }

    /// Whether the view is full enough to fold.
    fn over_threshold(&self, entries: &[Entry]) -> bool {
        let tokens = self.meter.total(entries.iter().map(|e| e.content.as_str()));
        tokens >= self.settings.fold_at()
    }

    /// Teach the meter what the request actually cost, so the next estimate is
    /// in the provider's scale rather than in a character count.
    fn learn(
        &mut self,
        id: TaskId,
        messages: &[ChatMessage],
        usage: pearl_provider::Usage,
        ms: u64,
        note: String,
    ) {
        let billed = usage.prompt_tokens;
        self.gauge.spent_with_cache(billed, usage.cached_tokens);
        let cost = pearl_core::Cost::turn(billed, usage.completion_tokens, ms);
        self.queue.charge(id, pearl_core::Turn::doing(cost, note));
        self.gauge.charge(cost);
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

        // This task's work belongs to its heading, if it has one: the turns go
        // into the log stamped with it, and the conversation this task reads
        // is that heading's -- its own, plus the shared line.
        let heading = self.queue.heading_of(id);
        self.topic = heading.map(|h| h.to_string());
        self.store.in_topic(self.topic.clone());
        // A topic scopes the conversation, not the files: say what other lines
        // of work wrote, or this task reads a file as it no longer is.
        if let (Some(heading), Some(tools)) = (heading, self.tools.as_ref()) {
            let own = self.queue.tasks_under(heading);
            if let Some(said) = tools.elsewhere(&own) {
                self.store.append(SessionRole::User, said)?;
            }
        }
        self.queue.start(id)?;
        for s in steering {
            self.store.append(SessionRole::User, s)?;
        }
        self.queue.drain_steering(id)?;
        self.store.append(SessionRole::User, title)?;
        // A nested task is a decomposition: its parts ran first, and the whole
        // is only worth asking for once it can read what they found.
        for finding in self.findings(id) {
            self.store.append(SessionRole::User, finding)?;
        }

        // The wire conversation lives in memory for the tool rounds; the
        // session keeps the durable text record of everything that happened.
        let mut messages = self.messages()?;
        let specs = self.all_specs();

        let mut rounds = 0;
        // One correction per task. An endpoint that refuses the retry too is
        // refusing something else, and looping would spend the human's money
        // finding that out.
        let mut taught = false;
        loop {
            rounds += 1;
            // Cancel is a promise the runner keeps, not one the screen makes.
            // Checked at the top of every round and again before the tools:
            // a task the human stopped stops calling the model and stops
            // spending, and what it did up to now is recorded as unfinished.
            if self.halted(id) {
                self.store.append(
                    SessionRole::User,
                    "[the human cancelled this task: everything above it is work in progress, not an answer]",
                )?;
                return Ok(Some(id));
            }
            if rounds > MAX_ROUNDS {
                self.queue.fail(id, "too many tool rounds")?;
                return Ok(Some(id));
            }
            // Fold before every request, not only the first: one task with
            // twelve tool rounds can fill the window by itself, and a fold
            // that only happens at a task's start never helps the task that
            // fills it. The wire conversation is rebuilt from the view
            // because the one in memory holds calls the fold may have taken.
            if self.compact(id).await? {
                messages = self.messages()?;
            }
            // The wait for the model is the longest silence the human sits
            // through, so the task says what it is waiting for. A note is
            // presentation: a cancelled node refuses it and the work goes on.
            let _ = self.queue.note(id, "asking the model");
            let request = CompletionRequest {
                model: self.settings.model(),
                reasoning_effort: pearl_provider::wire::effort_word(
                    self.settings.dialect(),
                    self.settings.effort(),
                ),
                extra: pearl_provider::wire::thinking_fields(
                    self.settings.dialect(),
                    self.settings.effort(),
                    self.settings.window(),
                ),
                messages: messages.clone(),
                stream: false,
                tools: specs.clone(),
            };
            // A provider that refuses ends this task, not the session: the
            // surface still has a queue and the human still has other work.
            let began = std::time::Instant::now();
            let completion = match self.provider.complete(request).await {
                Ok(completion) => completion,
                // A refusal about the thinking field is the endpoint telling
                // us its dialect. Believing it once is cheaper than making
                // the human read a 400 and guess which knob to turn; asking
                // again without the field is the whole correction.
                Err(ProviderError::Refused { status, ref message })
                    if !taught && names_thinking(message) =>
                {
                    taught = true;
                    // What the endpoint rejected decides how much of the
                    // ladder goes with it. A value it would not take costs
                    // that rung; a field it does not know costs every rung,
                    // and filing only the one in force would leave four more
                    // 400s waiting for the human to walk into.
                    let said = match self.settings.effort() {
                        Some(rung) if refusal_width(message) == Width::Rung => {
                            let said = self.settings.refuse(rung);
                            let _ = self.settings.apply("effort auto");
                            said
                        }
                        Some(_) | None => {
                            let said = self.settings.refuse_the_field();
                            let _ = self.settings.apply("effort auto");
                            said
                        }
                    };
                    let _ = self.queue.note(
                        id,
                        format!("refused ({status}): {said}, retrying"),
                    );
                    continue;
                }
                Err(e) => {
                    self.queue.fail(id, format!("provider: {e}"))?;
                    return Ok(Some(id));
                }
            };
            let note = work(&completion);
            self.learn(id, &messages, completion.usage, began.elapsed().as_millis() as u64, note);

            if self.halted(id) {
                self.store.append(
                    SessionRole::User,
                    "[the human cancelled this task while the model was answering: the answer above was not kept]",
                )?;
                return Ok(Some(id));
            }
            if completion.wants_tools()
                && (self.tools.is_some() || wants_fanout(&completion) || wants_ask(&completion))
            {
                messages.push(ChatMessage::assistant_calls(completion.tool_calls.clone()));
                self.store.append_asked(
                    serde_json::to_string(&completion.tool_calls).unwrap_or_default(),
                    format!("[calling {} tool(s)]", completion.tool_calls.len()),
                )?;
                // A question is the loop's own move too: it needs the queue
                // and the human, not the workspace.
                for call in completion.tool_calls.iter().filter(|c| c.function.name == ASK) {
                    let args: Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                    let question = args
                        .get("question")
                        .and_then(|q| q.as_str())
                        .unwrap_or("")
                        .to_string();
                    let said = self.wait_for_human(id, &question).await;
                    self.store.append_answered(call.id.clone(), said.clone())?;
                    messages.push(ChatMessage::tool_result(call.id.clone(), said));
                }
                // Ordinary tools run through the registry; fan-out is the
                // loop's own move, because it needs the queue and the pool.
                {
                    if let Some(tools) = &mut self.tools {
                        tools.for_task(id.to_string());
                        for call in &completion.tool_calls {
                            if call.function.name == FANOUT {
                                continue;
                            }
                            if call.function.name == ASK {
                                continue;
                            }
                            let args: Value =
                                serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                            let _ = self.queue.note(id, act(&call.function.name, &args));
                            let output = tools.call(&call.function.name, args).await;
                            if let Some(notice) = output.notice.clone() {
                                let _ = self.queue.notice(id, notice);
                            }
                            self.store.append_answered(call.id.clone(), output.content.clone())?;
                            messages.push(ChatMessage::tool_result(call.id.clone(), output.content));
                        }
                    }
                }
                for call in &completion.tool_calls {
                    if call.function.name != FANOUT {
                        continue;
                    }
                    let _ = self.queue.note(id, "fanning out");
                    let text = self.fanout(id, &call.function.arguments).await;
                    self.store.append_answered(call.id.clone(), text.clone())?;
                    messages.push(ChatMessage::tool_result(call.id.clone(), text));
                }
                continue;
            }

            self.keep_thinking(id, &completion)?;
            self.store.append(SessionRole::Assistant, completion.content.clone())?;
            self.queue.complete(id, completion.content)?;
            return Ok(Some(id));
        }
    }

    /// Put the task in front of the human and wait for their answer.
    ///
    /// The row says it is waiting and shows the question; the answer comes back
    /// as the human's next message, so the model reads it where a person would
    /// put it. A task taken away, finished or failed while the question stands
    /// ends the wait -- a question nobody can answer is not a reason to hang.
    async fn wait_for_human(&mut self, id: TaskId, question: &str) -> String {
        if let Err(e) = self.queue.ask(id, question) {
            return format!("ask: {e}");
        }
        let _ = self.queue.note(id, "waiting for you");
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if self.halted(id) {
                return String::from("ask: the human took the task away before answering");
            }
            match self.queue.get(id).map(|n| n.state) {
                Some(pearl_core::NodeState::Running) => {
                    return String::from(
                        "ask: the human has answered; their answer is the next message",
                    );
                }
                Some(pearl_core::NodeState::Queued)
                | Some(pearl_core::NodeState::Paused)
                | Some(pearl_core::NodeState::AwaitingInput) => {}
                _ => return String::from("ask: the task ended before the question was answered"),
            }
        }
    }

    /// Whether the human has taken this task away from the runner.
    fn halted(&self, id: TaskId) -> bool {
        self.queue
            .get(id)
            .map(|node| node.state != NodeState::Running)
            .unwrap_or(true)
    }

    /// A side question: ask the model against the session as read-only
    /// context and return the answer. Nothing enters the queue or the log;
    /// the aside lives on the surface, not in the record.
    pub async fn btw(&mut self, question: &str) -> Result<String, AgentError> {
        let mut messages = self.messages()?;
        messages.push(ChatMessage::new(Role::User, question.to_string()));
        let request = CompletionRequest {
            model: self.settings.model(),
                reasoning_effort: pearl_provider::wire::effort_word(
                    self.settings.dialect(),
                    self.settings.effort(),
                ),
                extra: pearl_provider::wire::thinking_fields(
                    self.settings.dialect(),
                    self.settings.effort(),
                    self.settings.window(),
                ),
            messages,
            stream: false,
            tools: vec![],
        };
        let began = std::time::Instant::now();
        let completion = self.provider.complete(request).await?;
        // A side question enters no task and no log, but it is still billed.
        self.gauge.charge(pearl_core::Cost::turn(
            completion.usage.prompt_tokens,
            completion.usage.completion_tokens,
            began.elapsed().as_millis() as u64,
        ));
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
        let settings = &self.settings;
        let queue = &self.queue;
        let gauge = &self.gauge;
        let runs = spawned.iter().zip(titles).map(|(s, title)| async move {
            let settled = match SessionStore::open(&s.session) {
                Ok(store) => {
                    let tools = if s.workspace.as_os_str().is_empty() {
                        None
                    } else {
                        Some(ToolRegistry::new(&s.workspace, policy))
                    };
                    run_child(provider, settings, title, store, tools, queue.clone(), s.child, gauge.clone()).await
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
        let mut messages = replay(&pearl_session::view(&store.read()?));
        let specs = self.all_specs();
        let mut rounds = 0;
        loop {
            rounds += 1;
            // A subagent is the parent's work: a child the human stopped stops
            // writing, and its settled text says so instead of an answer.
            if self.halted(child) {
                store.append(SessionRole::User, "[the human cancelled this task]")?;
                return Ok(());
            }
            if rounds > MAX_ROUNDS {
                self.queue.fail(child, "too many tool rounds")?;
                return Ok(());
            }
            let _ = self.queue.note(child, "asking the model");
            let request = CompletionRequest {
                model: self.settings.model(),
                reasoning_effort: pearl_provider::wire::effort_word(
                    self.settings.dialect(),
                    self.settings.effort(),
                ),
                extra: pearl_provider::wire::thinking_fields(
                    self.settings.dialect(),
                    self.settings.effort(),
                    self.settings.window(),
                ),
                messages: messages.clone(),
                stream: false,
                tools: specs.clone(),
            };
            let began = std::time::Instant::now();
            let completion = self.provider.complete(request).await?;
            let cost = pearl_core::Cost::turn(
                completion.usage.prompt_tokens,
                completion.usage.completion_tokens,
                began.elapsed().as_millis() as u64,
            );
            self.queue.charge(child, cost);
            self.gauge.charge(cost);
            self.gauge
                .spent_with_cache(completion.usage.prompt_tokens, completion.usage.cached_tokens);
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
                        let _ = self.queue.note(child, act(&call.function.name, &args));
                        let output = tools.call(&call.function.name, args).await;
                        store.append_answered(call.id.clone(), output.content.clone())?;
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
    settings: &Settings,
    title: String,
    mut store: SessionStore,
    mut tools: Option<ToolRegistry>,
    queue: Queue,
    id: TaskId,
    gauge: pearl_core::Gauge,
) -> Result<String, String> {
    store.append(SessionRole::User, title).map_err(|e| e.to_string())?;
    let entries = store.read().map_err(|e| e.to_string())?;
    let mut messages = replay(&pearl_session::view(&entries));
    let specs = tools.as_ref().map(|t| t.specs()).unwrap_or_default();
    let mut rounds = 0;
    loop {
        rounds += 1;
        if rounds > MAX_ROUNDS {
            return Err(String::from("too many tool rounds"));
        }
        let _ = queue.note(id, "asking the model");
        let request = CompletionRequest {
            model: settings.model(),
            reasoning_effort: pearl_provider::wire::effort_word(settings.dialect(), settings.effort()),
            extra: pearl_provider::wire::thinking_fields(
                settings.dialect(),
                settings.effort(),
                settings.window(),
            ),
            messages: messages.clone(),
            stream: false,
            tools: specs.clone(),
        };
        let began = std::time::Instant::now();
        let completion = provider.complete(request).await.map_err(|e| e.to_string())?;
        let cost = pearl_core::Cost::turn(
            completion.usage.prompt_tokens,
            completion.usage.completion_tokens,
            began.elapsed().as_millis() as u64,
        );
        queue.charge(id, cost);
        gauge.charge(cost);
        gauge.spent_with_cache(completion.usage.prompt_tokens, completion.usage.cached_tokens);
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
                    let _ = queue.note(id, act(&call.function.name, &args));
                    let output = hands.call(&call.function.name, args).await;
                    store
                        .append_answered(call.id.clone(), output.content.clone())
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
/// Name the act in the words the human would use: the tool and the thing it
/// touches. An argument that is not a path or a command is left out rather than
/// guessed at, so the line stays short enough to read at a glance.
/// What a turn was doing, in the width a bill line has. One call says what it
/// touched; a round of several says its shape, because six `read` lines are
/// not what the reader of a bill is asking for.
fn work(completion: &Completion) -> String {
    if !completion.wants_tools() {
        return String::from("answer");
    }
    // Every call says what it asked for, not only the name of the tool: a
    // turn that read two files and a turn that read one are different rounds
    // with the same word, and the report is the only place the difference was
    // ever going to show. Past two calls the tail is counted, because a note
    // that is a paragraph is a note the pane clips in the middle of a path.
    const NAMED: usize = 2;
    let mut said: Vec<String> = completion
        .tool_calls
        .iter()
        .take(NAMED)
        .map(|call| {
            let args: Value = serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
            act(&call.function.name, &args)
        })
        .collect();
    let rest = completion.tool_calls.len().saturating_sub(said.len());
    if rest > 0 {
        said.push(format!("+{rest}"));
    }
    said.join(", ")
}

fn act(tool: &str, args: &Value) -> String {
    const TELLING: [&str; 4] = ["path", "command", "pattern", "query"];
    for key in TELLING {
        if let Some(value) = args.get(key).and_then(Value::as_str) {
            let short: String = value.chars().take(60).collect();
            return format!("{tool} {short}");
        }
    }
    tool.to_string()
}

/// The log as the request it came from. A turn that asked for tools comes
/// back as a turn that asked for tools, and a tool turn comes back answering
/// the call it named -- a tool result with no id is not a turn the model can
/// answer, and an endpoint says so out loud rather than guessing.
///
/// A log written before calls were recorded has neither: its announcement is
/// plain text and its tool turns name nothing, so those turns are dropped.
/// Replaying them would be sending an unanswered question and calling it a
/// conversation.
pub fn replay(entries: &[Entry]) -> Vec<ChatMessage> {
    entries
        .iter()
        // Thinking is the human's record, not a turn the model had: an
        // endpoint that returns it rejects it as input on the next request.
        .filter(|e| e.role != SessionRole::Reasoning)
        // The queue is the surface's state, not a turn: it is replayed into
        // the human's list, never into the model's conversation.
        .filter(|e| e.role != SessionRole::Queue)
        .filter_map(|e| match (e.role, e.call.as_deref()) {
            (SessionRole::Tool, Some(call)) => {
                Some(ChatMessage::tool_result(call, e.content.clone()))
            }
            // A tool turn that names no call, or a call that names nothing to
            // send, is not a turn.
            (SessionRole::Tool, None) => None,
            (_, _) => match e.calls.as_deref().map(serde_json::from_str::<Vec<ToolCall>>) {
                Some(Ok(calls)) if !calls.is_empty() => {
                    Some(ChatMessage::assistant_calls(calls))
                }
                _ => Some(ChatMessage::new(provider_role(e.role), e.content.clone())),
            },
        })
        .collect()
}

fn provider_role(role: SessionRole) -> Role {
    match role {
        SessionRole::System => Role::System,
        SessionRole::User => Role::User,
        SessionRole::Assistant => Role::Assistant,
        SessionRole::Reasoning => Role::Assistant,
        SessionRole::Tool => Role::Tool,
        // A launch mark is where the view begins, so it is never on the wire.
        SessionRole::Launch => Role::System,
        // A summary is context the model reads, not a turn it had -- and it
        // is put on the human's side of the wire, because a fold that takes
        // the task's opening turn must still leave the conversation starting
        // with a user, which every endpoint requires.
        SessionRole::Compaction => Role::User,
        // Filtered out before this is asked; a queue snapshot is the human's
        // list and has no place on the wire.
        SessionRole::Queue => Role::System,
    }
}

/// Whether a refusal is about the thinking field rather than about the
/// request as a whole. The words are the houses' own names for it; a refusal
/// that mentions none of them is not evidence about the dialect.
fn names_thinking(message: &str) -> bool {
    const FIELDS: [&str; 5] =
        ["reasoning_effort", "enable_thinking", "thinking_budget", "budget_tokens", "thinking"];
    FIELDS.iter().any(|f| message.contains(f))
}
