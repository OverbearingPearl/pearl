# 8. What we take from Pi verbatim

Date: 2026-11-16

## Status

Accepted

## Context

Most of the loop is not in dispute. Copying it deliberately, and listing what
was copied, is cheaper than re-deriving it and lets a reviewer check the port
against one table instead of against the whole of Pi.

The table is pinned to a version, because Pi moves. A decision that cites a
file and a line is only checkable while the line still says the same thing.

## Options considered

- Re-deciding each borrowed detail on its own merits, which is how a port
  drifts into a different design without anyone choosing one.
- Copying Pi wholesale, which imports the surfaces ADR 9 refuses.

## Decision

Pinned to Pi 1.5.2, `packages/agent/src/` and `packages/coding-agent/src/`.
Everything below is copied without a design of our own.

| taken as is | what it pins down |
| --- | --- |
| event names and their order | `agent_start`, `turn_start`, `message_{start,update,end}`, `tool_execution_{start,update,end}`, `turn_end`, `agent_end` |
| queue insertion points | steering joins the inner loop's condition; follow-up is read only after the inner loop exits |
| tool argument shape | `edits[]`, each with an `oldText` that must match uniquely and not overlap |
| read offsets | `read` takes `offset` and `limit`, and says which range it returned |
| failures as results | a tool that fails returns a result marked as an error, it does not throw |
| output bounding | head or tail windows with an explicit truncated marker |
| truncated replies | a `length` stop fails every tool call of that turn instead of running them |
| abort | stop the run, keep the tool results already produced, hand queued messages back |
| one conversion point | the internal message becomes the wire message in one function and nowhere else |
| validation | tool arguments are checked once, at the trust boundary |
| tool order | calls within one turn run in order, and their results keep that order |
| tool loadout changes | a change to the available tools is declared to the model as a difference on a system message, so replaying the transcript yields the same tool set |

## Consequences

Two things in the pinned version are deliberately **not** taken, and are
recorded here so the omission is a decision rather than an oversight.

**Parallel tool execution.** Pi runs a batch of independent tool calls
concurrently. PEARL runs them in order, because the first layer of the safety
story is a change group per call and an interactive confirmation can appear
between two calls; concurrency would interleave both.

**A self-terminating event stream.** Pi's stream carries its own completion
predicate and accumulates the run's messages. PEARL's events are emitted and
not waited on; the loop owns termination and hands back the messages.

Cost: a port is only as good as the pin. When Pi moves, this table has to be
re-read, and a row that no longer matches its cited line is a decision to
make again rather than a line to edit quietly.
