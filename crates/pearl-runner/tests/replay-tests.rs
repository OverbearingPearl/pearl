//! Stories for the log as a request: what a log puts back on the wire.
//! A resumed log is the same conversation, so the messages it replays have to
//! be the messages the model saw. A tool result with no call id is not a
//! message the model can answer, and an endpoint says so out loud: DeepSeek
//! answered a replayed request with `messages[84]: missing field
//! 'tool_call_id'`.

use std::sync::Mutex;

use pearl_core::Queue;
use pearl_log::{Role, Store};
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_runner::Runner;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that asks for one tool call and then answers, and keeps every
/// request it was handed so a story can read what went on the wire.
struct Recorder {
    seen: Mutex<Vec<CompletionRequest>>,
    calls: Mutex<usize>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
            calls: Mutex::new(0),
        }
    }
}

impl Provider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.seen.lock().unwrap().push(request);
        let mut calls = self.calls.lock().unwrap();
        let round = *calls;
        *calls += 1;
        // The first round of a task asks for the file; every later one answers,
        // so a second queued task replays the first one's tool call.
        if round.is_multiple_of(4) {
            return Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: format!("call_{round}"),
                    kind: "function".into(),
                    function: Function {
                        name: "read".into(),
                        arguments: r#"{"path":"notes.txt"}"#.into(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion {
            content: "done".into(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_task_that_called_a_tool_when_the_next_task_is_sent_then_the_tool_turn_carries_the_id_it_answers()
 {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("notes.txt"), "the notes").unwrap();
    let provider = Recorder::new();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let mut runner = Runner::new(Queue::new(), provider, store, "mock")
        .with_tools(dir.path(), pearl_runner::Policy::WorkspaceWrite);

    runner.queue.append("read the notes");
    runner.run().await.unwrap();
    runner.queue.append("and now say something");
    runner.run().await.unwrap();

    let seen = runner.provider().seen.lock().unwrap().clone();
    let second_task = seen.last().expect("the second task's request");
    let asked = second_task
        .messages
        .iter()
        .find(|m| m.role == pearl_provider::Role::Assistant && !m.tool_calls.is_empty())
        .expect("the assistant message that asked for the file is replayed");
    let answered = second_task
        .messages
        .iter()
        .find(|m| m.role == pearl_provider::Role::Tool)
        .expect("the tool's output is replayed");
    assert_eq!(
        answered.tool_call_id.as_deref(),
        Some(asked.tool_calls[0].id.as_str())
    );
    assert_eq!(asked.tool_calls[0].function.name, "read");
    // And it is on the wire, under the name the endpoint reads.
    let body = serde_json::to_string(&second_task.messages).unwrap();
    assert!(body.contains(r#""tool_call_id":"call_0""#), "{body}");
    assert!(body.contains(r#""tool_calls":[{"#), "{body}");
}

#[rstest]
#[tokio::test]
async fn given_a_log_written_before_calls_were_recorded_when_it_is_replayed_then_no_tool_turn_goes_out_bare()
 {
    let dir = TempDir::new().unwrap();
    {
        let mut store = Store::open(dir.path().join("log.jsonl")).unwrap();
        // The shape every log written before this fix has: an announcement
        // with no calls behind it, and a tool result with no id to answer.
        store
            .append(Role::Assistant, "[calling 1 tool(s)]")
            .unwrap();
        store.append(Role::Tool, "the notes").unwrap();
    }

    let runner = Runner::new(
        Queue::new(),
        Recorder::new(),
        Store::open(dir.path().join("log.jsonl")).unwrap(),
        "mock",
    );
    let replayed = pearl_runner::replay(&runner.log().read().unwrap());
    assert!(
        replayed
            .iter()
            .all(|m| m.role != pearl_provider::Role::Tool),
        "a tool message with no id cannot be answered, so it is not sent"
    );
    assert!(
        replayed
            .iter()
            .any(|m| m.content.contains("[calling 1 tool(s)]"))
    );
}
