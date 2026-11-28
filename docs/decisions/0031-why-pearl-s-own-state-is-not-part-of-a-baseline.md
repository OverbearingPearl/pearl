# 31. Why pearl's own state is not part of a baseline

Date: 2026-11-27

## Status

Amended by ADR 46

## Context

ADR 13 captures every work file under the workspace root, skipping `.git` and
build output. The log is a file under that root by default
(`pearl-log.jsonl`), so it was captured too, and ADR 29 appends to it
continuously while a command runs.

Two things follow, and the second is worse than the first. A 142 MB log made
every `bash` op read and hold 142 MB: 761 ops in one real run came to
23.68 GB of captured bytes, which is the 20 GB the human reported. And
`Baseline::restore` writes back every captured file that differs, so `z`
rewound the log to its bytes before the command — every message since then gone,
the human's list with it, and a capture taken mid-append could leave a torn
last line, and a torn line made the whole log unreadable (`StoreError::Malformed`)
until the open began putting a torn tail right (ADR 46).

The undo of a shell command was eating the conversation that recorded it.

## Options considered

- Skip the log by name in `SKIP`: works for the default path and fails for
  `--log anywhere-else.jsonl`, which is the same bug with a different name.
- Skip `*.jsonl`: the human's own data files are jsonl too, and an undo that
  quietly leaves one behind is the promise broken in the other direction.
- Restore the log but re-append the entries written since: invents a merge of
  an append-only file, and the log is not the shell's work to merge.
- Let whoever opens a log name it, and leave those paths out of every capture
  and every copy: the exclusion follows the file, not its name.

## Decision

`Own` is a handle holding the absolute paths that are pearl's state rather than
the human's work. `Runner::with_tools` names the log and its claim;
`SubagentPool` names each child's log; a child's registry inherits the parent's
names. `Baseline::capture_except` and `copy_workspace_except` leave those paths
out, and a `Baseline` keeps the handle so a restore leaves them out too —
otherwise the undo would delete the files it was told not to touch.

Scratch named by the op that owns it (`.pearl-out-<op>`) is excluded for the
same reason: two tasks run at once, and one task's output file must not enter
another's before-image.

The skip list stays as it was. This is not a bigger skip list; it is a
different question — not "which names are never work" but "which files does
this run own".

## Consequences

`z` can no longer take the conversation with it, and a fan-out child gets the
human's files without the parent's conversation. A capture in this repository
went from 142 MB and 0.70 s to 1.6 MB and 14 ms, measured over 30 captures.

The exclusion is only as good as the naming. A caller that opens a log and
forgets to name it gets the old behaviour, so the naming lives where the log is
opened rather than where the baseline is taken.

Amends ADR 13, whose skip list was the whole answer, and ADR 15, whose copy per
child no longer carries the parent's log.

Cost: a handle that must be threaded to every place a log is opened, and an
exclusion the human cannot see — a file pearl owns is a file `z` will not
restore, and nothing on the screen says which those are.
