# 11. Why the view is single-focus and flat

Date: 2026-11-24

## Status

Accepted

## Context

The queue is a tree in data, but human attention is single-thread. Rendering the
whole tree at once splits attention; rendering only one node hides the parallel
work underneath.

## Options considered

- A single flat list with no nesting: simple, but cannot express subtasks.
- Multiple lanes or a fully visible tree: expresses parallelism, but splits the
  human's attention and is hard to edit.
- A single-focus flat list at the current level, with drill-in/out: the data is a
  tree, the view is flat, status badges give awareness and one cursor gives
  attention.

## Decision

The view is always a single-focus flat list at the current level (narrow to a
subtree, drill in and out). Parallelism is shown as status badges you scan;
attention is the one cursor. The human edits a flat list and navigates, never the
tree directly.

## Consequences

Single-thread attention is preserved while execution runs in parallel. The shape
mirrors a file manager or a debugger call stack.

Cost: the human sees one level at a time and must navigate to see another.
