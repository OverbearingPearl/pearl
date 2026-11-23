# 13. Why shell revert is a workspace baseline

Date: 2026-11-24

## Status

Accepted

## Context

ADR 8 makes file edits revertible: each write keeps a before-image, and
`revert` restores it. A shell command gets no per-write hook — it is arbitrary
code, and the sandbox (ADR 5) confines where it can write but cannot take a
write back. The user's requirement is that every operation class is both
confined and revertible, including under concurrent subagents: a `bash` op
that creates, edits, or deletes files must be undoable by op id, the same
promise the edit path already keeps.

The OS-level answers do not cover the platforms we ship. OverlayFS gives real
copy-on-write isolation but is Linux-only and needs a user namespace; macOS
has no equivalent unprivileged primitive, and Windows has none at all. A FUSE
overlay would work everywhere but adds an external kernel-extension dependency
(macFUSE/WinFsp) that the user must install — the same complexity transfer
ADR 4 refused for LSP.

## Options considered

- OverlayFS per op: real kernel copy-on-write, but Linux-only; macOS and
  Windows would still need a second mechanism.
- A FUSE userspace overlay: cross-platform in theory, but requires macFUSE or
  WinFsp installed by the user, an external dependency with its own lifecycle.
- Classifying commands (git checkpoint, trash instead of rm): covers only the
  commands it knows; an unknown command silently loses the promise.
- Snapshot the whole workspace before the op, diff and restore after: pure
  Rust, identical on every platform, coarse but total.

## Decision

Every `bash` op captures a `Baseline` of the workspace before it runs: every
work file keyed by relative path, skipping `.git` and build output. The op id
is reported in the tool output. `revert_bash(op)` restores changed and deleted
files and removes created ones, then consumes the baseline. Each op owns its
baseline, so reverting one op never touches another's before-image. The
baseline is whole-tree, so two agents sharing a tree could not revert safely;
each child of a fan-out therefore works inside its own copy, and ADR 15 carries
that decision.
The command's output is captured through a per-op file inside the root and
removed after reading, so diagnostics reach the model under every sandbox.

## Consequences

Every operation class the agent has — edit, write, bash — is now confined and
revertible by the same shape of promise. The revert is coarse: it restores the
whole tree to the op's start, so two ops touching the same file must be
reverted newest-first; the queue's single focus makes that order natural.
Large workspaces pay a full copy per shell op; the skip list keeps repositories
and build output out of the copy, and a Linux OverlayFS fast path can be added
later without changing the interface.

Cost: a workspace-sized memory and I/O bill on every shell op, and no
per-file granularity inside one op.
