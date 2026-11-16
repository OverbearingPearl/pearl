---
title: "ADR-0010: Deferred work runs on timers, not in sentinels"
date: "2026-11-16"
status: accepted
summary: >-
  Callbacks arrive from sentinels, so work is re-dispatched on a zero-delay timer, each dispatch is guarded by a generation counter, and session state is never buffer-local.
---

# Deferred work runs on timers, not in sentinels

## Context and Problem Statement

The loop is asynchronous at nine points: reading steering messages,
preparing the next turn, emitting an event, preparing a request, streaming a
response, executing a tool, closing a turn, reading follow-up messages, and
waiting for a model reply. Emacs has no promises, so each of those points is
a callback, and the callbacks arrive from process sentinels.

A sentinel runs inside the event loop. Work done there — file IO, a
`y-or-n-p`, a long `search-forward` — blocks timers, input and redisplay for
as long as it takes.

## Decision Outcome

Three rules, applied at every suspension point.

1. **Heavy work never runs in a sentinel.** A step reached from a callback is
   re-dispatched on a zero-delay timer, so it runs in ordinary command-loop
   context.
2. **The chain stays serialized, and is guarded by a generation counter.**
   The counter is captured when a run starts and compared before every
   dispatch; a mismatch means the run was aborted and the chain stops. The
   terminal callback always runs, so a caller's busy flag is always released.
3. **A dead buffer is an error, not a silence.** If the buffer a run belongs
   to was killed, the run reports it through the error callback instead of
   dropping the chain.

Session state is never held in a buffer-local variable. It is an explicit
object passed into the loop, because the core reads it back through the
projection and a variable attached to a buffer cannot be read from the core.

### Consequences

Every asynchronous step costs one timer hop, which is measurable and paid
once per step rather than once per token. The alternatives were worse: a
synchronous loop freezes the editor, and an unguarded callback chain dies
silently, leaving a console that never accepts input again.

Cost: an extra state variable per run, and every new suspension point has to
remember the three rules. Rule 2's counter is easy to forget precisely where
it looks unnecessary — a step that cannot abort.

What would change our mind: Emacs Lisp gaining a promise interop worth
depending on.
