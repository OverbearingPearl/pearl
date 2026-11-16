# PEARL

**P**i · **E**macs · **A**gent · **R**uns · **L**isp

> Pi's agent loop, reborn in Emacs Lisp.

PEARL is a Lisp-native reimplementation of [Pi](https://github.com/earendil-works/pi)'s
agent runtime for Emacs. It borrows Pi's defining structure — a minimal, model-agnostic
loop that calls tools and feeds results back — and expresses it in Emacs Lisp.

No Node. No npm. Just Emacs, a loop, and `eval`.

---

## Why Lisp?

Pi's core is deliberately small. `pi-agent-core` is described as a "small
provider-agnostic agent loop," and the project's own framing is that Pi keeps
"only the agent loop, tools, and providers" — everything else is left to
extensions. That design already leans Lisp-ward: a small
executable core, with behavior extended by redefinition rather than configuration.

Emacs Lisp makes the rest literal:

- **The loop is already an Emacs idiom.** Emacs has a native event loop, native
  queues, and native `condition-case`. An agent loop does not need a bridge to
  another runtime.
- **Extensibility is the platform.** Extensions in Emacs are packages that advise,
  override, or wrap functions. An agent that expects to be extended this way does
  not need a plugin registry.
- **There is precedent.** `macher-agent` is an "Emacs-native LLM agent harness"
  with asynchronous sub-agent orchestration. `magent` implements its
  agent loop, tool calls, permissions, sessions, and child agents in Emacs Lisp,
  using `gptel` only for model requests. PEARL is not proposing a new
  possibility; it is applying an existing pattern to Pi's specific loop architecture.

---

## What PEARL keeps, adds and drops

One picture, two halves, each read top-down. The left half is Pi's real
architecture. The right half is not PEARL's own structure; it is the same picture
after the deltas, node for node, so the two can be compared by scanning.

<!-- Mermaid draws subgraphs in reverse declaration order, and a cross-subgraph
     edge overrides `direction` inside one. Both panels declare `direction TB`
     and keep no edges between them, so they stay vertical and side by side.
     Declaring PEARL first is what puts PI on the left.

     Left-to-right order inside a rank is dagre's call, not the source order:
     `providers` always lands next to `model interface`, so the two panels
     declare the loop's `execute` and `stream` edges in opposite orders to get
     `providers` on the left in both. A `~~~` hidden link cannot do this: it is
     still a layout edge, so it pushes one node down a rank and the shared row
     collapses. -->

```mermaid
flowchart TB
  subgraph R["PEARL - the same picture, after the deltas"]
    direction TB
    r_surf["surface<br/>console"] -->|submit| r_loop[["agent loop<br/>double + queues"]]
    r_loop -->|stream| r_model["model interface"]
    r_loop -->|execute| r_tools["tools"]
    r_loop -.->|events| r_sess["session<br/>entries + patches"]
    r_tools -->|patch| r_sess
    r_sess -->|context| r_loop
    r_sess --> r_plan{{"rollback plan"}}
    r_ext["extensions<br/>advice"] ==>|advice| r_tools
    r_prov["providers<br/>one sse"] -->|implements| r_model
  end
  subgraph L["PI today"]
    direction TB
    l_surf["surface<br/>tui, rpc, print, json"] -->|submit| l_loop[["agent loop<br/>double + queues"]]
    l_loop -->|execute| l_tools["tools"]
    l_loop -->|stream| l_model["model interface"]
    l_loop -.->|events| l_sess[("session<br/>JSONL tree")]
    l_tools -.->|append| l_sess
    l_sess -.->|context| l_loop
    l_ext["extensions<br/>registry"] ==>|registry| l_tools
    l_prov["providers<br/>one per api"] -->|implements| l_model
  end
  classDef keep fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
  classDef moved fill:#e0f2f1,stroke:#00796b,color:#004d40,stroke-width:3px
  classDef added fill:#e3f2fd,stroke:#1565c0,color:#0d47a1
  classDef changed fill:#fff8e1,stroke:#ef6c00,color:#e65100
  class l_loop,l_model,l_sess,r_loop,r_model keep
  class l_surf,l_ext,l_prov,l_tools changed
  class r_surf,r_ext,r_prov,r_tools changed
  class r_sess moved
  class r_plan added
```

Colour is the only mark whose meaning is relative to the other half. A shape or
a line style means the same thing on both sides:

| mark | meaning |
| --- | --- |
| ![green node](docs/images/legend/green.svg) | this half keeps it: the module, its seam, its job |
| ![amber node](docs/images/legend/amber.svg) | this pair changed in place: four surfaces become one console, a registry becomes advice, adapters per api become one, tools take an intent instead of a position |
| ![teal node with a heavy border](docs/images/legend/teal.svg) | this pair moved: the shape of the session leaves the application layer and enters the core |
| ![blue node](docs/images/legend/blue.svg) | this is new: the rollback plan |
| ![box](docs/images/legend/box.svg) | a module, or the record the core owns (`["…"]`) |
| ![cylinder](docs/images/legend/cylinder.svg) | a store that owns a file (`[("…")]`) |
| ![hexagon](docs/images/legend/hexagon.svg) | an artifact derived from the records, not a module (`{{"…"}}`) |
| ![box flanked by two vertical bars](docs/images/legend/subroutine.svg) | a control structure, not a module (`[["…"]]`) |
| ![solid arrow](docs/images/legend/solid.svg) | data the receiver uses, handed over by the tail, in the same step (`-->`) |
| ![dashed arrow](docs/images/legend/dashed.svg) | the tail does not complete it: either nobody waits for it (`events`), or a layer this panel does not draw does it (`context` and `append` on the left) (`-.->`) |
| ![thick arrow](docs/images/legend/thick.svg) | changes behaviour instead of passing data: `advice`, `registry` (`==>`) |

The bracket in each row is the Mermaid spelling, so the legend, the block above
and the rendered figure all agree on one vocabulary.

Layers are named in [docs/architecture.md#layers](docs/architecture.md#layers)
and are not boxed here; the figure makes one claim, and it is about who records
and who reads back, not about where the line between the layers falls.

Dropped, and therefore absent from the right half: RPC, the print and JSON
modes, MCP, codemode, compaction.

Both halves draw `events` out and `context` back, and the difference is where
they run. `events` is dashed in both: the loop emits and never waits. `context`
comes back in both, and the line style is the whole point.

On the left it is dashed, because the read path does run through the
application layer: the product subscribes to events,
appends entries, and replays the branch into the next request. Nothing in the
core knows a session exists, which is why a second consumer of the runtime has
to rebuild persistence rather than reuse it.

On the right it is solid, and so is the edge that feeds it: `patch` runs from
the tool result to the session inside the core, in the same step that promises
revertibility. The left half records the same thing, `append`, but through the
product, which is why that one stays dashed. Both halves keep the tool result;
the difference is who writes it down.

That is the whole shape of the argument. The ring closes inside one band on the
right: the core records and the core reads back. A guarantee a layer above has
to remember to keep is a convention; one the core performs itself is not.

What moves between the halves is the shape of the record, not the file. The core
owns entries and patches, so it is drawn as a box on the right; writing them to
JSONL is an adapter and is not in the picture. The left half keeps a cylinder
because Pi's session manager really does own the file.

One delta has no node of its own. PEARL's tools do not take positions: a tool
takes an intent, a locator resolves it to a range, and the edit is bounded to
that range. That is the precondition for the patch above, so both `tools` boxes
are amber rather than green — the pair is the point, and the two boxes mean
different things even though they carry the same word. It is also the one place
PEARL deliberately does not follow Pi, so the argument lives in
[docs/differences.md#4-why-the-model-never-emits-positions](docs/differences.md#4-why-the-model-never-emits-positions)
and [docs/differences.md#5-safety-revertible-edits-confirmed-commands](docs/differences.md#5-safety-revertible-edits-confirmed-commands)
rather than in a node the left half could not match.

Why each decision costs what it costs, and what would overturn it, is in
[docs/differences.md](docs/differences.md). The layer rules the picture follows
are in [docs/architecture.md#layers](docs/architecture.md#layers).

## Relationship to Pi

PEARL is **not** an official Pi project, and it is **not** a fork.

It is a translation: same control-flow architecture, different language. PEARL
borrows Pi's nested loop structure and its two future-message channels — steering
and follow-up — and expresses them in Emacs Lisp. APIs, data structures, and
extension points will diverge where Lisp offers a better idiom.

Pi is MIT-licensed, originally created by Mario Zechner and now maintained under
`earendil-works/pi`. PEARL is released under the same MIT terms.

Where PEARL deliberately differs from Pi, and why →
[docs/differences.md](docs/differences.md).

### Prior art

Two claims here — that code should be located by tooling rather than by the
model, and that an applied edit has to be revertible — come from an earlier
Emacs project by the same author. None of its code is part of PEARL; the
argument for both claims is made in
[docs/differences.md](docs/differences.md).

---

## Architecture (Target)

> **What this section is:** the loop structure PEARL intends to implement, based on
> Pi's `pi-agent-core`. **What this section is not:** a description of finished code.
> The concrete Lisp realization may adapt Pi's control flow to Emacs's async model,
> but the ordering described here is the target.

Pi's agent runtime is a **double nested loop**, implemented in `runLoop` inside
`pi-agent-core`.

<pre>
    ● agent-start
         │
         ▼
╔══════════════════════════════════════════════╗
║  OUTER LOOP                                  ║
║                                              ║
║   ┌──────────────────────────────────────┐   ║
║   │  INNER LOOP                          │   ║
║   │                                      │   ║
║   │   ▸ stream model response            │   ║
║   │   ▸ parse tool calls                 │   ║
║   │   ▸ execute tools                    │   ║
║   │   ▸ inject results                   │   ║
║   │   ▸ consume steering messages        │   ║
║   │                                      │   ║
║   │   ↻  repeat while tool calls or      │   ║
║   │      steering messages remain        │   ║
║   └──────────────────────────────────────┘   ║
║                                              ║
║   follow-up messages queued?                 ║
║     ├── yes ──▶  re-enter INNER LOOP         ║
║     └── no  ──▶  agent ends                  ║
╚══════════════════════════════════════════════╝
         │
         ▼
    ● agent-end
</pre>

**Steering** messages are delivered after the current assistant turn finishes its
active tool calls and before the next model request. They let a caller intervene
mid-run without aborting the run.

**Follow-up** messages are delivered after the agent "would otherwise become idle" —
after the inner loop settles, as a new outer-loop iteration. They let a caller queue
additional work without starting a fresh agent run.

PEARL's job is to express this ordering with Emacs primitives: `condition-case` for
turn-level error handling, a queue for each message channel, and generic functions
for tool dispatch.

---

## Design Principles (Intended)

These are constraints on PEARL's architecture, not claims about current implementation.

1. **The loop is the core.** The outer/inner loop ordering is the product. Tools,
   model adapters, and UI are replaceable functions around it.
2. **No hidden state.** Message queues and session state should be inspectable
   Emacs data (lists, buffers, plists), not private variables inside closures.
3. **Extend by redefinition.** `advice-add`, `cl-defmethod`, and function
   redefinition are the extension surface — not a JSON configuration layer.
4. **Stay small.** If a feature can live outside the loop, it does.

---

## Development

The check suite and the CI pipeline are described in
[`ci/README.md`](ci/README.md). In short:

Layout: `pearl.el` is the only `.el` at the repository root, business modules
live in `lisp/`, tests in `test/`, design notes in `docs/`.

```sh
make lint    # checkdoc, package-lint, byte-compile, format, headers, layering
make test    # ERT over test/*.el
```

`make lint` is byte-for-byte what CI runs, so a green local run means a green
`lint` job. Only MELPA's own tool (melpazoid) runs in CI alone, and it runs
on demand rather than on every push; `ci/README.md` explains why.

## License

MIT. See [`LICENSE`](LICENSE); the copyright holder is OverbearingPearl.

---

## A Note on Pi

PEARL exists because Pi got one thing right: **the agent is the loop.** Not the UI,
not the plugin system, not the model provider list — the loop. Pi's authors had the
discipline to strip the architecture down to that core and the generosity to license
it MIT, which is what makes a rewrite like this possible without asking permission.

This project is a translation, not a competitor. It borrows Pi's loop ordering and
message-channel model and gives them a Lisp accent. If PEARL is ever useful, it is
because Pi was small enough to be understood and open enough to be rebuilt.

— *The PEARL contributors*
