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

Two diagrams, one skeleton: the application surface on top, the core below, one
rank of providers at the bottom. Read them side by side; the difference is a
single node changing layers.

Shapes carry the meaning, colour only decorates it: double border marks the
core, a plain box is kept, a hexagon is added or moved into the core, a capsule
is replaced, and a dashed box names what was cut.

**Pi today**

```mermaid
flowchart TB
  subgraph core["CORE"]
    loop[["agent loop<br/>double + queues"]]
    model["model in<br/>messages, tools, stream"]
  end
  subgraph app["APP"]
    sess[("session<br/>JSONL tree")]
    tools["tools<br/>read, edit, bash"]
    ext["extensions<br/>registry"]
    surface["surface<br/>tui, rpc, print"]
  end
  prov["providers<br/>11 adapters"]
  surface -->|submit| loop
  loop -->|stream| model
  loop -->|execute| tools
  loop -.->|events| sess
  ext -.->|register| tools
  prov -->|implements| model
```

Session is a leaf here: everything flows into it and nothing flows back. The
memory ring stops at the CORE/APP boundary, which is why a second consumer of
the runtime has to reimplement persistence rather than reuse it.

**PEARL**

```mermaid
flowchart TB
  subgraph core["CORE"]
    loop[["agent loop<br/>double + queues"]]
    model(["provider interface"])
    sess{{"session<br/>entries + patches"}}
    plan{{"rollback plan"}}
  end
  subgraph app["APP"]
    tools["tools<br/>read, edit, bash"]
    surface(["console"])
    cut["cut: registry, rpc<br/>print, json, mcp"]
  end
  prov(["one sse provider"])
  surface -->|submit| loop
  loop -->|stream| model
  loop -->|execute| tools
  loop -.->|events| sess
  sess -->|context| loop
  sess --> plan
  prov -->|implements| model
  classDef keep fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
  classDef add fill:#e0f2f1,stroke:#00796b,color:#004d40,stroke-width:3px
  classDef repl fill:#fff8e1,stroke:#ef6c00,color:#e65100
  classDef cut fill:#f2f2f2,stroke:#9e9e9e,color:#757575,stroke-dasharray:5 4
  class tools keep
  class sess,plan add
  class model,surface,prov repl
  class cut cut
```

Session moves into the core and the ring closes there: `loop -> events ->
session -> context -> loop`. That closure is the whole argument. An edit can
only be called revertible if the thing that guarantees it also records it,
because a guarantee a layer above can forget is a convention.

Why each of those decisions costs what it costs, and what would overturn it, is
in [docs/differences.md](docs/differences.md).

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
