# 28. Why undo replays one operation instead of a tree

Date: 2026-11-25

## Status

Accepted

## Context

`z` promised to take back "the last file change the model made through its
tools" (README, help), and it did not. The ledger recorded only `bash`, because
a shell command's before-image was the whole workspace and nothing else had a
snapshot to restore. `Workspace::revert` and `revert_all` existed and were never
called; the file tools' snapshots lived for the length of one `Workspace`.

Two further faults hid behind that. Every agent started its op counter at zero
and shared one `Ops`, so two tasks wrote `op 0` over each other and `z` took
back whichever insert the map kept. And `baseline::restore` was a whole-tree
restore: it walks the current tree and deletes every path the snapshot does not
know, so taking one op back destroyed everything written after it, including the
human's own edits.

## Options considered

**Record every tool call in one whole-tree snapshot per op.** Simple, and
already written: `Baseline::capture` before each call. The price is the restore
above -- correctness is only restored by deleting other people's work.

**Record each operation's own footprint.** A shell command still needs the tree
(it can write anywhere), but `edit` and `write` name their file. Keeping that
file's before and after bytes makes the revert a single write, leaves every
other path alone, and makes it possible to refuse when the file no longer holds
what the op wrote.

**Rely on git.** Not every workspace is a repository, and a human's uncommitted
work would be at the mercy of a stash.

**Hand the model a revert tool and let it decide.** The promise is the human's
(ADR 16); an undo that needs the model to still be running is not an undo.

## Decision

An op records what it touched: a shell op records its whole-tree before-image
(`Entry::Shell`), and `edit`/`write` record the one file as a
`(before, after)` pair (`Entry::File`). `Ops` owns the id counter, so two
registries over one ledger never share an id, and `z` takes back the newest op
by id rather than by whatever the map happens to hold.

A file op is undone only when the file still holds exactly the bytes the op
wrote. When it does not, `z` says so -- "{path} changed after that write, so it
was left alone" -- and leaves the file. Taking back an older op therefore never
undoes a newer one, by the human or by the model.

## Consequences

`z` now reaches edits and writes, which is what the help sentence and the README
already claimed. The tool that failed records nothing, so no key leads to an
undo that does nothing. A shell op is still a tree restore and can still take
later work with it; that is the remaining lie, and the fix is the same shape --
per-op footprints for commands too, which needs the sandbox to name what it
wrote.

Undo is per operation, not per task: `z` steps back one op at a time, and the
order is the ledger's id order, so a task's own work and another task's
interleave the way they happened.

Cost: two shapes in the ledger instead of one, a conflict rule the human can
meet (`z` refusing with a sentence), and a shell op that can still outrun the
restore.
