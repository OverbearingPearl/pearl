//! Stories for a question the runner really waits on.
//!
//! An agent that cannot ask guesses, and guessing is paid for in the human's
//! money and their files.

use std::sync::Mutex;

use pearl_agent::Agent;
use pearl_core::{NodeState, Queue};
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::SessionStore;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that asks one question through the tool, then answers.
struct Asker {
    calls: Mutex<usize>,
}

impl Provider for Asker {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        if *calls == 1 {
            return Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "call_ask".into(),
                    kind: "function".into(),
                    function: Function {
                        name: "ask".into(),
                        arguments: "{\"question\":\"which crate?\"}".into(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion {
            content: String::from("the tools one, as you said"),
            reasoning: None,
            model: "mock".into(),
            tool_calls: Vec::new(),
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_task_that_asks_when_the_human_answers_then_the_runner_carries_on() {
    let dir = TempDir::new().unwrap();
    let store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    let queue = Queue::new();
    let (id, _) = queue.append("which crate should hold this?");
    let agent = Agent::new(
        queue.clone(),
        Asker { calls: Mutex::new(0) },
        store,
        "mock",
    );
    let mut agent = agent;
    let watching = queue.clone();
    let answered = tokio::spawn(async move {
        // Wait until the row says it is waiting, then answer it like a human.
        for _ in 0..50 {
            if watching.get(id).map(|n| n.state) == Some(NodeState::AwaitingInput) {
                let asked = watching.get(id).unwrap().question.clone();
                assert_eq!(asked.as_deref(), Some("which crate?"), "the question is not on the row");
                watching.answer(id, "the tools one").unwrap();
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("the runner never asked");
    });

    let ran = agent.run().await.unwrap();
    answered.await.unwrap();

    assert_eq!(ran, 1, "the runner ran one task");
    assert_eq!(queue.get(id).unwrap().state, NodeState::Done);
    assert_eq!(
        queue.get(id).unwrap().result.as_deref(),
        Some("the tools one, as you said")
    );
}
