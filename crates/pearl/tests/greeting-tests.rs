//! Stories for what the model is asked when the human places a greeting.
//!
//! A night began with one word -- `hello` -- and cost 601k tokens over
//! twenty-two rounds and forty tool calls, then asked a question nobody could
//! answer. The ground named where writes land and nothing else, and the request
//! ended in a queue digest that talked about rows and their owners to a
//! conversation holding one row: a greeting, and an invitation to go and look.

use std::sync::Mutex;

use pearl_core::{Queue, Settings};
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_runner::{Policy, Runner};
use rstest::rstest;
use tempfile::TempDir;

/// A provider that records what it was asked and answers once.
#[derive(Default)]
struct Listener {
    asked: Mutex<Vec<CompletionRequest>>,
}

impl Provider for Listener {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.asked.lock().unwrap().push(request);
        Ok(Completion::text("hello", "mock"))
    }
}

#[rstest]
#[tokio::test]
async fn given_a_greeting_placed_on_the_surface_when_the_model_is_asked_then_the_request_is_the_ground_and_the_greeting()
 {
    let dir = TempDir::new().unwrap();
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    pearl::ground(&store, dir.path(), Policy::WorkspaceWrite).unwrap();
    let mut runner = Runner::new(Queue::new(), Listener::default(), store, "mock")
        .with_settings(Settings::new("mock"))
        .with_tools(dir.path(), Policy::WorkspaceWrite);
    runner.queue.append("hello");

    runner.run().await.unwrap();

    let asked = runner.provider().asked.lock().unwrap();
    let first = asked.first().expect("the model was never asked");
    let ground = &first.messages[0];
    assert!(
        ground.content.contains("with no tool"),
        "the model is not told a greeting needs no tool: {}",
        ground.content
    );
    let last = first.messages.last().expect("an empty request");
    assert_eq!(
        last.content, "hello",
        "the greeting is not the last thing the model reads"
    );
    for message in &first.messages {
        assert!(
            !message.content.contains("The queue, as it stands"),
            "a lone task was sent a digest of rows it does not own: {}",
            message.content
        );
    }
}
