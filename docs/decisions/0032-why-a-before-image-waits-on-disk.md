# 32. Why a before-image waits on disk

Date: 2026-11-27

## Status

Accepted

## Context

ADR 13 keeps a whole-tree before-image per `bash` op, and ADR 28 keeps every op
until it is taken back. Both are the promise: any op in this run can be undone,
on its own, byte for byte. Where the bytes wait was left unstated, and they
waited in RAM.

That multiplies the workspace by the number of commands. Twelve ops over an
8 MiB file grew the process by 88 MiB. A real run's 761 ops over a 144 MB
workspace came to 23.68 GB. Almost all of it is the same bytes again: a command
touches two files out of 265 and the other 263 are captured for nothing.

ADR 13 already allows a cheaper mechanism behind the same interface. This is
one, and the alternatives that change the promise were rejected first.

## Options considered

- Evict the oldest ops after N: an undo that silently does nothing for op 40 is
  worse than the memory it saves, and `revert` answers an unknown op with
  `Ok(0)`, so no story would have caught it. Contradicts ADR 28 and M10.
- Cap the ledger by total bytes: the same lie with a different trigger, and the
  op that gets dropped is the one the human is most likely to want back.
- Capture only the files a command touched: needs the sandbox to name what it
  wrote, which ADR 28 already records as the real fix and a larger change.
- Keep the bytes in RAM but deduplicated by content: removes the repetition and
  keeps one copy of the tree, but a large workspace still costs its own size
  forever, and a run that edits many files grows without bound.
- Write the bytes once by content into a temporary directory, and keep a name
  and a length per path per op: the tree is on disk, the ledger is a manifest.

## Decision

`Images` is a content-addressed store in the system's temporary directory,
opened on the ledger's first capture and shared by every op after it, so an
unchanged file is written once for the whole run. A `Baseline` holds
`HashMap<PathBuf, Image>` — an id and a length — and an `Arc` to the store.

The name is a 128-bit fingerprint from two runs of the standard library's
hasher over different inputs: no dependency for a number. A blob is verified
against its name on read, so a collision or a corrupted file is an error rather
than the wrong bytes restored.

Capture and restore visit one file at a time. Restore compares by name, so an
unchanged file costs a read and no write, and peak memory is one file rather
than two trees.

The store is removed when the last holder drops. A blob the system cleaned away
is an honest refusal — the revert fails with words — because taking an op back
is all or nothing.

## Consequences

The twelve-op story that measured 88 MiB of growth now measures under a
megabyte, and still restores op 0 exactly. Nothing is evicted and nothing is
capped: every op stays revertible for the run, which is what ADR 28 promised.

The bytes are now outside the workspace, so the sandbox does not confine them
and a `Policy::ReadOnly` run still writes them. That is pearl's own state, not
the agent's hands, and ADR 18 bounds the shell rather than the process.

A run that changes many large files fills the temporary directory instead of
memory. `/tmp` is not free either, and a full one refuses the command before it
runs, which is the same place a failed capture already refused it.

Cost: a revert now depends on a directory the OS may clean, an I/O bill per
changed file instead of a memory bill per op, and a fingerprint that is not a
cryptographic hash — a deliberate collision would be caught on read, not
prevented on write.
