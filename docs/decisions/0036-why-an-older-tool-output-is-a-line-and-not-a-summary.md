# 36. Why an older tool output is a line and not a summary

Date: 2026-11-28

## Status

Accepted

## Context

A night of shell work was measured from this repository's own session log: 365
requests, 25.1 million prompt tokens, 371 thousand tokens of new content. Each
round re-reads the whole conversation, so the same `grep` was paid for a hundred
times. At the largest request, tool outputs were 79% of the characters carried
and the model's own answers were 1.7%.

Compaction (ADR 7) already exists, but it is the wrong tool for this. It fires
near the window's edge, it costs a model call to write the summary, and it
summarises prose -- the decisions and the reasoning -- which is the part worth
keeping. The bulk it would have to eat is output that is already mechanical:
a directory listing, a test run, the same file read twice.

## Options considered

- **Truncate at read time**, as Claude Code does: cap every tool result when it
  is produced. Rejected: the first reading of a `grep` is the one that matters,
  and a cap decided before anyone knows what the task needs throws away the
  answer to make room for a question nobody has asked yet.
- **Truncate only in the summarizer's input**, as pi does. Necessary but not
  sufficient: it makes the fold cheaper without making the rounds cheaper, and
  the rounds are where the money went.
- **Summarise old outputs with a model call.** Rejected: it pays a request to
  compress text the log already holds verbatim, and a summary of a test run is
  less useful than the command that produced it.
- **Drop old outputs entirely.** Rejected: an endpoint refuses a tool call with
  no result, and the model loses the handle (`[op 12]`) it needs to undo the op.
- **Move the fold line every round**, keeping a fixed number of recent outputs.
  Rejected on price: see below.

## Decision

`fold_results` shortens the older tool outputs *on the wire* to
`[folded: N bytes, L lines]` followed by the first line of the output, capped at
60 characters. The newest twelve are carried whole. Outputs under 200 bytes are
never folded -- a stub is longer than an acknowledgement.

The log is untouched. This is a projection in exactly the sense ADR 7 gives it,
and it needs no summary entry because nothing was destroyed: the model can run
the command again, and the human reading the file finds what it printed.

The line moves in whole groups of sixteen outputs, not one at a time. Of the
outputs older than the kept twelve, only `floor(old / 16) * 16` are folded. The
seam therefore stays put for sixteen rounds and then jumps.

That quantisation is the load-bearing part. A prefix cache matches bytes from
the start of the request; rewriting one output in the middle invalidates
everything after it, including the recent tail. A seam that moved every round
would make each round pay full price for the tail it just re-read -- roughly
cancelling the saving at DeepSeek's tenth-of-a-price hit rate. A seam that moves
once per sixteen rounds pays that re-read once and keeps fifteen rounds of hits.

The window is measured against the projected request, not against the log, so
the fold does not fire on a conversation that fits with room spare.

## Consequences

The model is told, once per seam move, on the task's own row: `results: folded N
older tool outputs to a line each; the log keeps them whole`. A fold that
happens silently is indistinguishable from a model that forgot, and the human
reading a report has to be able to tell which happened.

The projection is generic over the message shape (`output` and `text` closures)
because `pearl-session` sits below `pearl-provider` and must not name a wire
type. The loop applies it to the messages it is about to send; the threshold
applies it to a copy of the view, because the fold itself must summarise what
was really said.

Two constants are guesses tuned to one measured night, not to a law: twelve
outputs is about five thousand tokens of work in hand, and sixteen is a cache
re-read amortised over enough rounds to be worth it. A task whose outputs are
small will never reach the seam; a task whose outputs are huge will want a
smaller `keep`. Neither is a knob yet.

Cost: the model cannot re-read an old output from context -- it must run the
command again, which costs a round and, for a command whose answer has changed,
returns something different from what it saw. A task that reasoned from the
exact bytes of an early `grep` and needs them forty rounds later pays to get
them back. And the stub keeps only the first line, so an output whose useful
part was in the middle is a handle and a size, nothing more.
