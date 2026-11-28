# 10. Why steering drains at the boundary

Date: 2026-11-24

## Status

Accepted

## Context

The human shapes a running task by inserting steering. Injecting it mid-token
corrupts the model's round; the queue must guarantee the human's note is not
forgotten and not swallowed by the agent's own confirmations.

## Options considered

- Inject steering immediately mid-stream: responsive, but breaks the round.
- Queue steering and drain it at the next safe round boundary: the note waits,
  then enters as a unit before the next round.

## Decision

Steering is an annotation on a queued or running node. It is stored on the node
and drained at the round boundary, never mid-token. An agent confirmation is a
state of the running node (`AwaitingInput`), not a new task, so it never swallows
queued work. Preempt saves a frame and pauses; resume restores it.

## Consequences

The human's note is a system guarantee, persisted outside the model context.
"Put it down now" realistically means the next safe boundary.

Cost: steering is not instantaneous; it lands at the boundary, not mid-token.
