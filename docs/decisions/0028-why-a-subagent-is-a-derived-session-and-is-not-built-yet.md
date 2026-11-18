# 28. Why a subagent is a derived session and is not built yet

Date: 2026-11-18

## Status

Accepted

## Context

ADR 9 listed subagents among the things PEARL deliberately does not build, in one
sentence and with no shape attached. Two things have changed since: PEARL grew
the machinery for two sessions in one root (ADR 22's lock, ADR 23's attribution),
which makes "another agent working here" a question with an answer instead of a
hypothetical; and the reference implementations turned out to disagree about
whether a subagent is even theirs.

Pi's core has no subagent: the only hits for the word in its source are in an
extension. What it has is a plugin ecosystem, and the most substantial one,
`billion-context`, ships three delegate tools (`acp_delegate`,
`acp_delegate_wait`, `acp_delegate_cancel`), a fleet shortcut, and a
`pi.subagents: false` switch to turn them off. Subagents in pi are a plugin
surface, and a separate `pi-subagents` plugin competes with it. Scalpel has no
subagents at all; what it has is `scalpel-hub`, a table of live consoles, and
`scalpel-lineage`, which is history rather than delegation.

The most useful artifact is `billion-context`'s treatment of a child session's
context, because it solves exactly the half PEARL would find easy. A child
registers `parentConversationId` at birth; the proxy records a read-only
`derivedFrom` link; `decompress` and `search_context` fall back along the parent
chain for content the child never saw, cycle-guarded and capped at depth 8.
Nothing is copied into the child and the parent is never modified.

That solves context inheritance and says nothing about undo, because a proxy does
not touch files. PEARL's difficulty is on the other side.

## Options considered

- Leave ADR 9's one-sentence refusal alone. Cheap, and it leaves the next person
  to rediscover why the obvious extension is not obvious.
- Build subagents as sibling sessions: one `task` tool, a second session id, and
  the child's result pasted into the parent. Attribution works, context does not:
  the child would have to be told everything again, and its patches would be
  attributed to a session the parent's rollback cannot reach.
- Define the shape now, build it when a need names itself.

## Decision

A subagent, when PEARL has one, is a **derived session**: its own JSONL file, its
own undo domain, its own lock, and a read-only link to the parent, exactly as
`derivedFrom` does it. The parent's entry references the child's patches; the
child inherits the parent's *entries* by reference and never a copy; and the
depth is capped and cycle-checked, because a chain of agents that each inherit
the last is how a prompt becomes unbounded.

It is not built, and the reason is specific rather than "it is hard". Undo is
session-scoped and path-grained (ADR 23). A child's turn happens *inside* the
parent's turn, so "undo that turn" in the parent must reach the child's patches,
which means the rollback plan needs a layer it does not have: a step that expands
into another session's plan, with conflicts refused by name across sessions. No
implementation we read has that layer, because the harnesses that have subagents
do not have PEARL's undo promise.

A subagent also has to be *worth* its cost: an inner run that only returns a
conclusion is what a tool call already is. The case that would justify it is a
call that needs several rounds of internal reasoning and must not pollute the
parent's transcript.

## Consequences

The refusal now has a shape, so the next person can either build it against a
definition or argue with one. The pieces it would reuse already exist: the
session file with its lock, the entry with an id and a parent, the attribution
rule that says a turn's paths are what its tools returned. What is missing is the
rollback layer, and that is a record of its own when it arrives.

Cost: nothing is built, so a user who wants a scout agent gets the manual version
- another console with another suffix - and pays for it in attention rather than
in tokens. And naming the shape now risks the shape looking more settled than it
is: `derivedFrom` is one proxy's answer to one half of the question, and the half
PEARL finds hard is the half that proxy never had to answer.
