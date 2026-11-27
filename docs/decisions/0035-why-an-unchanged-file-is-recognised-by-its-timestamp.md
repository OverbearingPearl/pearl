# 35. Why an unchanged file is recognised by its timestamp

Date: 2026-11-27

## Status

Accepted

## Context

A shell op captures the whole workspace before it runs (ADR 13), and the
bytes wait on disk under their own content's name (ADR 32). What was still
paid on every command was the reading: this repository is 275 files and
1.7 MB, and capturing it costs 90 ms cold, 42 ms warm, against **1.5 ms**
to walk and stat the same tree. A run of twenty commands therefore spent
about a second reading files that had not changed, to write down names it
already had.

The bytes cannot simply be skipped: an op's before-image must be the
bytes as of that op, or `z` restores the wrong version of a file and says
it succeeded. So the question is what evidence is enough to say "this
file is the one I already read".

## Options considered

- **Read and hash every file, every time.** Correct by construction, and
  the bill above. This is what the code did.
- **Trust size and mtime alone.** Cheap, and wrong in the case that
  matters: a filesystem stamping whole seconds cannot tell two writes
  apart inside one second, so a command that writes a file twice hands
  the next op a before-image of the first write. The undo then restores
  bytes nobody asked for, in silence.
- **Size and mtime, trusted only when the mark is older than the current
  second.** The guard git keeps on its index for exactly this reason. A
  file written in the last second is always read again; everything else
  — which is nearly every file in a workspace — is recognised by its
  stat.
- **Watch the tree with FSEvents/inotify.** Exact and cheap in steady
  state, but a platform API per OS, a daemon to keep alive, and a window
  between the event and the capture where the truth is uncertain. The
  product already refuses to need a daemon (ADR 4's neighbourhood).

## Decision

Each capture remembers, per path, the modification time, the length and
the image it read. The next capture reuses that image when the time and
length are unchanged **and** the remembered time is at least one second
older than now. Otherwise it reads the file again. The marks live with
the ledger, so every op of a run shares them and they go when it does.

Measured on this repository: 90 ms cold, then 2.2 ms, 1.9 ms, 1.9 ms for
the same 275 files.

## Consequences

A shell op's fixed cost drops by more than an order of magnitude, which
is most of what made the model's shell habit expensive. Nothing about
the interface changes: every op still has its own independent baseline,
and every op is still revertible.

Correctness now rests partly on a filesystem timestamp rather than only
on bytes. Two stories hold the two halves: an unchanged 24 MB tree must
not be read twice, and a file written twice inside one second must undo
to the first write. A future change that drops the one-second guard will
pass the first and fail the second.

Cost: a mark per file for the life of a run — a path, a time, a length
and a 128-bit name, some tens of kilobytes for this repository — and a
correctness argument that has to be re-read whenever the guard is
touched. A file whose contents change while its size and older-than-a-
second mtime are preserved (a deliberate `utimes`, or a filesystem
without timestamps) would be recognised wrongly; the product does not
defend against an adversary with write access to the workspace, and the
sandbox is what bounds that.