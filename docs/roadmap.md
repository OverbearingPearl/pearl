# Roadmap

A row is a promise and the acceptance that proves it. A stage is done when its
story passes, not when its code lands. The acceptance names what is true after
the feature and what was true before it.

| Stage | Delivers | Acceptance | Status |
|-------|----------|------------|--------|
| M0 | Workspace, CI subtree, layers declared | `make lint` green on an empty crate set | done |
| M1 | The queue: tree, state machine, scheduler | A queued task runs, pauses on preempt, resumes its frame, and a parent waits its children — proven by stories that fail without the queue | done |
| M2 | Session log + projection; one provider | A session resumes from its JSONL; a projection keeps the tail and drops nothing from the queue; a mock provider answers offline | done |
| M3 | Tools + per-OS sandbox | An anchored edit reverts to the original bytes; a climbing path is rejected; the sandbox compiles for macOS, Linux, and Windows | done |
| M4 | The agent loop | A task crosses the wire and persists; steering drains at the boundary into the session; two tasks complete in order | done |
| M5 | The single-focus view | The view renders one flat level with badges and a cursor; narrowing shows a subtree | done |
| M6 | The live queue surface | Run `pearl queue` and the tree is a live single-focus list you edit by keyboard: take a queued node into the hand, edit it, place it back at a new position or as steering; hold/resume/steer/redirect/cancel on the focused line. Before this the surface was render-only and `pearl queue` printed a placeholder | done |
| M7 | Tool-calling loop | The agent calls read/edit/bash through the queue, an edit is revertible from the session, and the compiler feeds diagnostics back — no LSP | done |
| M8 | Subagents under the queue | A node fans out to derived sessions (`spawn`), each subagent runs its own loop (`run_subagent`) and settles as a queue event; the parent finishes only when every child settles, and the human keeps one focus. Before this a node was a single session with no fan-out | done |
| M9 | Distribution | `make dist` builds a native binary on macOS, Linux, and Windows; the release matrix uploads one artifact per target | todo |
| M10 | Revertible shell | A bash op that creates, edits, or deletes files is reverted by op id to the exact prior bytes; each op's baseline is independent; the op's output reaches the model. Before this, bash was confined but its effects were permanent | done |
| M11 | Concurrent fan-out | Subagents run in parallel, each in its own workspace copy, and settle as queue events; reverting one child's op never touches another's tree. Before this, subagents ran sequentially on the shared tree | todo |

## Deliberately leaves out

- **LSP** (ADR 4): the compiler is the feedback channel; no language server the
  user must install.
- **An editor view**: the agent generates the code; the human shapes the queue.
- **Parallel attention**: subagents run in parallel, but the human tends one
  focus.
- **A container**: the boundary is a copy, a confinement you can see and kill.

## Before a tag

Re-read the four pages and the records top to bottom; the tag is the one moment
where a stale sentence is published. Run `make release-check` and `make dist` on
each platform.

## How to use this page

Move a row to `done` only when its story proves it, in the commit that makes the
story pass. If an acceptance itself was wrong, edit this page first, as the
process in `AGENTS.md` requires.