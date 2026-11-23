# 8. Why edits are anchored and revertible

Date: 2026-11-24

## Status

Accepted

## Context

An agent that writes code must be able to take a write back, and its edits must
survive the file shifting under them. Line-number edits break when lines move;
un-snapshotted writes cannot be reverted.

## Options considered

- Line-number edits with no snapshot: simple, but fragile and irreversible.
- Anchored text edits with a per-file snapshot: the anchor is unique text, the
  first edit saves the original bytes, and revert restores them.

## Decision

Edits are anchored by unique text and revertible by snapshot. `Workspace::edit`
requires the anchor to match exactly once, snapshots the file before the first
change, and `revert`/`revert_all` restore the prior state (removing a file that
did not exist). Paths are confined to the root; the boundary is a copy.

## Consequences

A write is proved only where a snapshot can take it back. Ambiguous or missing
anchors are rejected rather than guessed.

Cost: each touched file keeps its original bytes in memory until revert.
