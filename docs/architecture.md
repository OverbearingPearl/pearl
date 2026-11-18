# Architecture

How the parts fit. What each choice costs is in [docs/decisions/](decisions/);
what exists today is in [roadmap.md](roadmap.md). This page is the structure:
what the units are, how they may depend on each other, how a turn flows, and
what undo guarantees.

## Units

The layer rules say what a module may depend on. This table says which modules
there are, and what each one is allowed not to know. The divisions are the
promise; whether a unit is one file or three is a refactoring.

| unit | layer | one line | may not know about |
| --- | --- | --- | --- |
| `pearl.el` | entry | autoloads only: `pearl` and the console's mode | everything else |
| `contract` | core | message, tool call, tool, event and entry, with their wire form | any IO |
| `event` | core | the event kinds and their data | who is listening |
| `loop` | core | the double loop and its two queues | buffers, files, provider names |
| `session` | core | entries, `parentId`, patches, projection, prompt assembly, rollback plan | the storage format, the files a prompt comes from |
| `tool` | core | the tool shape, dispatch, and validation at the trust boundary | which tools exist |
| `provider` | core | the request interface and the fake that tests use | HTTP, SSE |
| `provider-sse` | adapter | one HTTP client, streaming, with its timeouts and its one retry rule | the loop, the session |
| `proc` | adapter | one bounded subprocess run: deadline, output cap, diagnostics | what a command means |
| `session-jsonl` | adapter | append-only storage for entries and patches, one writer at a time | what an entry means |
| `console` | product | the buffer, rendering, batched redraw, the status line, the files the prompt is read from | the loop's internals |
| `tools-read`, `tools-edit`, `tools-shell` | product | one tool each, one unit each | each other |
| `review` | product | tool output as `next-error` and `xref` entries, folded when navigable | which tools exist |
| `undo` | product | change groups and the per-turn snapshot, wired to `C-/` | the storage format |

One unit, one file: `lisp/pearl-<unit>.el`, with the package prefix the suite
requires of published modules. Tests mirror them without one:
`test/<unit>-tests.el`. Test modules are free of the prefix on purpose (a test
is not published); the `-tests` suffix keeps a module's `provide` distinct from
the one in `lisp/`.

One test module mirrors no unit: `test/support-tests.el` declares every
planned function so a story can be written before the module exists, and the
few helpers every story shares: waiting on the event loop, temp roots, and a
local TCP server the streaming stories read from. A declaration is dropped
when the module that answers it lands; a declaration left behind is invisible,
because it only defines a function that is not already defined.

The right-hand column is the part worth arguing about in review, because it is
the part a require rule can enforce. A unit with an empty "may not know" column
has not been placed yet.

## Layers

Three layers, told apart by **dependency direction**, not by importance. Read
the picture as "is used by":

```mermaid
flowchart TB
  core["core<br/>loop, session, provider, tool, contract"]
  adapters["adapters<br/>provider-sse, session-jsonl, proc"]
  product["product<br/>console, tools, review, undo"]
  core -->|used by| adapters -->|used by| product
```

Both arrows read "is used by", and nothing points the other way: core may not
require an adapter, an adapter may not require the product. That one rule is
what the whole table below is a consequence of.

The rule is checked rather than trusted: each module's layer is declared in
`config/ci.el`, and the suite fails a `require` that points against the arrows.
[ADR 12](decisions/0012-layer-rules-are-checked-not-documented.md) records why
the table lives there.

| | what it is for | how it is tested | replacing it means |
| --- | --- | --- | --- |
| core | a guarantee | ERT alone: no network, no buffer, no user | replacing the project |
| adapters | one job, replaceable | ERT with a fake at the other end | swapping a plugin |
| product | a usable surface | end to end | swapping a shell |

Two tests settle a placement argument:

> **Delete the module. Are the core's tests still green?**

Green means it was product or adapter. Red means it was core. Applied upstream:
delete Pi's product and its core tests still pass, so that boundary is drawn
correctly.

> **Does it need to know a file path, a buffer, or a provider's name?**

Yes is product or adapter. No, only the shape of entries and patches, is core.

One thing sits in two layers on purpose. Entries, their order and their patches
are core, because they are the guarantee the loop issues; writing them to a
file is an adapter, because that is a storage decision. Split that way, "session
in the core" does not mean "IO in the core".

## The two rings

Two pictures, because there are two loops and one picture of both became a
spiral.

**Control ring**: who is watching the agent work.

```mermaid
flowchart LR
  console["console"] ==>|submit| loop[["loop"]]
  loop ==>|emit| events["events"]
  events -.->|render| console
```

