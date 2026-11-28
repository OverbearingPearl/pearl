# 17. Why the queue is a handle and the runner does not exit

Date: 2026-11-24

## Status

Accepted

## Context

The surface documented itself as the program: you place a task, you watch its
badge change. Until now nothing ran a placed task. `pearl` with no arguments
built an `App` over one `Queue`, `pearl run` built an agent over another, and
the only loop that executed anything was `Runner::run`, which drained what was
already there and returned. A process with an empty queue therefore had nothing
to do, and the surface had no way to hand it work afterwards.

Two smaller faults pointed the same way. `Runner::step` let a provider error
propagate out of the loop, so one refused request ended the process rather than
the task. And `Queue` was a plain struct held by value, so two holders could
only mean two copies: the surface's tree and the runner's tree, drifting.

The queue has to be one object with two holders, and the runner has to be a
thing that waits.

## Options considered

- A `SharedQueue` wrapper that delegates every method to a mutex-protected
  `Queue`: no change to the state machine, but 22 forwarding methods that must
  be kept in step with the real API, and the same `&mut self` signatures, which
  force the surface to hold the queue exclusively and lock the runner out.
- Keep `Queue` by value and drive `step` from the surface's event loop: one
  owner and no lock, but every request blocks the terminal, and a tool loop of
  sixty rounds freezes the view the surface exists to show.
- Mirror the queue through a channel between the two sides: each side owns its
  own copy and sends updates, which is two writers over one truth, and the
  second one to add a field is how they start to disagree.
- Make `Queue` itself a handle over one tree behind a mutex: unchanged state
  machine, unchanged call sites, and a clone that shares.

## Decision

`Queue` is `Arc<Mutex<Tree>>`. Every method takes `&self`, locks for the length
of its own call, and returns owned values; `get` returns a `TaskNode` and `view`
a `Vec<TaskNode>` rather than borrows into the tree. Cloning a queue shares the
tree, so the surface and the runner are the same run by construction.

`Runner::serve(stop)` drains the queue and, when there is nothing to run, waits
on a `watch` channel instead of returning. The surface starts it over
`queue.clone()` and sends `true` on exit. `spawn_runner` takes the caller's
queue by value, so the one signature that could have created a second tree now
requires the first.

A provider error inside `step` fails that node and returns, because a refused
request is a fact about one task. The log keeps the queue's other work.

## Consequences

A task placed on the surface runs, and the list stays editable while it does:
the runner holds the lock only for the reads and writes of its own call, never
across a request. `pearl run` and the surface share one implementation of "run
what is there".

Cost: the compiler no longer proves that only one holder of a queue mutates it,
so a caller that reads a snapshot and then acts on it can act on a state that
has moved. The window is one method call, and the cost of closing it would be
holding the lock across the work — which is the freeze this record rejected.