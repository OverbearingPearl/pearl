//! Stories for the loop telling the surface what it is doing.
//!
//! A tool round can take seconds. What the human gets during those seconds is
//! the point of the project, so the act is recorded where the eye can reach it
//! while the work runs -- not only in the log afterwards. The stories therefore
//! watch the node during the run, which is when a person would be looking.

use pearl_agent::Agent;
use pearl_core::Queue;
use pearl_provider::{Completion, CompletionRequest, Function, Provider, ProviderError, ToolCall};
use pearl_session::SessionStore;
use rstest::rstest;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

/// A provider that asks for one tool and then answers. It takes time to answer,
/// because a model that replies instantly gives the eye nothing to catch: the
/// story is about the wait, so the wait has to be real.
struct OneTool {
    calls: AtomicUsize,
    tool: &'static str,
    args: String,
}

impl Provider for OneTool {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        tokio::time::sleep(Duration::from_millis(120)).await;
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(Completion {
                content: String::new(),
                reasoning: None,
                model: "mock".into(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    kind: "function".into(),
                    function: Function {
                        name: self.tool.into(),
                        arguments: self.args.clone(),
                    },
                }],
                usage: Default::default(),
            });
        }
        Ok(Completion::text("finished", "mock"))
    }
}

/// A command that keeps the tool busy until the test writes `go.txt`: the
/// story must see the act while the tool runs, not race a command that
/// finishes before the eye can sample it.
fn wait_for_go(dir: &std::path::Path) -> String {
    let go = dir.join("go.txt");
    if cfg!(windows) {
        // The bound is a ceiling, not the wait: the loop ends when the file
        // appears. cmd spends about a millisecond on each look, so the bound
        // is what the fixture costs when the file is missed -- twenty million
        // looks was three minutes, two hundred thousand was still three.
        // Three thousand is a few seconds, and the eye only needs a third of
        // one. The path is absolute so the wait does not depend on the
        // directory the confined shell was started in.
        format!(
            "{{\"command\":\"for /l %%i in (1,1,3000) do @if exist \\\"{}\\\" exit /b 0\"}}",
            go.display()
        )
    } else {
        format!(
            "{{\"command\":\"while [ ! -f '{}' ]; do sleep 0.05; done\"}}",
            go.display()
        )
    }
}

/// A command that returns at once: the story is about the wait for the model.
#[cfg(not(windows))]
const NOOP: &str = r#"{"command":"true"}"#;
#[cfg(windows)]
const NOOP: &str = r#"{"command":"rem"}"#;

/// Sample what the node says while the work runs, as an eye would.
/// The eye is another thread, like the surface's draw loop; on a
/// single-threaded runtime it is never polled and the story proves nothing.
fn watch(queue: &Queue) -> (Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let queue = queue.clone();
    let handle = tokio::spawn(async move {
        loop {
            for node in queue.view() {
                if let Some(act) = node.activity {
                    let mut seen = sink.lock().unwrap();
                    if seen.last() != Some(&act) {
                        seen.push(act);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    (seen, handle)
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn given_a_tool_that_takes_time_when_it_runs_then_the_task_names_the_act() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let provider = OneTool {
        calls: AtomicUsize::new(0),
        tool: "bash",
        args: wait_for_go(dir.path()),
    };
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Agent::new(queue.clone(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    a.queue.append("wait a moment");

    // The tool waits for this file; the eye gets the whole wait to look.
    let release = dir.path().join("go.txt");
    let opener = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::write(&release, "").unwrap();
    });
    let (seen, eye) = watch(&queue);
    a.run().await.unwrap();
    eye.abort();
    opener.await.unwrap();

    let seen = seen.lock().unwrap();
    // The row names the tool and as much of the command as fits beside it.
    // What the eye is owed is that much: the tail of a command is the first
    // thing a narrow row drops, and a story that asked for the file's name
    // was asking for the part the row is allowed to lose.
    assert!(
        seen.iter()
            .any(|s| s.starts_with("bash ") && s.len() > "bash ".len()),
        "the running task never named its work: {seen:?}"
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn given_a_task_waiting_on_the_model_when_it_waits_then_it_says_so() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let provider = OneTool {
        calls: AtomicUsize::new(0),
        tool: "bash",
        args: NOOP.to_string(),
    };
    let store = SessionStore::open(dir.path().join("s.jsonl")).unwrap();
    let mut a = Agent::new(queue.clone(), provider, store, "mock")
        .with_tools(dir.path(), pearl_tools::sandbox::Policy::WorkspaceWrite);
    a.queue.append("think about it");

    let (seen, eye) = watch(&queue);
    a.run().await.unwrap();
    eye.abort();

    let seen = seen.lock().unwrap();
    assert!(
        seen.iter().any(|s| s.contains("asking the model")),
        "the wait for the model was silent: {seen:?}"
    );
}
