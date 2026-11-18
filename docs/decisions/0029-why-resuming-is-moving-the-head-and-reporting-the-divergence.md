# 29. Why resuming is moving the head and reporting the divergence

Date: 2026-11-18

## Status

Accepted

## Context

A session file is the record and a console is a view over one, but nothing said
how a record becomes a session again. The roadmap's M6 acceptance — "kill Emacs,
reopen, and the history is there, in the order it happened" — is satisfied by
reading the file with no console at all, so the strongest claim the README makes
("it can be read back afterwards") had no operation behind it: a user who closed
Emacs lost the conversation while the entries sat on disk.

Resuming and branching look like two features and are one. Pi answers both with
a tree and a leaf pointer: `branch(id)` moves the leaf and deletes nothing,
`resetLeaf()` moves it to nothing so the first message can be re-edited into a
sibling, `--resume` picks a file, and `/tree` moves within one. Files are not
part of that answer — pi's own security page tells the user to keep snapshots or
version control instead, which is the divergence below, unhandled.

## Options considered

- **A `resume` entry type** — "I resumed here" is not a fact about the
  conversation, and every projection would then have to skip it.
- **Copying the path into a new file on each branch** (pi's
  `createBranchedSession`) — two files and two locks for one conversation.
- **Rewriting an earlier entry to carry the new input** (pi's
  `appendContextEdit`) — a record that can change what a turn was asked cannot be
  replayed, the same reason ADR 27 keeps compaction out of the entries it covers.
- **A picker in the core** — the core would learn where files live, which ADR 3
  forbids. The product's picker and its cost are a separate question, priced
  below.
- **Rolling the files back automatically on resume** — it deletes work the user
  may have meant to keep, and the direction has no inverse.

## Decision

Resuming restores entries into a session; branching moves the head.
`pearl-session-restore` builds a session from the entries a store returned,
with the head either named or defaulting to the last of them.
`pearl-session-set-head` accepts nil, meaning before the root, because
re-editing the first message is a sibling and not a replacement. A branch is
appended, never copied: the second attempt lands in the same file and the first
stays readable, so one conversation stays one file with one writer (ADR 22).

Moving the head changes what the model reads and does not touch the disk. That
divergence is reported rather than resolved: before moving, the console asks
`pearl-undo-plan` what the abandoned turns wrote, prints the count of
turns and the paths, and applies nothing unless asked. The report comes from the
plan that already exists, so there is one attribution mechanism and not two.

## Consequences

The record outlives the console, and a resumed session is indistinguishable from
one that never stopped: the same append, the same projection, the same rollback
plan.
A resumed session sees the disk as it is now and not as it was at the branch
point. The report is the only thing between the user and that misunderstanding,
so it is not optional, not a message that can be switched off, and not printed
after the move.
Cost: the picker lists session files by modification time and offers no search,
no tags, no rename — pi finds a session by its content and PEARL does not. A
fork into another project is absent. Nothing versions the files themselves: a
resume says what diverged and cannot take back a write made in a branch nobody
was watching.
