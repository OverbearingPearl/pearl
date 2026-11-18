# 23. Why an undo is scoped to a session and as narrow as the change

Date: 2026-11-18

## Status

Accepted

## Context

Two consoles can already run on one root with two suffixes, and the question
that follows is what "undo that turn" means when somebody else has been editing
the same tree. Nothing in the contracts answered it, and the pieces that would
answer it disagreed with each other:

- the rollback plan is built from patches, and a shell call produces none, so a
  turn that only ran commands has an empty plan and its undo is a silent no-op;
- `pearl-undo-restore` restores whole paths from a snapshot, and nothing says
  what restoring a path that did not exist at snapshot time means;
- `pearl-undo-latest` is keyed by root, so console A's rollback can offer
  console B's turn;
- `pearl-undo-turn-end` computes "what this turn changed" as a diff of the
  worktree, which cannot distinguish this turn's writes from another session's.

The last one is the deep one, because it is a definition rather than a bug. A
tree diff answers "what changed", never "who changed it", and a conversation's
undo is a claim about the second question.

## Options considered

- One undo for the whole project, with a single global timeline of turns. Simple
  to describe, and it makes `C-c C-r` in one console revert another console's
  later turns, which the user cannot see from where they are standing.
- Session undo that restores whole files from the snapshot. Correct in the
  single-writer case, and it silently reverts another session's edits elsewhere
  in the same file.
- Session undo that uses the narrowest mechanism able to express each change:
  patch reversal where a patch exists, snapshot restore only for a path a
  command wrote and the session can name, file-level refusal there.

## Decision

An undo belongs to the session whose transcript the user is looking at, and it
touches only what that session can attribute.

Attribution: a turn's paths are the patches its tools returned plus the paths a
command's write scope proved. A command whose scope could not be proved
contributes nothing, and the turn is marked not revertible **when it happens**,
in the console, rather than when the user tries to undo it. A turn with nothing
attributable is not offered for rollback at all.

Granularity: a path with a patch is reversed by the patch, because a patch
addresses the text it wrote and leaves the rest of the file alone. A path
without one is restored from the turn's snapshot, whole, and refuses if the file
changed since the turn. The plan says which step is which, so a user reading it
can tell "this line comes back" from "this file comes back".

Scope: `pearl-undo-latest` becomes session-scoped; the console remembers the
last turn of its own session. Before writing anything, the rollback reports per
path what will be reverted, which paths another session changed since, and which
paths will be refused — so the interleaving is visible instead of assumed. A
refusal is the whole plan or nothing, as with every other conflict here.

A repository-wide "put the tree back to how it was at 14:32" is a different
gesture with a different radius, and is not built: if it arrives it gets its own
name, its own confirmation, and its own record.

## Consequences

Undo cannot reach into another console's text. A same-region conflict refuses by
name instead of merging, which is what the user is told to expect. Two sessions
editing different parts of one file both keep their work, because the second
undo reverses text and not a file. And a shell write inside the proved scope is
now genuinely revertible, which it was only in theory before this record.

Cost: a shell write whose scope was not proved is not revertible by the turn
undo even though the snapshot happens to contain the bytes, because the session
cannot say which bytes were its. That is the honest half of "prove the scope":
the price of a confirmation is that the action leaves the undo's reach. And
"undo" now has two failure modes to explain — refused by name, and never offered
— which is one more thing a console has to say well.
