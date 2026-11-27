# 37. Why the fold point is three eighths of the window

Date: 2026-11-28

## Status

Accepted

## Context

A task's prompt is a sawtooth. It climbs by one round's work at a time, and a
fold cuts it back to the retained tail. Every round of the climb is billed
again from the start of the conversation, so the price of a night is not the
peak of that sawtooth but its mean: `(fold point + retained tail) / 2`, times
the number of rounds.

The default was three quarters of the window -- 96k of a 128k window -- chosen
as a safety margin below the endpoint's limit. That is a peak-oriented choice,
and it is the wrong quantity to optimise. A session log of six real tasks
averaged 68,844 prompt tokens per request against 371,104 tokens of new content:
67.7 times the work, almost all of it the climb being read again.

Two changes landed before this one and moved the floor. Older tool outputs are
now carried as a line each (ADR 36), and a fold reads the two ends of an output
rather than its middle. With the bulk bounded, folding late no longer buys
anything: the history that reaches 96k is mostly prose and tool calls, and
cutting it at 48k loses the same kind of thing twice as often for half the
mean.

## Options considered

**Keep three quarters.** Fewest folds, and the choice every harness makes: pi
folds at `contextWindow - reserveTokens` with a 16k reserve, which is 0.875 of a
128k window. Defensible when a fold is expensive and the bulk is unbounded.
Neither is true here after ADR 36 and the ends-only fold input: a fold now bills
about 6k tokens against 40k of work between folds, so its own price is under a
percent of the night however often it runs.

**Half the window.** A fraction a human can hold in their head, and 64k of
headroom for one round's growth. Costs a mean of 36k against 28k -- 28% more for
a round number.

**An absolute token count, not a fraction.** `fold at 48000` reads better than
`trigger 0.375`, but it does not follow the window: a human who drops to a 32k
window would have a fold point above it and never fold at all. The fraction is
what makes `window` and `trigger` one knob rather than two that disagree.

**Adaptive: fold when the last round's growth predicts an overflow.** Cleverer,
and it needs a predictor of how much the next round will add, which nothing in
the loop knows. The sawtooth's mean is already the right objective and a
constant fraction hits it.

## Decision

The default trigger is 0.375: a 128k window folds at 48k. The knob is unchanged
-- `trigger` still takes any fraction from 0.1 to 0.95, and a kept config that
names one still wins over the default.

Measured on a task of sixty rounds of eight-kilobyte shell output, billing four
characters to the token: the mean prompt fell from 28,885 to 23,585, and the
sawtooth from 24k-54k to 6k-46k. The story asserts the mean stays under a fifth
of the window, which the old default fails.

## Consequences

A long task folds roughly twice as often. Each fold is a page of prose the
model writes from the two ends of the history, so the thread it keeps is the
same kind of thread it kept before -- there is no version of this where folding
at 48k loses something folding at 96k would have saved, because the fold's
contract already asks for paths, errors, decisions and open objectives rather
than for the transcript.

The retained tail is unchanged at 8k, scaled down for small windows. On a 16k
window the fold point is 6k and the tail 3k, so a small window folds every few
rounds; that is the price of a small window and the human chose it.

Two existing stories named the old number and were rewritten to name the
promise instead: the threshold story now asserts the fold point keeps its
fraction when the window moves, and the fits-without-folding story seeds a
history that fits whatever the fraction is.

Cost: twice the folds, and a default that looks timid beside every other
harness's. The number is three eighths rather than a half because the
measurement said 48k, and a reader who wants the round number has to be told
why it is not there.