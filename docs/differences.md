# Where PEARL differs from Pi

PEARL is a translation of Pi's agent runtime into Emacs Lisp. Everything the
two agree on is in [architecture.md](architecture.md); this document is the
list of places where they do not agree, and why.

Each section answers one question and carries three fields, in this order:

- **Pi** — what the upstream source does, with the file it happens in. Facts,
  so they can be checked again later.
- **PEARL** — what this package does, or `planned` when it does not exist yet.
  Plans are never written in the present tense.
- **Why, and what it costs** — the motive and the price. A choice with no
  price is not a choice.

Two optional fields appear where they apply: **What would change our mind**,
and **See** links into the rest of `docs/`.

Status legend: `done` exists and is tested, `planned` is decided but unwritten,
`open` is not decided.

---

## 1. Why Emacs-native, and not an Emacs front end to a Node agent

**Pi.** The agent runtime runs on Node (>= 22.19). The loop, tools and
providers are TypeScript modules; interfaces (interactive, print, JSON, RPC,
SDK) are all in-process callers of the same runtime
(`packages/coding-agent/src/modes/`).

**PEARL.** `planned`. The loop is Emacs Lisp and the model stream is read
inside Emacs.

**Why, and what it costs.** The loop is control flow, and Emacs already owns
control flow: a native event loop, native queues, `condition-case`, buffers as
first-class context. A bridge to a foreign runtime puts a process boundary in
the middle of the one thing this package is about, and every feature after that
has to answer "which side of the boundary does this live on".

The cost is real: no npm ecosystem (no provider matrix, no MCP client, no
existing test fixtures), and Emacs Lisp is single-threaded, so streaming and
tool execution must be arranged around that instead of relying on promises.

**What would change our mind.** If a required capability turns out to be
unavailable inside Emacs at all — not inconvenient, unavailable — the boundary
moves. Nothing so far is in that category.

**See** [glossary.md](glossary.md), [layers.md](layers.md).

---

## 2. Why the loop stays double

**Pi.** Two nested loops, spelled out in
`packages/agent/src/agent-loop.ts`: the outer `while (true)` at line 187 and
the inner `while (hasMoreToolCalls || pendingMessages.length > 0)` at line 192.
Follow-up messages are read only after the inner loop exits (line 311), and
they re-enter it.

**PEARL.** `planned`. Same two loops, same two conditions.

**Why, and what it costs.** The two message channels are distinguished by
*which loop condition they feed*, not by a flag:

| channel | condition | arrives |
| --- | --- | --- |
| steering | inner loop | after the current turn's tool calls, before the next request |
| follow-up | outer loop | when the agent would otherwise stop |

A single loop with one queue cannot express "this message must not interrupt
the running turn" and "this message must wait until the agent is idle" at the
same time; it can only pick one, or add an ordering rule that every caller then
has to respect.

The cost is that the loop's shape is not free to change: the two conditions are
the specification, and the tests have to pin them rather than the outcome.

**What would change our mind.** A test that shows both channels behaving
identically in a real workflow — then one of them is dead weight.

