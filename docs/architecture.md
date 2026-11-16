# Architecture

How the parts fit. The differences from Pi are in [decisions.md](decisions.md);
what each layer may depend on is at the end of this page.

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

## The memory ring

Entries `e1` and `e2` are conversation; `e3` executed a tool and is the only
entry carrying a patch. `e4` is a second branch under the same parent.

```mermaid
flowchart TB
  e1["e1 user"]
  e2["e2 assistant + toolCall"]
  e3[("e3 tool result<br/>reverse patch")]
  e4["e4 alt branch"]
  rebuild(["rebuild context"])
  rb(["rollback"])
  comp{{"compaction<br/>open"}}
  e1 --> e2
  e2 --> e3
  e2 --> e4
  e3 --> rebuild
  e3 --> rb
  rebuild -.-> comp
```

## The four rules

**1. Only entries that changed something outside the session carry a patch.**
Conversational entries carry none. If every entry had to carry one, rollback
would start depending on the assumption that an empty patch is harmless.

**2. Rollback is path-scoped, never entry-scoped.** "Undo this step" always
means "undo the last write on the active path". Otherwise a parallel branch
forces the question "does that patch still apply here", which has no good
answer.

**3. Compaction attaches to the read side only.** `rebuild` is its only
consumer. The tree keeps every node. Implemented as "write a summary and delete
the old entries", the tree and the patches stop agreeing — this is the one thing
the memory diagram is warning about.

**4. An edit is anchored by text, and the anchor is the proof.** The model quotes
the text it is replacing and the tool matches it uniquely; a failed match is a
loud error, not a near-miss. The anchor is also what makes the reverse patch
verifiable: the same check that lets an edit in lets it out.

## How undo is layered

The four rules describe the durable record. The undo a user actually presses is
cheaper, and the two are deliberately separate:

| layer | mechanism | lives in |
| --- | --- | --- |
| one tool call | an Emacs change group, amalgamated into a single undo step | the tool, at execution time |
| one turn, on disk | a tree snapshot taken before the turn | the session adapter |
| per entry, durable | the patch on the entry | the core |

The first layer is why `C-/` works and why there is no undo command to learn. The
second survives a restart. The third is what a second consumer of the runtime
would need, and it is built only when something needs it.

## Layers

Three layers, told apart by **dependency direction**, not by importance. A
module is in the core because of what it may not depend on, not because it
matters more.

| | Core | Adapters | Product |
| --- | --- | --- | --- |
| May depend on | Emacs built-ins and its own data | core | core and adapters |
| Exists to provide | a guarantee | one job, replaceable | a usable surface |
| Tested by | ERT alone, no network, no buffer, no user | ERT with a fake at the other end | end to end |
| Replacing it means | replacing the project | swapping a plugin | swapping a shell |

The two tests that settle a placement argument:

> **Delete the module. Are the core's tests still green?**

Green means it was product or adapter. Red means it was core. Applied upstream:
delete Pi's product and its core tests still pass, so that boundary is drawn
correctly.

> **Does it need to know a file path, a buffer, or a provider's name?**

Yes is product or adapter. No, only the shape of entries and patches, is core.

### Why the session split is not a compromise

Session straddles two layers, deliberately:

| Half | Layer | Why |
| --- | --- | --- |
| entry order, `parentId`, patches, rollback atomicity | core | it is the guarantee the loop issues |
| writing entries to a file or a database | adapter | a storage decision, not a semantic one |

The core owns a data shape and its operations; an adapter owns a format. Split
that way, "session in the core" does not mean "IO in the core".

## Related

- [decisions.md](decisions.md) — six decisions and their costs
- [glossary.md](glossary.md) — one word, one meaning
- [roadmap.md](roadmap.md) — what exists today
