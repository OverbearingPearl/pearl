---
title: "ADR-0009: What PEARL deliberately does not build"
date: "2026-11-16"
status: accepted
summary: >-
  No MCP, codemode, subagents, extra surfaces or locators; compaction is deferred to the projection seam and can arrive without touching the tree.
---

# Context

Pi has an MCP client, a JavaScript sandbox, compaction, four interface modes,
skills, prompt templates, themes, packages, a model catalog, a TUI library,
and a plugin registry. Every one of them is a surface with its own
maintenance.

# Decision

None of them, at least not before the loop, the session and one real edit
path work. That includes a symbol locator: nothing in the edit path resolves
symbols to ranges, and ADR 4 says why.

Compaction is the one item deferred rather than refused. It changes what the
model reads, so it hangs off the projection function and can arrive without
touching entries or patches.

Multi-session management and subagents are refused for the same reason as the
rest: they are surfaces, and neither is on the path to the promise.

# Consequences

The package stays small enough to read in an afternoon, which is what makes
the loop reviewable and the guarantees checkable.

Cost: a user who wants one of those surfaces has to use a different tool for
it, and a contributor who wants to build one has to argue for it in a new ADR
rather than in a pull request. The refusal is also the thing most likely to
be wrong, so it is written down where it can be reversed on the record.
