# Roadmap

A row is a promise and the acceptance that proves it. A stage is done when its
story passes, not when its code lands. The acceptance names what is true after
the feature and what was true before it.

| Stage | Delivers | Acceptance | Status |
|-------|----------|------------|--------|
| M0 | Workspace, CI subtree, layers declared | `make lint` green on an empty crate set | done |
| M1 | The queue: tree, state machine, scheduler | A queued task runs, pauses on preempt, resumes its frame, and a parent waits its children — proven by stories that fail without the queue | done |
| M2 | Session log + projection; one provider | A session resumes from its JSONL; a projection keeps the tail and drops nothing from the queue; a mock provider answers offline | done |
| M3 | Tools + per-OS sandbox | An anchored edit reverts to the original bytes; a climbing path is rejected; a confined process writes inside the workspace and is denied outside it, executed for real on macOS, Linux, and Windows | done |
| M4 | The agent loop | A task crosses the wire and persists; steering drains at the boundary into the session; two tasks complete in order | done |
| M5 | The single-focus view | The view renders one flat level with badges and a cursor; narrowing shows a subtree | done |
| M6 | The live queue surface | Run `pearl queue` and the tree is a live single-focus list you edit by keyboard: take a queued node into the hand, edit it, place it back at a new position or as steering; hold/resume/steer/redirect/cancel on the focused line. Before this the surface was render-only and `pearl queue` printed a placeholder | done |
| M7 | Tool-calling loop | The agent calls read/edit/bash through the queue, an edit is revertible from the session, and the compiler feeds diagnostics back — no LSP | done |
| M8 | Subagents under the queue | A node fans out to derived sessions (`spawn`), each subagent runs its own loop (`run_subagent`) and settles as a queue event; the parent finishes only when every child settles, and the human keeps one focus. Before this a node was a single session with no fan-out | done |
| M9 | Distribution | A release tag runs `make release-check` and `make dist` on each of macOS, Linux, and Windows, and attaches one native binary per platform to the release page, so a user downloads and runs PEARL without a Rust toolchain. Before this, a binary existed only where someone had compiled it | done |
| M10 | Revertible shell | A bash op that creates, edits, or deletes files is reverted by op id to the exact prior bytes; each op's baseline is independent; the op's output reaches the model. Before this, bash was confined but its effects were permanent | done |
| M11 | Concurrent fan-out | Subagents run at the same time, each in its own workspace copy (ADR 15), proven by two children meeting a barrier that a sequential fan-out could never open; one child's writes never appear in another's copy or in the parent's tree. Before this, subagents ran one after another on the shared tree | done |
| M12 | Aside | A side question is answered against the session read-only and shown on the surface; the queue gains no node and the log gains no entry. Before this, every question had to become work or wait | done |
| M13 | Compaction that runs | A session that outgrows 75% of its window folds its oldest turns into a summary the model wrote, appends it with the range it covers, and the next request is smaller and still carries the work; a process that reopens the session reuses that summary without asking a model again, and the estimate of the context learns from each completion's billed `prompt_tokens`. Before this, `project` existed and no caller ran it, the summary was a count of entries, and nothing measured the context. Proven by `crates/pearl-agent/tests/compaction-tests.rs` and `crates/pearl-session/tests/compaction-tests.rs` | done |
| M14 | The surface runs what you place | A task placed on the surface is picked up by the scheduler, runs while the list stays editable, and reports its state as a badge: running, paused on preempt, done, failed. Before this the surface edited a queue nothing executed, and `pearl run` was the only way to start work. Proven by `crates/pearl/tests/surface-tests.rs`, `crates/pearl/tests/runner-tests.rs` and `crates/pearl-agent/tests/surface-tests.rs` | done |
| M15 | The report reaches the eye | The focused task's reply is on the screen beside the queue, without entering a mode or opening the log; a failure shows its reason, a long reply shows its newest line, and a task that has not run says so. Before this the surface drew a badge and a title, and the answer existed only in `pearl-session.jsonl`. Proven by `crates/pearl-tui/tests/report-tests.rs` and `crates/pearl/tests/surface-tests.rs` | done |
| M16 | The wait says what it is waiting for | While a task runs, the pane names the act — `asking the model`, or the tool and the file or command it touches — so the seconds of generation are not a blank. Before this a running task said `no report yet` and the badge said only that it was running. Proven by `crates/pearl-core/tests/activity-tests.rs`, `crates/pearl-agent/tests/activity-tests.rs`, `crates/pearl-tui/tests/report-tests.rs` and `crates/pearl/tests/surface-tests.rs` | done |
| M17 | A refused key says why | A key that does not apply to the focused task puts its reason on the screen in the task's own words — the state it is in and the state the move needed — and a refused placement keeps the request in the hand. Before this every refusal was swallowed by `let _ =`, so `resume` and `take` looked like a broken tool. Proven by `crates/pearl-tui/tests/notice-tests.rs` and `crates/pearl/tests/surface-tests.rs` | done |

## Deliberately leaves out

- **LSP** (ADR 4): the compiler is the feedback channel; no language server the
  user must install.
- **An editor view**: the agent generates the code; the human shapes the queue.
- **Parallel attention**: subagents are many hands under one queue, but the
  human tends one focus.
- **A container**: the boundary is a copy, a confinement you can see and kill.

## Before a tag

Re-read the four pages and the records top to bottom; the tag is the one moment
where a stale sentence is published. Run `make release-check` and `make dist` on
each platform.

## How to use this page

Move a row to `done` only when its story proves it, in the commit that makes the
story pass. If an acceptance itself was wrong, edit this page first, as the
process in `AGENTS.md` requires.