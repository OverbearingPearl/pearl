# 2. Why the queue is the core

Date: 2026-11-24

## Status

Accepted

## Context

The population PEARL serves lets the agent generate the code; the human barely
views or edits it. During generation the human falls into a flow void: idle,
thinking of the next prompt, or juggling sessions. The valuable, ecosystem-empty
idea is a continuous single-thread role for the human while the agent works.

## Options considered

- Make the code buffer the shared subject (an editor view): serves the shrinking
  human-primary population, not this one.
- Make subagents and parallel sessions the core: pushes the human into managing
  many conversations, which breaks single-thread attention.
- Make a visible, editable task queue the core: the human shapes the queue while
  the agent runs, a continuous role that preserves flow.

## Decision

The task queue is the core mechanism and the moat. It is a tree in data and a
single-focus flat list in view, persistent outside the model context so
compaction cannot drop it. Steering, follow-up, preempt, and nesting are queue
operations the system guarantees, not suggestions to the model. Subagents are a
second-tier amplifier served by the queue, not the bet.

## Consequences

The queue, its state machine, and its scheduler are built to the best we can;
non-core parts leverage existing crates. The human's attention stays
single-thread while execution runs in parallel underneath.

Cost: the queue is a bespoke mechanism with no off-the-shelf equivalent, so we
own its correctness entirely.
