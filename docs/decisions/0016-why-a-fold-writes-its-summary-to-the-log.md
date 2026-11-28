# 16. Why a fold writes its summary to the log

Date: 2026-11-24

## Status

Accepted

## Context

ADR 7 made folding a projection: the log stays whole and the model is shown a
summary plus a recent tail. It left open where the summary lives. The first
implementation recomputed it on every start, and `project` returned a string
that went no further than the next request.

A fold costs a model call over the very history it is trying to shrink. Two
things follow. A resumed run pays that call again before it does any work,
and the summary it gets back is not the one the previous process used, so the
same log renders two different conversations. The second is worse than the
cost: a run that resumes twice has no fixed context, and nothing to point at
when the model loses a decision that was folded ten rounds ago.

## Options considered

- Recompute the summary on every start: no new entry shape and the log stays
  purely conversational, but every resume pays a model call before the first
  token of work, and two runs over one log disagree about what was said.
- Keep the summary in memory and write it only when the run is discarded:
  the same cost on a crash, and a resume is exactly the case where the process
  did not get to write.
- Append the summary as an entry carrying the range it covers: one model call
  per fold, a resume that reads rather than asks, and a context a reader can
  trace back through.

## Decision

A fold appends an entry with `role = Fold`, its text, and `covers`, the
seq range it stands in for. `view` drops the entries a later summary covers and
places the summary where that range stood — not at the end, where it was
appended, because a digest of the old messages has to come before the new ones.
`split` computes the new coverage from the old summary's `accounts_from`, so a
second fold reaches back to the start of the conversation rather than only to
the last summary.

The summary is text the model wrote, so the log is no longer only what a user
and a model said to each other. That is the price of a resumable context and it
is paid in one place: `Entry::covers`.

## Consequences

A resumed run reuses the summary it already has, and `view` returns the same
conversation to every process that opens the file. The log keeps every original
entry, so a bad summary can be re-derived from what it replaced.

Cost: the log gains an entry per fold whose content is machine-written prose, and
a reader has to honour `covers` to see the conversation as the model saw it.