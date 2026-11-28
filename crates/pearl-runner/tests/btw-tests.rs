//! Stories for the side question: an answer that leaves no trace in the record.

use std::sync::Mutex;

use pearl_core::Queue;
use pearl_log::Store;
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError};
use pearl_runner::Runner;
use rstest::rstest;
use tempfile::TempDir;

/// A provider that answers a fixed string and remembers the last message it
/// was asked, so a story can prove the question reached the model.
struct CapturingProvider {
    answer: &'static str,
    last: Mutex<Option<String>>,
}

impl Provider for CapturingProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        let last = request.messages.last().map(|m| m.content.clone());
        *self.last.lock().unwrap() = last;
        Ok(Completion {
            content: self.answer.to_string(),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Default::default(),
        })
    }
}

fn runner(dir: &TempDir) -> Runner<CapturingProvider> {
    let store = Store::open(dir.path().join("log.jsonl")).unwrap();
    let provider = CapturingProvider {
        answer: "the side answer",
        last: Mutex::new(None),
    };
    Runner::new(Queue::new(), provider, store, "mock")
}

#[rstest]
#[tokio::test]
async fn given_a_log_when_btw_then_answer_returns_and_nothing_is_recorded() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir);
    let (id, _) = a.queue.append("the real work");
    a.run().await.unwrap();
    let before = a.log().read().unwrap().len();
    let view_before = a.queue.view().len();

    let answer = a.btw("by the way, why?").await.unwrap();

    assert_eq!(answer, "the side answer");
    // The question reached the model as the last message.
    assert_eq!(
        a.provider().last.lock().unwrap().as_deref(),
        Some("by the way, why?")
    );
    // The log did not grow, and the queue did not gain a node.
    assert_eq!(a.log().read().unwrap().len(), before);
    assert_eq!(a.queue.view().len(), view_before);
    assert_eq!(a.queue.get(id).unwrap().state, pearl_core::NodeState::Done);
}

#[rstest]
#[tokio::test]
async fn given_an_empty_log_when_btw_then_the_question_still_reaches_the_model() {
    let dir = TempDir::new().unwrap();
    let mut a = runner(&dir);
    let answer = a.btw("a cold question").await.unwrap();
    assert_eq!(answer, "the side answer");
    assert_eq!(
        a.provider().last.lock().unwrap().as_deref(),
        Some("a cold question")
    );
    assert!(a.log().read().unwrap().is_empty());
}
