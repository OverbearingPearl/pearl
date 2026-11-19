# 35. Why a commit stages only what it names

Date: 2026-11-19

## Status

Accepted

## Context

The shell tool refuses git on an unattended run, for a reason that has not
changed: a `git add -A` that runs while another session is halfway through an
edit commits that edit under this turn's message, and nothing in the result
says so.  The refusal is right and it leaves the loop unable to commit at all.

A commit is the one write whose scope is not the file it touches.  The anchor
of ADR 4, the snapshot of ADR 16 and the epoch of ADR 33 each answer a question
about a path; the question here is which paths enter the commit, and none of
them answers it.  The epoch makes a crossing visible after it happened, and by
then the commit exists.

## Options considered

- Run git through the shell tool behind the ordinary confirmation.  Rejected:
  a command that names git has no provable scope, so the prompt asks the user
  to audit a shell string for the one property that matters, and an unattended
  run refuses; the tool cannot tell `git add a.txt` from `git add -A`.
- Commit everything and rely on the epoch to report a crossing afterwards.
  Rejected: a report is not a refusal.  The failure this prevents is one the
  user discovers in a commit that already exists.
- Derive the paths from the turn's own entries, since the session records what
  it wrote.  Rejected: an entry answers "what did this turn write", and the
  commit's contents are the model's judgement after reading the diff, the
  roadmap and the tree.  Deriving them would invent a policy the record does
  not have, and it would silently drop a path the model meant to include.
- A tool that requires the paths and refuses an index that already holds
  something.  Chosen.

## Decision

The `commit` tool takes a message and the paths to commit, and it does exactly
three writes: `git add -- PATHS`, a check that the index now holds those paths
and no others, and `git commit -m MESSAGE`.  It refuses, without writing, when
there is no root, no message or no path; it refuses when the index already
holds something, because that something belongs to whoever staged it; and it
resets the paths it staged when the check or the commit fails, so a refused
commit leaves the index as it found it.

It does not confirm.  What the confirmation in ADR 5 buys is a snapshot before
an effect the turn cannot take back; committing names its paths, cannot carry a
file it was not told about, and is reversed by moving the branch back.  The
dangerous command - arbitrary git, whose scope cannot be established - is still
refused by the shell tool.  The guard is the path list, and a guard that asks a
question as well as refusing is a guard the model learns to talk around.

The result carries the new head as `:commit` and the epoch the commit was made
over as `:epoch`, so the verdict and the tree it is about travel together the
way ADR 33 asks.

A commit may also cite a gate: the note a shell result carries when the command
exited zero.  The note names every path the tree had at the time and how big it
was; a path that is not the same in the tree being committed is refused, by
name, and the caller runs the gate again.  The citation is a string the model
copies, not a claim it types: nothing about the shape has to be understood to
carry it, and a note that names no path refuses everything rather than passing
it.

## Consequences

The tool can be offered in every mode, including unattended, because the thing
that made git unsafe there was the scope.  A commit that needs a path the model
forgot is refused and retried with the path, which costs a round trip and makes
the omission visible.

A user who has staged work of their own blocks the tool until they unstage:
the refusal is deliberate, because the alternative is committing their work
under the model's message.

Cost: a path list in every commit call, an empty-index rule that refuses work
the user might have wanted included, and no pre-commit confirmation for a write
that changes history - reversible, but history is the one thing a later reader
takes as given.
