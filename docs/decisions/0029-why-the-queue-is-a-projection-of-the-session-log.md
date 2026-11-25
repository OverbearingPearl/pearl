# 29. Why the queue is a projection of the session log

Date: 2026-11-25

## Status

Accepted

## Context

A session file already held everything the model said and everything it wrote,
and `--resume` rebuilt the conversation from it -- but the queue lived only in
memory. Coming back to a directory therefore produced the strangest of
half-restores: the model remembered the work, the human saw an empty list, and
nothing on the screen said which of the two was true.

A second file would have been the quick answer, and it would have been a second
truth: a list and a log that can disagree, with no rule for which one wins after
a crash between two writes.

## Options considered

**A sidecar `pearl-queue.json` written on exit.** Simple, and wrong at exactly
the moment it matters: a kill, a crash, or a closed terminal leaves the file
behind the log, and nothing can say which is newer.

**A sidecar written whenever the queue changes.** Current, but still a second
truth: reconciliation needs a rule, and the rule would be a timestamp.

**Queue events in the log, replayed into the list.** The most faithful shape,
and the largest: every operation becomes an entry, and the queue becomes a
state machine that must replay to the same tree. Worth doing when the log needs
to answer "how did this list come to be", which today it does not.

**The queue as a snapshot entry in the same log.** One writer, one order, one
recovery rule -- the last snapshot wins, exactly like the last launch mark. The
snapshot is small (titles, states, children), loses nothing the human can see,
and costs the model nothing because it is not a turn.

## Decision

The queue is a projection of the session log, appended to it whenever it
changes. A `Queue` entry carries the serialized tree; the surface's watcher
appends a new one each time the snapshot differs, so a list typed at the
keyboard and closed without a run is still there in the morning.

No request ever reads it: `replay()` and `view()` drop `Queue` entries before
budgeting, so a snapshot is the human's state and not a token the model pays
for. On `--resume` the newest snapshot is put back on the queue with every task
that had not settled marked `held`, so coming back never starts work or spends
money; the surface says how many tasks came back.

The store's sequence counter is shared by its clones, because the runner and the
watcher both append to one file.

## Consequences

A resume now restores both halves or neither: the conversation from the log, the
list from its newest snapshot, cut at the same launch mark. A human who resumes
to a wrong path still gets the error they were given before (`nothing to
resume`), so silence is not among the outcomes.

The ledger is not restored: `z` after a resume has no before-images, and the
surface says so rather than pretending the first undo will work.

Cost: one more role in the log, two writers on one file (serialized by a shared
counter, not by a lock across processes), and a snapshot's size that grows with
the list rather than with the work.
