# 27. Why compaction is a projection and not a rewrite

Date: 2026-11-18

## Status

Accepted

## Context

ADR 9 left compaction out and said it was the one item deferred rather than
refused, because its seam is the projection function. That was half a design:
it says where compaction plugs in, not what it does to the record, and the two
answers that exist in the wild disagree about exactly that.

Pi compacts by summarising: when the estimate passes `contextWindow -
reserveTokens`, or the provider answers with an overflow, or the user asks, it
picks a cut point, summarises everything older with a dedicated prompt, and the
summary replaces those messages in the projection. The summary is a message, it
is rendered as one, and the originals stop being sent.

`billion-context` does something else: it injects four tools into the
conversation and lets the model decide when to fold a range; `compress` folds,
`decompress` restores, `search_context` searches what was folded, `acp_status`
reports usage. Its README is explicit that this is "incremental, reversible, and
prefix-cache friendly", and that compression costs at most two percent of a
session's cache hit rate. It can be reversible because it keeps the originals in
its own store - a second system of record, which is the price of being a proxy
that cannot see the client's session file.

Scalpel offers compression to the user instead: a threshold on growth since the
last offer, a summary turn when the offer is accepted, and the summary folded to
one line that `C-c C-o` expands. It also has the sharper primitive:
`scalpel-console-forget-history` stops sending history while leaving every turn
on screen. Its docstring has the sentence this record is built on - "Forgetting
is about what the agent reads, not about what the user sees."

PEARL has the thing neither of them has: the session is the record, and the
projection is a pure function over it (ADR 3). So the second system of record is
not necessary, and rewriting the record is not permitted. What is left to decide
is whether the projection may drop what it does not send, and whether it may do
so differently on every request.

## Options considered

- Rewrite the session: summarise old entries and replace them. Simple to
  project, and it destroys the two things the record exists for - replay and
  rollback.
- Keep a second store of originals, as a proxy must. Correct, and it makes two
  answers to "what happened" that eventually disagree.
- Fold in the projection: an entry says what it covers, the projection obeys it,
  and nothing is deleted.

## Decision

Compaction is an entry, and the projection is its only effect.

`pearl-session-compact` appends a `compaction` entry whose data carries the
entries it covers and the summary text. Nothing is removed: the covered entries
stay on the branch, `pearl-session-expand` returns them, and the record reads the
same after a restart as it did before. The projection renders the summary once,
at the compaction's position, and omits the covered entries; expanding is a read
of the branch, not a projection mode.

Three levels, applied in this order and no other, because each is less lossy than
the next:

1. **Fold** - an old tool result the branch has moved past projects as a short
   marker; the whole text stays in the entry. Lossless.
2. **Truncate** - text over a limit is cut with a marker that states the true
   size, so the model knows it was cut. Lossy and honest.
3. **Summarise** - prose over budget becomes a `compaction` entry covering the
   entries it replaces. Lossy, reversible, and attributable because the entry
   says what it covers.

The projection is append-only in one specific sense: a projection with no new
events is byte-identical to the one before it, and folding a range never changes
the messages before that range. Both are required by the provider's prefix cache,
which is keyed on the head of the message list; a projection that recomputes its
head every turn pays full price for every request. This is the one lesson from
`billion-context` that PEARL has to take, because PEARL currently re-folds on
every projection and would lose the cache on the first fold.

Triggers are three, and all three are events rather than timers: a threshold on
the token estimate, the provider's overflow answer (`:reason overflow` from the
adapter, which is the failure that actually happens), and a command the user
runs. A threshold that fires silently is refused: the console says it compacted
and offers the expansion, in the same way scalpel offers and never assumes.

## Consequences

The record stays the record: replay sends what the live session sent, rollback
still has the entries a patch refers to, and a summary can always be traced back
to what it summarised. A second surface gets compaction for free because it is a
projection rule and not a surface feature. And because the record is intact,
"undo the compaction" is not a special operation - it is expanding a view.

Cost: the projection carries the compaction's bookkeeping, so it can no longer be
"walk the branch and stringify"; it has to know which entries are covered, which
makes it slightly less trivial to test than a fold. Summarising costs a model
call, so a session that compacts needs a provider and a window size, which means
the product layer owns the act even though the core owns the rule - a split that
has to be stated in the contract or an implementer will put the call in the
projection and make it impure. And the prefix-cache requirement constrains future
folds: a better fold that rewrites the head is not available to us, even if it
would save more tokens.
