# 4. Why the edit path is anchored by text

Date: 2026-11-16

## Status

Accepted

## Context

An applied edit has to be located in the file before it can be made. There
are two ways to locate it: the model quotes the text it is replacing, or the
model names an intent and a locator resolves it to a range.

## Options considered

- The model supplies an anchor: a block of text that must match the file
  exactly once and must not overlap another edit in the same call.
- The model supplies an intent — a symbol, a region — and a per-language
  locator (LSP, tree-sitter, structural scanning) resolves it to a range
  before anything is written.

## Decision

An edit is a unique anchor plus its replacement, several disjoint edits per
call, as in Pi's edit tool.

## Consequences

The anchor is not the poor cousin of a locator; it is the only mechanical
proof that the model has seen the text it is changing. A locator answers
"does this symbol exist", which is weaker than "is this the text you read",
and the common failure is not a missing symbol but the right symbol edited
wrongly, which no locator catches.

The locator route was rejected rather than deferred: it costs a per-language
layer that has to be present, current and parseable; it fails differently in
every language; and it buys bulk mechanical renames, which if they ever earn
their keep are a separate tool rather than the default edit path.

Cost: a stale anchor fails loudly and takes a round trip, and there is no
deterministic rename across many files.

What would change our mind: a rename-shaped task that keeps recurring, which
would argue for a second tool, not for a different default.
