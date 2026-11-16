# 2. Why Emacs-native, and not an Emacs front end to a Node agent

Date: 2026-11-16

## Status

Accepted

## Context

Pi's runtime is Node (>= 22.19): the loop, the tools and the providers are
TypeScript modules, and each interface is an in-process caller of the same
runtime. An Emacs surface for that runtime already exists in several forms,
and all of them talk to a process.

## Options considered

- A bridge: Emacs draws the conversation, a Node process owns the loop, the
  two speak over stdio or RPC.
- A port: the loop, the tool dispatch and the session live in Emacs Lisp.

## Decision

The loop is Emacs Lisp, and the model stream is read inside Emacs.

## Consequences

Being in the process buys exactly three things, and they are the reason for
the decision.

**Edits join the undo history.** A tool call becomes one undo step in the
buffer it edited, so the user's `C-/` reverts it. A bridged agent cannot do
this: its writes arrive from another process and land as an opaque block.

**Unsaved buffers are context.** No save step, no IPC round trip, no second
copy of a file that is being edited.

**The editor's vocabulary is available.** `compilation-mode`, `next-error`,
`xref`, `diff-mode` instead of printed text.

Cost: one thread, so slow Elisp freezes the whole editor rather than one
agent. No isolation, so sandboxing becomes a per-platform story rather than a
container. No npm, so anything that exists only as a Node SDK is out of
reach.

What would change our mind: an implementation that uses none of the three
abilities above would be better off bridged.
