# 27. Why compaction is a projection and not a rewrite

Date: 2026-11-18

## Status

Accepted

## Context

ADR 9 left compaction out and said it was the one item deferred rather than
refused, because its seam is the projection. That was half a design: it says
where compaction plugs in, not what it does to the record.

The three answers that exist disagree about exactly that. Pi summarises the
older messages and sends the summary in their place; it writes that summary as
an entry too, so the difference is narrower than "rewrite" - Pi records the
boundary (`firstKeptEntryId`) and not the ids it covered, and its surface shows
the result as settled rather than as a fold a reader can open. `billion-context` lets the
model fold ranges with four injected tools and keeps the originals in a store of
its own, which is the price of being a proxy that cannot read the client's
session. Scalpel offers a summary turn to the user and folds it to one line, and
says the sentence this record is built on: "Forgetting is about what the agent
reads, not about what the user sees."

PEARL has what neither of them has - the session is the record and the
projection is a pure function over it (ADR 3), so a second system of record is
unnecessary and rewriting the record is forbidden.

## Options considered

- Rewrite the session: summarise old entries and replace them. Easy to project,
  and it destroys the two things the record exists for, replay and rollback.
- Keep a second store of originals, as a proxy must. Correct, and it makes two
  answers to "what happened" that eventually disagree.
- Fold in the projection: an entry says what it covers, the projection obeys it,
  and nothing is deleted.

## Decision

Compaction is an entry, and the projection is its only effect. The entry carries
the ids it covers and the summary; the covered entries stay on the branch,
`pearl-session-expand` returns them, and the projection renders the summary once
at the compaction's position.

The cut is measured in tokens and stored as an entry id, because the constraint
is the window and a count of entries says nothing about it. It is found once,
walking back from the head, and then written into the entry, so an entry appended
later cannot move the cut under a reader; it never falls on a tool result, which
belongs to the call that produced it.

The projection is latched, which is what the provider's prefix cache actually
requires: what was sent last time is replayed, and the latch is refreshed only
by the events that must change it anyway - an append, a fold, a different tool
loadout, a head that moved. It is not refreshed by the passage of time or by a
new estimate, because that is the difference between a rule and a knob: a
projection re-derived every turn is correct and pays full price on every
request, and `billion-context` shows the same rule from the outside, where a
transform's fresh output is held back until a fold releases it.

The summary is rendered at the earliest position its covered entries occupy,
and not at the compaction entry's own, because the bytes before the fold point
are what the request is billed for; the fold should move that point as far back
as it honestly can. Two positions are refused: before the branch's first user
entry, which providers reject when it goes missing, and between a tool call and
the result that answers it, which no provider accepts on its own. A range that
begins between a call and its result is widened until the pair is inside it,
because a cut that separates them is not a smaller context, it is an invalid
one.

Triggers are three and all are events rather than timers: a threshold on the
estimate, the provider's `:reason overflow`, and a command the user runs. A
threshold that fires silently is refused; the console says it compacted.

The three levels of loss - fold, truncate, summarise - are applied in that order
and described in [the architecture page](../architecture.md#compaction), because
they describe the design in force rather than the argument for it.

## Consequences

The record stays the record: replay sends what the live session sent, rollback
still has the entries a patch refers to, and a summary traces back to what it
summarised. A second surface gets compaction for free, and "undo the
compaction" is expanding a view rather than a special operation.

Cost: the projection carries the compaction's bookkeeping, so it can no longer
be "walk the branch and stringify". Summarising costs a model call, so the
product layer performs the act while the core owns the rule - a split that has
to be stated in the contract, or an implementer will put the call inside the
projection and make it impure. The latch means the session keeps a copy of what
was last sent, and a rule about when to drop it that is wrong in the early
direction serves a stale context to a model that has no way to say so. And the
prefix-cache requirement rules out a better fold that rewrites the head, even if
it would save more tokens.