**See** [architecture.md#the-two-rings](architecture.md#the-two-rings).

---

## 3. Why the session lives in the core

**Pi.** `pi-agent-core` is stateless: `Agent` holds messages in memory
(`packages/agent/src/agent.ts`), and the only durable thing it knows about is a
`sessionId` forwarded to providers for cache-aware backends. Persistence lives
in the product: `AgentSession` subscribes to agent events and appends entries
(`packages/coding-agent/src/core/agent-session.ts:548` subscribing,
`:1232` appending), and the entry tree is `SessionManager`
(`packages/coding-agent/src/core/session-manager.ts`, entries carry a
`parentId` at line 60).

**PEARL.** `planned`. Session is a core module next to the loop: entries, their
`parentId`, and the reverse patch each mutating entry carries. Storage is a
separate, replaceable module.

**Why, and what it costs.** The split above has a price, and Pi's own tree pays
it: `pi-durable` needs a recoverable agent, cannot reuse `AgentSession`
(which is bound to a working directory, settings, extensions and a buffer), and
therefore reimplements `session/` and `storage/` — and pulls in `chord` to keep
document state (`packages/durable/package.json` depends on `chord`, not on
`pi-agent-core`). One consumer pays once; a second consumer pays again.

PEARL's differentiating claim is revertibility. That claim cannot be enforced
by convention across a layer boundary: if the loop hands a tool result to the
product and trusts the product to record an inverse operation, the guarantee is
a promise, not a property. Putting session in the core makes the patch and the
result one atomic step.

The cost: the core is bigger than Pi's (about six files there; more here), and
every consumer inherits the session model instead of choosing its own.

**See** [layers.md#why-the-session-split-is-not-a-compromise](layers.md#why-the-session-split-is-not-a-compromise),
[glossary.md#reverse-patch](glossary.md#reverse-patch).

---

## 4. Why the model never emits positions

**Pi.** The edit tool takes `oldText` and `newText` and matches the text
against the file (`packages/coding-agent/src/core/tools/edit.ts:18`, the schema
comment states it must be unique and non-overlapping). Locating code is the
model's job; the tool verifies afterwards.

**PEARL.** `planned`. Tools take an intent (a symbol, a region description)
and a locator resolves it to a byte range before anything is written.

**Why, and what it costs.** A wrong edit is a defect, not something a reviewer
is expected to catch. If the model supplies the text to match, correctness
depends on the model reproducing the file exactly, and the failure mode is
silent: a near-match lands in the wrong place or nowhere.

The cost: PEARL needs a locator layer (LSP, tree-sitter, structural scanning)
and a way to degrade when none of them can resolve. It also makes the tool
schemas less familiar to models trained on text-replacement tools.

**What would change our mind.** Evidence that a locator cannot resolve a
useful fraction of real edits, in which case PEARL would need a documented
fallback rather than pretending the locator always wins.

**See** [glossary.md#locator](glossary.md#locator),
[glossary.md#bounded-edit](glossary.md#bounded-edit).

---

## 5. Safety: revertible edits, confirmed commands

**Pi.** No permission system. The README states it: tools and extensions run
with the operating system permissions of the Pi process, and isolation is
delegated to containers (`packages/coding-agent/docs/containerization.md`).

**PEARL.** `planned`. File edits are applied without asking, because they land
on a resolved, verified range and are revertible as a whole; commands always
ask, because their effects are outside anything a reverse patch can undo.

**Why, and what it costs.** The two mechanisms protect different things.
Pi's position answers "what may this process touch" and hands the answer to the
environment. PEARL answers "can this change be taken back" and keeps it inside
the agent. An editor-resident agent has the second problem more often than the
first.

The cost is a promise that must be kept: "revertible" only covers file edits.
It says nothing about a shell command's side effects, which is exactly why
commands are confirmed rather than reverted.

**See** [glossary.md#revertible](glossary.md#revertible).

---

## 6. Why the extension surface is advice, not a registry

**Pi.** Extensions register tools, commands, event handlers, renderers and
providers through an API object (`packages/coding-agent/docs/extensions.md`:
`pi.registerTool`, `pi.on`, `pi.registerCommand`, `ctx.ui.custom`), and are
distributed as npm or git packages.

**PEARL.** `planned`. `advice-add`, `cl-defmethod` and plain redefinition.
No registry, no manifest, no discovery step.

**Why, and what it costs.** A registry is a protocol that both sides must keep
implementing; in Emacs the protocol already exists and every user knows it.
A registry also creates a second place where behaviour is defined, and the loop
stops being readable on its own.

The cost: no discovery (the user loads a file), no versioned hook contract, and
advice can break in ways a typed API would have caught.

**See** [layers.md](layers.md).

---

## 7. Why one provider interface and not eleven adapters

**Pi.** One module per API family under `packages/ai/src/api/`
(`anthropic-messages`, `openai-responses`, `openai-completions`,
`google-generative-ai`, `google-vertex`, `bedrock-converse-stream`,
`mistral-conversations`, `cloudflare`, `llama-cpp`, and the lazy variants),
plus a generated model catalog and an auth layer.

**PEARL.** `planned`. One interface (`pearl-provider`) with two
implementations: a small SSE client, and a fake provider used by tests. A
second real provider comes only when someone needs it.

**Why, and what it costs.** Most of Pi's provider surface is data and
per-vendor quirks — message placement, streaming tool-call deltas, stop
reasons, reasoning fields, the catalog — not agent semantics. Reproducing that
in Emacs means re-discovering eleven vendors' edge cases and tracking their
changes forever, for a package whose claim is about the loop.

The cost: models behind providers we do not implement cannot be used, and
users who already have credentials configured elsewhere must configure them
again.

**What would change our mind.** A second consumer that needs a provider we do
not have, twice.

**See** [glossary.md#provider](glossary.md#provider).

---

## 8. What PEARL deliberately does not build

**Pi.** Also has: MCP client, a JavaScript sandbox (`codemode`), compaction,
four interface modes, skills, prompt templates, themes, packages, a model
catalog, and a TUI library.

**PEARL.** `planned`: none of it, at least not before the loop, the session and
one real edit path work.

**Why, and what it costs.** Every one of those is a surface with its own
maintenance, and each one answers a question PEARL has not yet answered.
Compaction is the clearest example: it changes what the model reads, so it
depends on decisions about `parentId` traversal and patches that do not exist
yet.

The cost: PEARL will feel smaller than Pi for a long time, and some users will
leave for that reason. They are not the users this package is for yet.

**See** [architecture.md#the-four-rules](architecture.md#the-four-rules).

---

## Index

| Document | Answers |
| --- | --- |
| [architecture.md](architecture.md) | how the parts fit: two rings, the memory ring, four rules |
| [layers.md](layers.md) | what core, adapters and product mean here, and how to tell |
| [glossary.md](glossary.md) | one word, one meaning, and what it replaces |
