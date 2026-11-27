//! TEMPORARY repro: where the token numbers on a row come from, and what the
//! ledger leaves out.

use std::sync::Mutex;

use pearl_agent::{Agent, CompactionPolicy};
use pearl_core::{Cost, Gauge, NodeState, Queue, Settings};
use pearl_provider::{Completion, CompletionRequest, Provider, ProviderError, Usage};
use pearl_session::{Budget, Role, SessionStore};
use rstest::rstest;
use tempfile::TempDir;

/// Bills a fixed prompt per request and remembers every request it saw, so a
/// request nobody charged for is visible as a gap between two counts.
struct Bills {
    requested: Mutex<Vec<Vec<String>>>,
    prompt: u32,
}

impl Bills {
    fn new(prompt: u32) -> Self {
        Self {
            requested: Mutex::new(Vec::new()),
            prompt,
        }
    }

    fn seen(&self) -> usize {
        self.requested.lock().unwrap().len()
    }

    fn folds(&self) -> usize {
        self.requested
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.iter().any(|m| m.contains("You are compacting")))
            .count()
    }
}

impl Provider for Bills {
    async fn complete(&self, request: CompletionRequest) -> Result<Completion, ProviderError> {
        self.requested
            .lock()
            .unwrap()
            .push(request.messages.iter().map(|m| m.content.clone()).collect());
        let folding = self
            .requested
            .lock()
            .unwrap()
            .last()
            .map(|r| r.iter().any(|m| m.contains("You are compacting")))
            .unwrap_or(false);
        Ok(Completion {
            content: if folding {
                String::from("SUMMARY: folded")
            } else {
                String::from("done")
            },
            reasoning: None,
            model: "mock".into(),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: self.prompt,
                completion_tokens: 10,
                cached_tokens: 0,
            },
        })
    }
}

fn tight() -> CompactionPolicy {
    CompactionPolicy {
        budget: Budget {
            keep_recent: 3,
            tail_tokens: 200,
            min_fold_tokens: 100,
        },
    }
}

fn small_window() -> Settings {
    let settings = Settings::new("mock");
    settings.apply("window 1000").unwrap();
    settings
}

fn seed(dir: &TempDir, turns: usize) -> SessionStore {
    let mut store = SessionStore::open(dir.path().join("session.jsonl")).unwrap();
    for i in 0..turns {
        store
            .append(Role::User, format!("turn {i}: {}", "x".repeat(400)))
            .unwrap();
        store
            .append(Role::Assistant, format!("reply {i}: {}", "y".repeat(400)))
            .unwrap();
    }
    store
}

#[rstest]
#[tokio::test]
async fn repro_the_fold_request_is_billed_to_nobody() {
    let dir = TempDir::new().unwrap();
    let store = seed(&dir, 20);
    let gauge = Gauge::new();
    let mut agent = Agent::new(Queue::new(), Bills::new(4_000), store, "mock")
        .with_compaction(tight())
        .with_settings(small_window())
        .with_gauge(gauge.clone());
    agent.queue.append("the next task");

    agent.run().await.unwrap();

    let seen = agent.provider().seen();
    println!("requests the endpoint saw: {seen}");
    println!("folds among them:          {}", agent.provider().folds());
    println!("gauge turns charged:       {}", gauge.cost().turns);
    println!("gauge session bill:        {}", gauge.cost().describe());
    println!("gauge ctx reading:         {}", gauge.describe(1000));
    assert_eq!(agent.provider().folds(), 1, "no fold ran to observe");
    assert_eq!(
        gauge.cost().turns as usize,
        seen,
        "the ledger is missing {} request(s): the fold is billed to nobody",
        seen - gauge.cost().turns as usize
    );
}