**Memory ring**: what changed, what was recorded, what is read next time.

```mermaid
flowchart TB
  loop[["loop"]] -->|execute| tools["tools"]
  tools -->|patch| loop
  loop -->|append| session[("session")]
  loop -.->|emit| events["events"]
  session -->|context| loop
```

An architecture picture of an agent with no ring in it is almost always wrong,
because the job is exactly "look, change, record, look again".

The memory ring is also where PEARL differs most from Pi, and the difference is
invisible in an arrow. Pi's loop emits `events` and never reads a session: the
product subscribes, appends, and replays the branch into the next request, so
the ring leaves the core and comes back through a layer above it. Here the
session is in the core, so the ring closes inside one layer — and a guarantee no
layer above can forget is not a convention.

## The prompt

The system prompt is a list of sections in an order the code fixes, and each
section has one owner:

| section | owner | read when |
| --- | --- | --- |
| `preamble` | the package, built from the tool declarations the request carries | rendered, every request |
| `user` | `pearl-user-instructions-file`, `~/.emacs.d/pearl/AGENTS.md` by default | once, when the console is wired |
| `project` | `AGENTS.md` at the console's root, and no directory above it | once, when the console is wired |

The console reads the two files and hands them to the session; the session
assembles and never opens a file, which is why a core story can test a prompt
with a plist. The preamble is rendered untagged, and each file is tagged with
the path it came from, so the model can tell a rule written for this repository
from one written for this user. A file is read up to 32 KiB and cut with a
marker beyond that: the prompt is paid for on every request, and there is no
compaction to pay it down with.

There is no fourth way in. Text that is computed rather than stored — the
current error, the turn's number — is a section an extension stores on the
session before the next turn, and advice on `pearl-session-prompt-sections` is
how the shipped text is changed. The reasoning is in
[ADR 21](decisions/0021-why-the-prompt-has-one-mechanism-and-not-a-seam-beside-it.md);
the two records it supersedes are the history of a design that moved twice
before it settled, and are in `superseded/`. This page has the shape.

## Undo

Undo is layered, and the layers are used in this order:

| layer | mechanism | lives in | survives |
| --- | --- | --- | --- |
| one tool call | an Emacs change group, amalgamated into one `undo` step | `undo`, called by the edit tool | until the buffer is killed |
| one turn, on disk | a tree snapshot taken when the turn starts | `undo`, attached to the loop's events | a restart |
| one entry, durable | the patch the entry carries, or the turn's snapshot where only that can reverse the change | the core | a restart, and a second consumer |

The first layer is buffer-scoped on purpose: `C-/` restores the text and the
saved file follows the user's next save; a turn's state on disk is the second
layer's job, and the console's rollback is the gesture for it.

The snapshot is product work, so the loop cannot take it: `pearl-undo-attach`
subscribes to `turn-start` and `turn-end` on the event stream, and the console
wires that in when it builds the loop. The direction is the layer rule doing
its job — undo may require core, core may not require undo.

The first layer is why there is no undo command to learn: `C-/` already means
what it should mean. The second is what makes a change revertible after Emacs
has forgotten the buffer. The third is what a second consumer of the runtime
would read, and it is built when something needs it.

Three rules follow:

**1. Only entries that changed something outside the session carry a patch.**
Conversational entries carry none. If every entry had to carry one, rollback
would start depending on the assumption that an empty patch is harmless.

**2. Rollback is path-scoped, never entry-scoped.** "Undo this step" means
"undo the last write on the active path". Otherwise a parallel branch forces
the question "does that patch still apply here", which has no good answer.

**3. A restore that cannot verify its baseline refuses.** Every patch is checked
against the file before it is applied. A conflict is reported by name, not
guessed at, because a wrong restore is worse than a refused one.

A path the turn's snapshot never covered - outside the root, or ignored inside
it - is copied before the command runs, because a confirmed write that cannot be
taken back is the one thing the confirmation cannot buy. The copy is a pre-image,
it is capped at 64 MiB per path, and the plan names a step of its own kind for
it, so what comes back is a file the snapshot never had.

A turn's paths are attributed, not discovered: the patches its tools returned
plus the paths a command's write scope proved. A diff of the worktree would
answer "what changed" and the question here is "what did this turn change",
which stops being the same question the moment a second console writes in the
same root. So a path with a patch is reversed by text and leaves the rest of the
file alone, a path only a command wrote is restored whole from the turn's
snapshot, and a turn whose command scope was never proved is marked not
revertible when it lands. An undo belongs to the session the user is looking at;
repo-wide time travel is a different gesture and is not built.
[ADR 23](decisions/0023-why-an-undo-is-scoped-to-a-session-and-as-narrow-as-the-change.md)
has the reasoning.

