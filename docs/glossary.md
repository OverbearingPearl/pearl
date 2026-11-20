# Glossary

One word, one meaning. A term that needs a qualifier in the sentence to be
understood belongs in two entries, not one.

Where a word means something in Pi and something else here, the entry says so.
Where PEARL invents a word, the entry names what it replaces.

---

### agent

The state machine around the loop: messages, model, tools, and the queues.
`Agent` in Pi (`packages/agent/src/agent.ts`); in PEARL the module that owns the
loop and the session handle.

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

### emitter

The function the loop is handed as `:emit`: an object two listeners can
subscribe to, or nil, in which case the loop makes its own. It is
fire-and-forget - [events](#events) travel along it and nothing comes back.

Not the same as a callback that returns a value. The one place the loop waits
for an answer is [context](#context), and the module that answers it is the one
that owns persistence.

### entry

One immutable record in a session: a message, a model change, a compaction, a
tool execution with its patch, or an undo. Entries are never edited; only
appended. An undo is another entry, not a rewrite.

### parentId

The field that makes entries a tree rather than a log
(`packages/coding-agent/src/core/session-manager.ts:60`). The entry with no
parent is the root; the entry the user is standing on is the head.

### active branch

The path from the root to the head. It is what the model reads. Other branches
stay in the file and are not part of the conversation until the head moves.

Not "the session": the session is the whole tree.

### pre-image

A copy taken of a path before a command writes it, because the turn's snapshot
does not cover that path: the file was outside the root, or ignored inside it.
Restoring it is what makes a confirmed write revertible, and a path too large to
copy is a write that says it cannot be taken back.
[ADR 25](decisions/0025-why-the-boundary-is-a-copy-and-not-a-cage.md) says why
the boundary is a copy rather than a sandbox.

### session lock

A directory beside a session file, naming the pid that intends to write it. One
writer per file; a holder whose process is gone is taken over.
[ADR 22](decisions/0022-why-a-session-file-has-one-writer.md) says why.

### session

The entry tree, plus the operations over it: append, branch, rebuild, roll back.
It owns no file format; storing it is an adapter's job. It also outlives the
console that wrote it: `pearl-session-restore` turns the entries a store read
back into a session, and that is what resuming is
([ADR 29](decisions/0029-why-resuming-is-moving-the-head-and-reporting-the-divergence.md)).

In Pi the same word sometimes means the console session; here it never does —
the interactive surface is the [console](#console).

### patch

What a mutating entry carries so its effect can be undone later: which tool
ran, which file it touched, the text it replaced and the text it wrote, plus a
hash for the cases where there is no text on one side (create, delete, rename).
The core records patches and never interprets them; reading one and checking it
against the file is a product-layer job.

Not needed for `C-/`: that path uses [change group](#change-group). A patch is
what survives a restart and what a second consumer of the runtime would read.

### tool result

What a call returns: the text the model reads, plus `:error` when it failed and
`:patch` when it wrote outside the session. A failure is a result rather than a
signal, so the turn that reads it can be told what went wrong instead of dying.

Not a [patch](#patch): a result describes one call, a patch is what a mutating
entry carries so the effect can be taken back later.

### change group

An Emacs mechanism (`prepare-change-group`, `accept-change-group`,
`cancel-change-group`) that makes a set of buffer modifications one unit. PEARL
wraps each tool call in one: if the tool fails, the group is cancelled and the
buffer is untouched; if it succeeds, `undo-amalgamate-change-group` folds the
whole call into a single undo step.

This is the everyday undo path, and it is why there is no undo command.

### undo step

One press of `C-/`. PEARL's goal is that one tool call equals one step, so the
user never has to know how the agent grouped its edits.

### timeline

The undo page: every session of a root, straightened into one list ordered by
entry timestamp, each node naming the session it belongs to. It is a projection
over the session files, so a live writer contributes a valid prefix and nothing
has to be kept in sync.

Not a record. Not a total order either: two clocks can disagree and a turn can
end after a later one began, so the list is an interleaving and the page says so
rather than pretending to a causal order. A refresh inserts and never rebuilds,
which is what lets point and folds survive it.

### snapshot

A copy of the working tree taken before a turn, as a dangling commit built
through a temporary index: the worktree and the real index are never touched,
and a file is covered whether or not it was tracked when the turn began. It is
what makes a change revertible after the buffer is gone or Emacs has
restarted.

### baseline

The snapshot a turn's changes are measured against. A file whose contents no
longer match its baseline is a conflict, and a restore refuses rather than
guesses.

### revertible

The property that a change can be taken back exactly. It holds because the change
either went through a buffer (a change group) or is covered by a snapshot, and
because every restore is checked against its baseline first.

Not "undo": undo is the gesture, revertible is the property. Not covered: side
effects outside the file system — a command that sent a request, wrote to a
database or pushed a branch is not revertible, which is why commands are
confirmed.

### rollback

The operation: cancel a change group, or restore files from a snapshot. Always
scoped to a turn or a branch, never a single edit plucked out of the middle.

### provider

A model API client: one interface, one implementation per vendor. In PEARL,
`pearl-provider`, with an SSE implementation and a fake one for tests.

### credential

The key a provider sends. PEARL resolves it when a request is made — the user's
`auth-source` entry first, then `PEARL_API_KEY`, then one prompt per session —
and stores it nowhere: not the custom file, not the session, not an event.
[ADR 17](decisions/0017-why-the-api-key-is-resolved-at-use-not-stored.md) says
why.

### fold

A region the console shows collapsed: old tool output, or a compressed range.
The content is still there and still in the buffer, so a fold is view state and
not a deletion; it is keyed by the event it belongs to and it is always
expandable by a command, never collapsed beyond reach.

### follow

Whether the page inserts what a session file gains while the user is looking at
it. On by default; the keys that change what the page shows turn it off, and `g`
turns it back on. While it is off the mode line carries a count of nodes instead
of the nodes.

Not a preference. A page that moves while it is read is worse than one that is a
minute old and says so, and reading is touching the thing the page would move.

### retry

A second attempt at a request that failed before it delivered anything. PEARL
retries 429 and 503, honours an integer `Retry-After`, and stops retrying the
moment an event has been emitted, so nothing is rendered twice and no tool call
runs in two turns.
[ADR 18](decisions/0018-why-a-retry-stops-at-the-first-byte.md) says why.

### model

The model id a request names. It is a property of the console, not of the
provider: `C-c C-m` changes it for that buffer and the next request carries
it. `pearl-console-model-default` is the default for a console that has not
chosen one.

### console

The surface that acts: a buffer where the user talks to the agent and sees the
event stream, and the one place a run is submitted, confirmed, stopped, rolled
back or moved. Every other surface is a [page](#page). A product-layer module.
It may be replaced; the loop must not know it exists.

### state

What the [console](#console) is doing, derived from [events](#events) and never
from a timer: `waiting` after a submit and before the first delta, `streaming`
while deltas arrive, `tool` while a call runs, `confirm` while a question is
open, `error` after a failure, nil when idle. Each has its own status word and
face, because "nothing yet" and "a command is running" are different facts and a
spinner can only say that time passed.

Not [session](#session) state. A state lives in the buffer and dies with it; a
fold is view state for the same reason. The record does not know the console
exists.

### context

The message sequence sent to the model for one request: system prompt, active
branch, tool declarations.

### context page

What that sequence is, drawn as a buffer of its own: the prompt first, then
the projection in the order the provider receives it, and a
[compaction](#compaction) summary where the turns it covers stood, every
section carrying an estimate. It is read-only and rebuilt from the record, so
it is a view of [context](#context) and not a second copy of it — a page that
could be edited would be a second writer of what the model reads.

### page

A surface that draws a projection of the record and takes no input: the context
page is one. A page is rebuilt from the record on demand and keeps no state a
redraw would have to preserve, so it cannot show something the record does not
say. It may not require the console or the app — the `page` layer is where that
is enforced — and an action it shows is named for the console rather than
performed by the page. It shares no buffer, no fold and no buffer-local with
the [console](#console), which is why adding one cannot move the console's
redisplay cost.

Not a second window onto the console: the console keeps its own buffer, and a
page is a different buffer over a different input.

Not the user's selection — see [selection](#selection).

### selection

The buffers, regions or files the user has chosen to make available.

Not built; [roadmap.md](roadmap.md) owns its timing. The word is reserved so
that [context](#context) is never used for it: if a selection arrives, the
console resolves it into context, and the two stay distinct.

### prompt section

One labelled piece of the system prompt, `(:name :path :text)`: the preamble
PEARL ships, the user's file, the project's file, or one another caller stores
on the session. The order is fixed in code, not by configuration.
[ADR 21](decisions/0021-why-the-prompt-has-one-mechanism-and-not-a-seam-beside-it.md)
says why.

### projection

The pure function that turns a session and the tool loadout into
[context](#context): walk the active branch, fold tool output that a later turn
has already read, and hand the result to the loop. It owns what the model sees
and nothing else — it cannot add an entry, and compaction is a change to it.

### story

A test written as Given-When-Then, one file per module. Its `:when` is exactly
one user action and each `:then` states an observable outcome, so the test is
about what a person does rather than about what a function returns. The
acceptance criterion for a roadmap stage is a story.

Not a synonym for "test": a unit test named after a function is not a story,
and a story that needs two actions is two stories.

### compaction

An entry that covers a range of older entries with a summary. It changes what
[context](#context) is built from and never what the tree contains: the covered
entries stay on the branch and expanding them is a read, which is what makes a
summary reversible.

The console draws the entry where it stands: the summary as a row and the
range it covers as a fold, closed, so the page says what the model reads next
([ADR 27](decisions/0027-why-compaction-is-a-projection-and-not-a-rewrite.md)).
