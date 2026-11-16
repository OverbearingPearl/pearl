---
title: "ADR-0005: Safety: revertible edits, confirmed commands"
date: "2026-11-16"
status: accepted
summary: >-
  Edits are undone by change groups and snapshots; commands are confirmed because their effects reach outside anything undo covers.
---

# Safety: revertible edits, confirmed commands

## Context and Problem Statement

Pi has no permission system: tools and extensions run with the operating
system permissions of the Pi process, and isolation is delegated to
containers. A container is a distribution answer for a CLI. An editor plugin
is the user's own Emacs process, running with the user's own credentials, in
a directory the user is already editing.

## Considered Options

- Ask before every write, per hunk.
- No gate at all; rely on the operating system.
- A read-only sandbox.
- Per-platform kernel sandboxing (Seatbelt, Linux user namespaces, Windows
  job objects).
- Three layers of reversibility, and confirmation only where reversibility
  cannot reach.

## Decision Outcome

Three layers, relied on in this order.

| layer | mechanism | covers |
| --- | --- | --- |
| edits | one Emacs change group per tool call, amalgamated into one undo step | anything that goes through a buffer |
| files | a tree snapshot per turn, and a conflict check per file before restoring | what is on disk, including what a command wrote |
| commands | a write scope limited to the project root, and confirmation when that scope cannot be established | what a command may touch |

### Consequences

None of the three is a sandbox: this stops accidents, not adversaries. That
is stated here rather than discovered in a bug report.

Per-hunk approval is refused because the first layer already covers the
failure it guards against, and taxing every correct edit to do it teaches the
user to click through the prompt. A read-only sandbox is refused because
builds and tests must write. Per-platform kernel sandboxing is refused
because three mechanisms that each rot — Seatbelt is deprecated, Linux user
namespaces are restricted per distribution, Windows has none — buy only an
accident that undo already stops.

Cost: a command that sent a request, wrote to a database or pushed a branch
is not revertible. That is the whole reason commands are confirmed while
edits are not, and it is a limit of the promise rather than a gap in it.
