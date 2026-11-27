//! Stories for the whole product: the surface and the runner over one queue.
//!
//! This is the seam the per-module stories cannot see. The surface owns the
//! keys, the runner owns the work, and both are given the same queue.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pearl_agent::Agent;
use pearl_core::{NodeState, Queue, TaskId};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::SessionStore;
use pearl_tui::App;
use rstest::rstest;
use tempfile::TempDir;
use tokio::sync::watch;

/// Answers every request, counting them, after a pause long enough that the
/// surface can be typed into while the first task runs.
struct Talker {
    calls: Arc<AtomicUsize>,
}

impl Talker {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Provider for Talker {
    async fn complete(&self, _request: CompletionRequest) -> Result<Completion, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(120)).await;
        Ok(Completion {
            content: format!("answer {n}"),
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage::default(),
        })
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

async fn until(queue: &Queue, id: TaskId, want: NodeState) {
    for _ in 0..200 {
        if queue.get(id).map(|n| n.state) == Some(want) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let found = queue.get(id).map(|n| n.state);
    panic!("task never reached {want:?}: found {found:?}");
}

#[rstest]
#[tokio::test]
async fn given_an_empty_surface_when_a_task_is_typed_and_placed_then_it_runs_to_done() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut app = App::new(queue.clone());
    let mut agent = Agent::new(
        queue.clone(),
        Talker::new(),
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    );
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    // What a user does: press append, type the request, confirm.
    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "ship the thing");
    app.handle_key(key(KeyCode::Enter));

    let id = queue.view().first().expect("nothing was placed").id;
    assert_eq!(queue.get(id).unwrap().title, "ship the thing");
    until(&queue, id, NodeState::Done).await;
    assert_eq!(queue.get(id).unwrap().result.as_deref(), Some("answer 0"));
    assert!(app.aside.is_none());
    drop(serving);
}

#[rstest]
#[tokio::test]
async fn given_a_task_in_flight_when_another_is_typed_then_the_keys_still_answer_and_both_finish() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut app = App::new(queue.clone());
    let mut agent = Agent::new(
        queue.clone(),
        Talker::new(),
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    );
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "first");
    app.handle_key(key(KeyCode::Enter));
    let first = queue.view().first().unwrap().id;
    until(&queue, first, NodeState::Running).await;

    // The surface is not blocked by the work: a second request is placed while
    // the first is still running.
    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "second");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(queue.len(), 2);

    until(&queue, first, NodeState::Done).await;
    let second = queue
        .view()
        .iter()
        .map(|n| n.id)
        .find(|id| *id != first)
        .unwrap();
    until(&queue, second, NodeState::Done).await;
    drop(serving);
}

#[rstest]
#[tokio::test]
async fn given_a_task_typed_on_the_surface_when_it_finishes_then_its_reply_is_on_the_screen() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut app = App::new(queue.clone());
    let mut agent = Agent::new(
        queue.clone(),
        Talker::new(),
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    );
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "ship the thing");
    app.handle_key(key(KeyCode::Enter));
    let id = queue.view().first().unwrap().id;
    until(&queue, id, NodeState::Done).await;

    // The whole promise: the work was done, and the person can read what came
    // of it without leaving the surface or opening the log.
    let screen = pearl_tui::render_to_string(&queue, 80, 24);
    assert!(
        screen.contains("answer 0"),
        "the reply never reached the eye:\n{screen}"
    );
    drop(serving);
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn given_a_task_in_flight_when_the_screen_is_drawn_then_it_says_what_it_is_doing() {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut app = App::new(queue.clone());
    let mut agent = Agent::new(
        queue.clone(),
        Talker::new(),
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    );
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "ship the thing");
    app.handle_key(key(KeyCode::Enter));
    let id = queue.view().first().unwrap().id;
    until(&queue, id, NodeState::Running).await;

    // Draw while it runs, which is when the human is looking. The pane has to
    // say something other than a badge: that is the whole point of the surface.
    let mut screens = Vec::new();
    for _ in 0..20 {
        screens.push(pearl_tui::render_to_string(&queue, 80, 24));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        screens.iter().any(|s| s.contains("asking the model")),
        "a running task showed nothing of its work:\n{}",
        screens.last().unwrap()
    );
    until(&queue, id, NodeState::Done).await;
    drop(serving);
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn given_a_task_in_flight_when_a_key_that_does_not_apply_is_pressed_then_the_screen_says_why_not()
 {
    let dir = TempDir::new().unwrap();
    let queue = Queue::new();
    let mut app = App::new(queue.clone());
    let mut agent = Agent::new(
        queue.clone(),
        Talker::new(),
        SessionStore::open(dir.path().join("s.jsonl")).unwrap(),
        "mock",
    );
    let (_stop, stop_rx) = watch::channel(false);
    let serving = tokio::spawn(async move { agent.serve(stop_rx).await });

    app.handle_key(key(KeyCode::Char('a')));
    typed(&mut app, "ship the thing");
    app.handle_key(key(KeyCode::Enter));
    let id = queue.view().first().unwrap().id;
    until(&queue, id, NodeState::Running).await;

    // A key that does not apply. The human pressed it, so the surface owes
    // them the reason rather than a screen that looks exactly as before.
    app.handle_key(key(KeyCode::Char('R')));
    let screen = pearl_tui::screen(&app, 80, 24);
    assert!(
        screen.contains("retry:") && screen.contains("needs"),
        "the refused key left no trace on the screen:\n{screen}"
    );
    until(&queue, id, NodeState::Done).await;
    drop(serving);
}
