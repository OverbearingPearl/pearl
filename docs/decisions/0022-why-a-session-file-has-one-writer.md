# 22. Why a session file has one writer

Date: 2026-11-18

## Status

Accepted

## Context

The record is append-only because a session that can be rewritten is one whose
history can disagree with what the user watched happen. That argument is about
one writer. It says nothing about two, and two are easy: a second Emacs on the
same project with the default suffix, or a console opened twice with the suffix
still on the command line.

Nothing in the format resists it. Both writers append valid lines, both files
parse, the entries even keep their own order. What breaks is everything above
the line: `parentId` chains that were two conversations become one tree, a
rollback plan walks a branch that includes somebody else's turn, and the
projection replays an assistant answer to a question this user never asked. The
failure looks like a model that lost the thread, which is the hardest kind of
bug to attribute.

The existing protection is the torn-tail rule, and it is about a different
accident: a crash mid-line. A collision is not torn; it is two honest histories
in one file.

## Options considered

- Let them interleave and rely on the torn-tail report. That report is about
  line boundaries, and a collision produces well-formed lines.
- Refuse a second open by looking for the other process on each open. There is
  no such lookup; a lock is the lookup.
- Put the answer in the format, one entry per writer with a session id inside.
  Every reader would then filter, and a filter that is forgotten once is a
  transcript that silently loses half its lines.
- A lock beside the file, held by the writer, released when it closes, and
  taken over when its holder is gone.

## Decision

`pearl-session-jsonl-open` takes `:lock`, and a locked open means "I intend to
write". The lock is a directory named after the file, `<path>.lock`, holding an
`owner` file with the pid that took it: creating a directory is the one atomic
create every platform offers, and a check-then-write on a plain file is a race
with a smaller name.

A live holder signals `pearl-session-locked` naming the path and the pid, and
the console turns that into a `user-error` that names the session, says whether
the holder is this Emacs, and suggests another suffix. A holder whose pid is no
longer a process is stale: the lock is removed and taken over, once. Where the
platform cannot answer whether a pid is alive, the open refuses and the message
names the lock, so the user can remove an artifact rather than guess.

Readers do not lock. `pearl-session-jsonl-read` on a file another process is
appending to is a read of a valid prefix, which is what a transcript is anyway.

The console is the only writer today, so it takes the lock when it is wired and
the close it already performs releases it.

## Consequences

Two Emacs instances can work on one project as long as their suffixes differ,
which is what the suffix was for; the second one now says so instead of
producing a conversation that answers itself. A killed Emacs leaves a lock
behind and the next run takes it over, so the crash story stays a crash story.

Cost: one more artifact in the store directory, and a lock the user did not
create is a file they may have to remove by hand. A pid can be reused by an
unrelated process, in which case a dead session looks alive and refuses; the
message names the lock for exactly that reason. And the lock is advisory in the
plain sense: anything that writes the file without opening it through this
module is not refused, so the guarantee is about PEARL's writers, not about the
filesystem.