#[rstest]
#[tokio::test]
async fn repro_the_row_is_a_sum_of_the_contexts_not_one_context() {
    // What `learn` does once per round (agent.rs:381-382): one `Cost::turn`,
    // added to the node. A round's prompt is the whole conversation, so the
    // sum over rounds counts the same conversation again and again.
    let q = Queue::new();
    let (id, _) = q.append("a task with twelve rounds");
    q.start(id).unwrap();
    // A conversation that grows as the loop appends tool results.
    for round in 0..12u32 {
        let prompt = 8_000 + round * 800;
        q.charge(id, Cost::turn(prompt, 300, 1_000));
    }
    let node = q.get(id).unwrap();
    let ctx = 8_000 + 11 * 800; // the last request's prompt: what `ctx` reads
    println!("the row prints:      {}", node.cost.describe());
    println!(
        "the last round cost: {}",
        node.turns.last().unwrap().cost.describe()
    );
    println!("the header ctx:      {ctx}");
    println!(
        "ratio row/ctx:       {:.1}x",
        node.cost.up as f64 / f64::from(ctx)
    );
    println!(
        "ratio row/bill:      the row's ↑ is {}x the last request's",
        node.cost.up / ctx
    );
    assert_eq!(node.cost.up, (0..12).map(|r| 8_000 + r * 800).sum::<u32>());
}

#[rstest]
#[tokio::test]
async fn repro_cached_tokens_count_as_up_as_if_they_were_new() {
    let q = Queue::new();
    let (id, _) = q.append("a task on a cached endpoint");
    q.start(id).unwrap();
    let gauge = Gauge::new();
    for _ in 0..5 {
        // Every round: 20k prompt, 18k of it served from cache.
        gauge.spent_with_cache(20_000, 18_000);
        q.charge(id, Cost::turn(20_000, 200, 900));
    }
    let node = q.get(id).unwrap();
    println!("the row prints:        {}", node.cost.describe());
    println!("the header ctx:        {}", gauge.describe(128_000));
    println!(
        "cache rate the header  claims: {}",
        gauge.cache_rate().unwrap()
    );
    println!(
        "the row counts {} prompt tokens to say that 90% of them were free",
        node.cost.up
    );
    assert_eq!(node.cost.up, 100_000);
}

#[rstest]
#[tokio::test]
async fn repro_a_restored_row_keeps_the_tokens_and_loses_the_clock() {
    let q = Queue::new();
    let (id, _) = q.append("ran before the last launch");
    q.start(id).unwrap();
    q.charge(id, Cost::turn(26_700, 89, 12_000));
    q.complete(id, "done").unwrap();
    let snapshot = q.snapshot();
    println!("snapshot: {snapshot}");

    let revived = Queue::new();
    revived.restore(&snapshot);
    let node = revived.get(id).unwrap();
    println!("row after relaunch:   {}", node.cost.describe());
    println!("worked_ms:            {}", node.worked_ms());
    assert_eq!(node.state, NodeState::Done);
    assert_eq!(node.cost.up, 26_700);
}

#[rstest]
#[tokio::test]
async fn repro_a_relaunch_keeps_the_row_totals_and_zeroes_the_session() {
    // `main.rs:109` builds a fresh `Gauge` every launch; the queue comes back
    // from the snapshot with each node's `cost` intact. The two readings the
    // architecture calls "one bill, three readings" no longer add up.
    let q = Queue::new();
    let (done, _) = q.append("finished last launch");
    q.start(done).unwrap();
    q.charge(done, Cost::turn(148_800, 3_600, 720_000));
    q.complete(done, "answered").unwrap();
    let snapshot = q.snapshot();

    let revived = Queue::new();
    revived.restore(&snapshot);
    let gauge = Gauge::new(); // what `main.rs` makes on the next launch
    gauge.charge(Cost::turn(9_000, 100, 2_000)); // one round of the new work

    let row = revived.get(done).unwrap().cost;
    println!("row still prints:      {}", row.describe());
    println!("session line prints:   {}", gauge.cost().describe());
    println!(
        "the row is {}x the session total it is part of",
        row.up / gauge.cost().up.max(1)
    );
    assert!(row.up > gauge.cost().up * 10);
}
