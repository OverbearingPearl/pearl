# 33. Why a snapshot carries the list and not the work

Date: 2026-11-27

## Status

Accepted

## Context

ADR 29 appends the queue to the log whenever it changes, and its cost line
claims a snapshot grows with the list rather than with the work. Measured on a
real directory, that was false. A snapshot of two dozen answered tasks was
222 KiB, of which 14 KiB was the list: `result` was 48%, `reasoning` 22%,
`turns` 21%. Every one of those bytes was already in the log as the turn that
produced it.

The watcher wrote whenever anything on a row differed, and what differed
while a request was in flight was the activity line and the climbing total.
In one live log, 31 of 34 consecutive snapshots differed only in activity,
cost or turns. A night of asking wrote the same answers hundreds of times:
142 MB of log, 133 MB of it snapshots. With ADR 31 and ADR 32 the multiplier
is gone, but the log still grew by its own size, and every reader paid for it.

## Options considered

- **Keep the snapshot whole and write it less often.** A debounce halves the
  writes and keeps the copy. The log still grows with the work, and a crash
  loses the last debounce window of the list rather than of the activity line.
- **Move the list to a sidecar file.** Rejected by ADR 29 for a reason that
  still holds: two files are two truths, and the one that is not the log has
  no sequence number, no launch mark and no coverage.
- **Cap or prune old snapshots.** The log is append-only (ADR 7). Pruning is
  a rewrite, and a rewrite of the record to save space is the trade this
  project has refused everywhere else.
- **Drop the transcript from the snapshot and read it back from the entries.**
  The answers are already there, stamped with the task that spoke them.

## Decision

A snapshot carries the list: titles, states, structure, held, question, error,
notices, steering, scope, focus, and each row's total. It does not carry
`result`, `reasoning` or `turns`. Those are read back on resume from the
entries written for that task, in the log the task wrote to.

A round's bill is a record of its own, `Role::Bill`, appended once the round
is over -- a round costs the model's wait plus the tools it asked for, and is
not final until they have run. `step` writes the rounds at every exit, and the
top of each round writes them while the task runs.

The watcher compares `Queue::standing`: the list with no activity line and no
running total. It writes the whole snapshot when that differs, so a written
snapshot is fresh rather than stale by one trigger.

A child's report is read from the child's own log beside the parent's, not
copied into the parent's: ADR 15 keeps a child's conversation its own.

Nothing is dropped from an old log. A snapshot that carries its answers is
still read, and what it carries wins -- which is also why a cancelled task
gets no answer put back however many assistant turns it wrote on the way.

## Consequences

Twenty-four answered tasks with the list written two hundred times: 21 MiB
becomes 1.6 MiB. Three seconds of one task asking the model: fourteen writes
become one. The row's total stays in the snapshot because it is a number and
not a transcript, and a restored row keeping its totals is a promise the
agent's own stories make; only a row with no total is billed from the rounds
read back, so nothing is counted twice.

A resume now reads the log for the reports as well as for the list, in one
pass. A window that dies mid-round loses that round's bill, not the rounds
the human already read. A tool that adds a way to settle a task without
writing an assistant turn adds a row that comes back with nothing to read.

Cost: the report is no longer in one place. Reading a restored row means the
snapshot plus the entries stamped with its id, and a child's row means a
second file. The invariant that keeps them consistent is that every settle
writes an answer and every round writes a bill; a path that forgets either is
a row that comes back empty, and only a story at the product level catches it.