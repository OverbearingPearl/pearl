# 7. Why compaction is a projection

Date: 2026-11-24

## Status

Accepted, amended by [16](0016-why-a-fold-writes-its-summary-to-the-log.md)

## Context

Long sessions exceed the context window. Rewriting history to compact it loses
the canonical replayable log and risks dropping the task queue. The queue must
survive compaction because it is the human's editable subject.

## Options considered

- Rewrite the session in place: smaller, but the original log is gone and the
  queue could be summarized away.
- A projection: keep the full log, render a summary plus recent entries for the
  model, leave the source untouched.

## Decision

Compaction is a projection, not a rewrite. `pearl-session` keeps the append-only
JSONL log; `project` renders a summary plus the retained tail for the model. The queue lives outside the model context entirely, so no
projection can drop it. Summarization calls a model through a trait; the
mechanism is independent of that choice.

## Consequences

The canonical log stays replayable and the queue is never lost. The non-core
summarization leverages the provider rather than a bespoke engine.

Cost: the full log is retained on disk, so a long session costs disk as well as
the summary that stands in for it.
