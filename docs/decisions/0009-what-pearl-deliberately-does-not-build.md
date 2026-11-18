# 9. What PEARL deliberately does not build

Date: 2026-11-16

## Status

Amended by [31. Why a subagent is a derived session and what its writes cost](0031-why-a-subagent-is-a-derived-session-and-what-its-writes-cost.md)

## Context

Pi has an MCP client, a JavaScript sandbox, compaction, four interface modes,
skills, prompt templates, themes, packages, a model catalog, a TUI library,
and a plugin registry. Every one of them is a surface with its own
maintenance.

## Options considered

- Building each surface because Pi has one, which costs a maintainer per
  surface and buys nothing on the path to the promise.
- Refusing all of them without saying which, which leaves the next session to
  guess whether a missing feature is an oversight.

## Decision

None of them, at least not before the loop, the session and one real edit
path work. That includes a symbol locator: nothing in the edit path resolves
symbols to ranges, and ADR 4 says why.

Compaction is the one item deferred rather than refused: it changes what the
model reads, so it belongs to the projection, and ADR 27 records how it is
written down when it arrives.

Multi-session management and subagents are refused for the same reason as the
rest: they are surfaces, and neither is on the path to the promise. A subagent,
if one is ever built, is a derived session and not a shared one: its own record,
its own lock, a read-only link to the parent, and a rollback that has to expand
another session's plan. That last rule is the reason it is refused rather than
deferred with a shape, since no implementation to copy has it.

## Consequences

The package stays small enough to read in an afternoon, which is what makes
the loop reviewable and the guarantees checkable.

Cost: a user who wants one of those surfaces has to use a different tool for
it, and a contributor who wants to build one has to argue for it in a new ADR
rather than in a pull request. The refusal is also the thing most likely to
be wrong, so it is written down where it can be reversed on the record.
