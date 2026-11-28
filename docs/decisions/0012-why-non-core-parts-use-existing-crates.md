# 12. Why non-core parts use existing crates

Date: 2026-11-24

## Status

Accepted

## Context

PEARL's value is the queue mechanism, not the HTTP client or the TUI toolkit.
Reinventing solved problems spends effort the moat needs and adds maintenance.

## Options considered

- Build everything in-house: maximal control, but the core suffers for it.
- Leverage excellent crates for non-core parts, build the core ourselves:
  reqwest for HTTP, ratatui/crossterm for the TUI, serde/tokio/tracing/clap,
  landlock and windows-sys for the sandbox; the queue is bespoke.

## Decision

Non-core parts use stable, orthodox crates; the core queue mechanism is built to
the best we can. Folding's mechanism is ours but its summarization calls the
provider; the projection is not a reinvented summarizer.

## Consequences

Effort concentrates on the ecosystem-empty queue. Dependencies are chosen for
lifecycle stability and pinned in the workspace.

Cost: we inherit the crates' API changes and must track their stability.
