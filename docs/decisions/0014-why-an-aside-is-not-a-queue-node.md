# 14. Why an aside is not a queue node

Date: 2026-11-24

## Status

Accepted

## Context

While the queue runs, the human has questions that are not work: "why does
the borrow checker complain here?", "what does this flag do?". The `/btw`
move, named by oh-my-pi, answers such a question against the current
conversation without disturbing it. PEARL's queue is the handoff desk for
work; putting a question on it would make the scheduler dispatch it as a
task, give it a state machine it does not need, and spend the human's single
focus on something that was never meant to be tended.

The question still needs the log as context — an answer about "this
file" is worthless without the conversation — but it must not become part
of that conversation, or the next task's model context carries a digression
the human already closed.

## Options considered

- A low-priority queue node: the scheduler would run it as work, and the
  queue's promise — every node is something the human asked to be done —
  would bend to hold something that is only asked to be answered.
- A log entry excluded from the projection: keeps the record complete, but
  adds a role the model never sees and a projection rule to remember; it is
  the route to take if asides ever need an audit trail.
- A persisted side branch, as oh-my-pi keeps btw-history: a second store
  with its own lifecycle, more machinery than a quick question deserves.

## Decision

An `Aside` is a single loose node held outside the queue: a question, an
optional answer, and a three-state life (`Asking`, `Answered`, `Failed`).
`Runner::btw` builds the model's messages from the log read-only, appends
the question, asks with no tools, and returns the answer; nothing is
appended to the log and no queue node is created. The surface holds the
aside (`b` to ask, Esc to dismiss), shows it on its own line, and settles it
when the answer arrives. The scheduler never sees it.

## Consequences

A digression costs one provider call and leaves no trace: the log, the
queue, and the next task's context are exactly what they were. The surface
keeps the human's attention on the queue while the question is answered
beside it.

Cost: an aside is ephemeral — it vanishes on restart and has no audit
trail — and it cannot yet be followed up in place; a second question is a
new aside. If either becomes a real need, the log-entry route above is the
way, not a queue node.
