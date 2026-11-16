# Architecture

How the parts fit. What each choice costs is in [decisions.md](decisions.md);
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
| `session` | core | entries, `parentId`, patches, projection, rollback plan | the storage format |
| `tool` | core | the tool shape, dispatch, and validation at the trust boundary | which tools exist |
| `provider` | core | the request interface and the fake that tests use | HTTP, SSE |
| `provider-sse` | adapter | one HTTP client, streaming, with its timeouts | the loop, the session |
| `session-jsonl` | adapter | append-only storage for entries and patches | what an entry means |
| `console` | product | the buffer, rendering, batched redraw, the status line | the loop's internals |
| `tools-read`, `tools-edit`, `tools-shell` | product | one tool each, one unit each | each other |
| `undo` | product | change groups and the per-turn snapshot, wired to `C-/` | the storage format |

One unit, one file: `lisp/pearl-<unit>.el`, with the package prefix the suite
requires of published modules. Tests mirror them without one:
`test/<unit>-tests.el`. Test modules are free of the prefix on purpose (a test
is not published); the `-tests` suffix keeps a module's `provide` distinct from
the one in `lisp/`.

The right-hand column is the part worth arguing about in review, because it is
the part a require rule can enforce. A unit with an empty "may not know" column
has not been placed yet.

## Layers

Three layers, told apart by **dependency direction**, not by importance. Read
the picture as "is used by":

```mermaid
flowchart TB
  core["core<br/>loop, session, provider, tool, contract"]
  adapters["adapters<br/>provider-sse, session-jsonl"]
  product["product<br/>console, tools, undo"]
  core --> adapters --> product
```

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

```mermaid
flowchart LR
  ui["console"] -->|submit| loop[["loop"]]
  loop -->|emit| ev["events"]
  ev -.->|render| ui
  loop -->|stream| prov["provider"]
  loop -->|execute| tools["tools"]
  tools -->|patch| sess[("session")]
  ev -->|entry| sess
  sess -->|context| loop
```

- **Control ring** `console -> loop -> events -> console`: who is watching the
  agent work.
- **Memory ring** `loop -> tools -> session <- events`, then `session -> loop`:
  what changed, what was recorded, what is read next time.

An architecture diagram of an agent with no ring in it is almost always wrong,
because the job is exactly "look, change, record, look again".

The two rings are also where PEARL differs most from Pi, and the difference is
invisible in an arrow. Pi's loop emits `events` and never reads a session: the
product subscribes, appends, and replays the branch into the next request, so
the ring leaves the core and comes back through a layer above it. Here the
session is in the core, so the ring closes inside one layer — and a guarantee
that no layer above can forget is not a convention.

## Undo

Undo is layered, and the layers are used in this order:

| layer | mechanism | lives in | survives |
| --- | --- | --- | --- |
| one tool call | an Emacs change group, amalgamated into one `undo` step | the tool, at execution time | until the buffer is killed |
| one turn, on disk | a tree snapshot taken before the turn | the session adapter | a restart |
| one entry, durable | the patch the entry carries | the core | a restart, and a second consumer |

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

What this does not cover is in [decisions.md](decisions.md): commands, their
side effects, and why the answer is confirmation rather than a sandbox.

## Compaction

Compaction belongs to the read side: it replaces what the model reads with a
summary, and the tree keeps every node. It is the one deferred feature whose
seam already exists — it hangs off the projection that builds `context` — and
adding it must not touch entries or patches.

## Related

- [decisions.md](decisions.md) — the differences from Pi, each with its price
- [glossary.md](glossary.md) — one word, one meaning
- [roadmap.md](roadmap.md) — what exists today
