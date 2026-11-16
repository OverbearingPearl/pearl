# Glossary

One word, one meaning. A term that needs a qualifier in the sentence to be
understood belongs in two entries, not one.

Where a word means something in Pi and something else here, the entry says so.
Where PEARL invents a word, the entry names what it replaces.

---

### agent

The state machine around the loop: messages, model, tools, and the queues.
`Agent` in Pi (`packages/agent/src/agent.ts`); in PEARL the module that owns
the loop and the session handle.

Not a synonym for "assistant", and not the planner inside one turn.

### run

Everything from `agent-start` to `agent-end`: one admission of input, several
turns.

### turn

One assistant response plus the tool calls it makes. A run is a sequence of
turns. Turn boundaries sit between the inner loop's iterations, never inside
one.

### loop

The control flow, and specifically a **double loop**: an inner loop that
repeats while there are tool calls or steering messages, and an outer loop that
re-enters the inner one when follow-up messages are waiting.

Called `runLoop` in Pi (`packages/agent/src/agent-loop.ts:187`).

### steering

A message delivered after the current turn's tool calls and before the next
model request. It interrupts the conversation without ending the run. It is the
only channel that can arrive mid-run.

### follow-up

A message delivered when the agent would otherwise stop. It re-enters the inner
loop as a new outer iteration, so it never splits a turn.

### events

What the loop emits as it runs: turn boundaries, message deltas, tool calls and
results. Asynchronous and fire-and-forget - the loop does not wait for a
consumer and nothing comes back along that edge.

Distinct from [context](#context), which is what the loop asks for before each
request: that edge returns a value, and whoever carries it is who owns
persistence. In Pi it runs through the application layer; in PEARL it runs
inside the core.

### entry

One immutable record in a session: a message, a model change, a compaction, or
a tool execution with its patch. Entries are never edited; only appended.

### parentId

The field that makes entries a tree rather than a log
(`packages/coding-agent/src/core/session-manager.ts:60`). The entry with no
parent is the root; the entry the user is standing on is the head.

### active branch

The path from the root to the head. It is what the model reads. Other branches
stay in the file and are not part of the conversation until the head moves.

Not "the session": the session is the whole tree.

### session

The entry tree, plus the operations over it: append, branch, rebuild, revert.
It owns no file format; storing it is an adapter's job.

In Pi the same word sometimes means the console session; here it never does —
the interactive surface is the [console](#console).

### reverse patch

The inverse operation an entry carries so that its effect can be undone. Only
entries that changed something outside the session carry one.

Replaces "lineage". A patch is data; a lineage was a report about data.

### patch

The value an entry carries so its effect can be undone, stored as an opaque
`(target . inverse-op)` pair. The core records patches and never interprets
them; reading and applying one is a product-layer job.

### patch log

The sequence of reverse patches along the active branch, in order. It is a
view, not a second structure.

### rollback plan

The pure function from a range of entries to either an ordered list of inverse
operations or an explicit conflict. It answers "what would undoing this mean"
without touching a buffer, which is why it can be tested with ERT alone and
why it lives in the core while applying it does not.

### revertible

The property that a change can be taken back exactly, because its anchor had to
match the file and its reverse patch was recorded in the same step.

Not "undo": undo is a gesture a user makes, revertible is a property the
runtime guarantees. Not "rollback": rollback names the operation, revertible
names the reason it is allowed.

### rollback

The operation: apply the reverse patches of a chosen range of entries. Always
path-scoped, never a single entry in isolation.

### provider

A model API client: one interface, one implementation per vendor. In PEARL,
`pearl-provider`, with an SSE implementation and a fake one for tests.

### console

The interactive surface: a buffer where the user talks to the agent and sees
the event stream. A product-layer module. It may be replaced; the loop must not
know it exists.

### context

The message sequence sent to the model for one request: system prompt, active
branch, tool declarations.

Not the user's selection — see [selection](#selection).

### selection

The buffers, regions or files the user has chosen to make available. Resolved
by the console into context; never called "context".

### compaction

Replacing the tail of what the model reads with a summary, without deleting
entries. A read-side operation: it changes what [context](#context) is built
from, not what the tree contains.

Open: whether PEARL implements it at all, and in what shape.