What this does not cover is a command whose write scope could not be proved: it
is confirmed before it runs, and the confirmation is the point where the action
outranks the undo. Why the answer there is confirmation and not a sandbox is in
[ADR 5](decisions/0005-safety-revertible-edits-confirmed-commands.md).

## The event loop

The loop is asynchronous at every point where it waits: for a model reply, for
a tool, for the next turn's preparation. Emacs has no promises, so each of
those points is a callback, and the callbacks arrive from process sentinels. A
sentinel runs inside the event loop, where work blocks timers, input and
redisplay alike.

Three rules follow, and they are the reason the console never freezes without
looking like a bug:

1. **Heavy work is re-dispatched, never run in a sentinel.** A step reached
   from a callback is scheduled on a zero-delay timer, so it runs in ordinary
   command-loop context. The chain stays serialized: each step's callback is
   what schedules the next.
2. **A run carries a generation counter.** It is captured when the run starts
   and compared before every dispatch; a mismatch means the run was aborted,
   and the chain stops through the terminal callback so a caller's busy flag
   is always released.
3. **A dead buffer is an error, not a silence.** If the buffer a run belongs to
   was killed, the run reports it rather than dropping the chain. The loop
   knows the owner as a liveness predicate (`:guard'), never as a buffer, so
   the rule lives in the core without breaking the layer rule
   ([ADR 15](decisions/0015-the-loop-knows-its-owner-as-a-liveness-predicate.md)).

One consequence shapes the modules: session state is an explicit object passed
into the loop, not a buffer-local variable. The core reads the session back
through the projection, and a variable attached to a buffer cannot be read from
the core. [ADR 10](decisions/0010-deferred-work-runs-on-timers-not-in-sentinels.md)
records the decision, and [ADR 3](decisions/0003-why-the-session-lives-in-the-core.md)
records what makes it necessary.

The same choice is what makes resuming possible, and it costs one function. An
entry list an adapter read is turned back into a session by
`pearl-session-restore`, and a second attempt at a point is an append after
`pearl-session-set-head` moved there — the branch is written, never copied, so
one conversation stays one file. A fork leaves the files alone, and the console
says what the abandoned turns wrote before it moves, using the rollback plan it
already had ([ADR 29](decisions/0029-why-resuming-is-moving-the-head-and-reporting-the-divergence.md)).

## Presentation

The console is a renderer over events and nothing else knows how anything looks.
An event becomes sections - `(:text :role :fold)` - in a pure function, and the
sections are applied as text properties: a face looked up from the role, a fold
id, a help hint, `rear-nonsticky` so the user's typing inherits nothing.

Faces are a vocabulary: one face per role (the user's words, the assistant's,
tool output, the status line, an error, a fold marker), and a theme changes a
face without a formatter changing. A fold is view state, keyed by the event it
belongs to, held in the buffer and never written to the session, so folding is
reversible and the record carries no facts about how it was displayed. The
console's own state - `waiting`, `streaming`, `tool`, `confirm`, `error` - comes
from the events, because each of those is a different fact and a timer can only
say that time has passed ([ADR 26](decisions/0026-why-the-console-owns-presentation-and-the-session-owns-none.md)).

## Compaction

Compaction belongs to the read side and it is an entry: `pearl-session-compact`
appends a `compaction` entry that covers a range, the projection renders its
summary once at that position and omits what it covers, and nothing is deleted.
`pearl-session-expand` returns the covered entries, so expanding is a read rather
than a mode and "undo the compaction" is not an operation.

Three levels, in this order, each less lossy than the next: fold an old tool
result to a marker while its text stays in the entry, truncate over-long text
with a marker that states the true size, summarise prose into a compaction entry.
The order is a rule because the cheap mechanism should be exhausted before the
expensive one.

The projection is append-only in the sense a provider's cache needs: two
projections of an unchanged branch are the same list, and folding a range never
changes the messages before it. Recomputing the head every turn would be correct and
would cost full price on every request.

The trigger set is three and none of them is a timer: the token estimate passing
a reserve, the adapter's overflow answer, and a command. The rule lives in the
core because a projection that forgot compaction would send a different
conversation than replay does; the model call that produces the summary lives in
the product, because the core is tested with no network
([ADR 27](decisions/0027-why-compaction-is-a-projection-and-not-a-rewrite.md)).

## Related

- [docs/decisions/](decisions/) — one record per difference, each with its price
- [glossary.md](glossary.md) — one word, one meaning
- [roadmap.md](roadmap.md) — what exists today
