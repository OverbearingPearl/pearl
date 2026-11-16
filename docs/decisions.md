# Where PEARL differs from Pi

PEARL translates Pi's **loop** and builds a different product around it. This
document is the list of places where the two do not agree, and what each
disagreement costs. What is copied as is gets one table near the end and no
argument: repeating a decision you agree with spends the reader's attention on
nothing.

Each section answers one question and carries three fields, in this order:

- **Pi** — what the upstream source does, with the file it happens in.
- **PEARL** — what this package does instead.
- **Why, and what it costs** — the motive and the price. A choice with no price
  is not a choice.

Two optional fields appear where they apply: **What would change our mind**, and
**See** links into the rest of `docs/`.

Which of these exist in the code right now is [roadmap.md](roadmap.md)'s
business, not this document's.

---

## 1. Why Emacs-native, and not an Emacs front end to a Node agent

**Pi.** The runtime runs on Node (>= 22.19): the loop, tools and providers are
TypeScript modules, and every interface is an in-process caller of the same
runtime (`packages/coding-agent/src/modes/`).

**PEARL.** The loop is Emacs Lisp and the model stream is read inside Emacs.

**Why, and what it costs.** Being in the process buys exactly three things.
**Edits join your undo history**: a tool call becomes one undo step in the buffer
it edited, so the user's `C-/` reverts it, which a bridged agent cannot do.
**Unsaved buffers are context**: no save, no IPC. And the **editor's vocabulary**
is available: `compilation-mode`, `next-error`, `xref`, `diff-mode`, instead of
printing text.

The price: one thread, so slow Elisp freezes the whole editor rather than one
agent; no isolation, so sandboxing is a per-platform story rather than a
container; and no npm, so anything that only exists as a Node SDK is out of
reach.

**What would change our mind.** An implementation that only uses the in-process
part, and none of the three, would be better off bridged.

**See** [architecture.md#layers](architecture.md#layers).

---

## 2. Why the session lives in the core

**Pi.** `pi-agent-core` is stateless: `Agent` holds messages in memory
(`packages/agent/src/agent.ts`), and the only durable thing it knows is a
`sessionId` forwarded to providers. Persistence lives in the product:
`AgentSession` subscribes to events and appends entries
(`packages/coding-agent/src/core/agent-session.ts:548`, `:1232`), and the entry
tree is `SessionManager` (`packages/coding-agent/src/core/session-manager.ts`,
entries carry a `parentId` at line 60).

**PEARL.** Session is a core module next to the loop: entries, their `parentId`,
and the optional patch each mutating entry carries. Storage is a separate,
replaceable module.

**Why, and what it costs.** The split above has a price, and Pi's own tree pays
it: `pi-durable` needs a recoverable agent, cannot reuse `AgentSession` (which is
bound to a working directory, settings, extensions and a buffer), and therefore
reimplements `session/` and `storage/` — and pulls in `chord` to keep document
state. One consumer pays once; the second pays again.

PEARL's claim is that an applied change can be taken back. That claim cannot be
enforced across a layer boundary: if the loop hands a tool result to the product
and trusts the product to record what changed, the guarantee is a promise rather
than a property. In the core, the patch and the result are one step.

The cost: the core is bigger than Pi's, and every consumer inherits the session
model instead of choosing its own.

**See** [glossary.md#patch](glossary.md#patch),
[architecture.md#the-memory-ring](architecture.md#the-memory-ring).

---

## 3. Why the edit path is anchored by text

**Pi.** The edit tool takes `oldText` and `newText` and matches the text against
the file (`packages/coding-agent/src/core/tools/edit.ts:18`; the schema requires
a unique, non-overlapping match). Locating code is the model's job; the match is
what proves the model looked.

**PEARL.** The same shape: a unique anchor plus its replacement, several
disjoint edits in one call.

**Why, and what it costs.** The anchor is not the poor cousin of a locator; it is
the only mechanical proof that the model has seen the text it is changing. A
locator answers "does this symbol exist", which is weaker than "is this the text
you read", and the common failure is not a missing symbol but the right symbol
edited wrongly, which no locator catches.

The alternative was to resolve intents to ranges first, with LSP or tree-sitter.
Rejected: it costs a per-language layer that has to be present, current and
parseable; it fails differently per language; and it buys only bulk mechanical
renames, which if they ever earn their keep are a separate tool rather than the
default edit path.

The cost of the anchor is that a stale one fails loudly and takes a round trip,
and that there is no deterministic rename across many files.

**What would change our mind.** A rename-shaped task that keeps recurring, which
would argue for a second tool, not for a different default.

---

## 4. Safety: revertible edits, confirmed commands

**Pi.** No permission system. Tools and extensions run with the operating system
permissions of the Pi process, and isolation is delegated to containers
(`packages/coding-agent/docs/containerization.md`).

**PEARL.** Three layers, in the order they are relied on:

| layer | mechanism | covers |
| --- | --- | --- |
| edits | Emacs change groups, amalgamated into one undo step per tool call | anything that goes through a buffer |
| files | a tree snapshot per turn, and a conflict check per file before restoring | what is on disk, including what a command wrote |
| commands | a write scope limited to the project root; confirmation when that cannot be established | what a command may touch |

**Why, and what it costs.** None of the three is a sandbox: this stops accidents,
not adversaries. Pi's answer — "we do not police it, containerise it" — is a
distribution answer for a CLI and does not transfer to an editor plugin, which is
the user's own Emacs process with the user's own credentials.

A read-only sandbox would be stronger and is wrong here, because builds and tests
must write. Per-platform kernel sandboxing is possible and not worth it: three
mechanisms that each rot (Seatbelt is deprecated, Linux user namespaces are
restricted per distribution, Windows has none), all to stop an accident that undo
already stops.

**What would change our mind.** A user-facing requirement for adversarial
isolation, which is a container's job, not this package's.

**See** [glossary.md#revertible](glossary.md#revertible).

---

## 5. Why the extension surface is advice, not a registry

**Pi.** Extensions register tools, commands, event handlers, renderers and
providers through an API object (`packages/coding-agent/docs/extensions.md`:
`pi.registerTool`, `pi.on`, `pi.registerCommand`), and are distributed as npm or
git packages.

**PEARL.** `advice-add`, `cl-defmethod` and plain redefinition. No registry, no
manifest, no discovery step. Three named seams exist because they are data, not
behaviour: the tool list, the prompt function, and the console mode hook.

**Why, and what it costs.** A registry is a protocol both sides must keep
implementing; in Emacs that protocol already exists and every user knows it. A
registry also creates a second place where behaviour is defined, and the loop
stops being readable on its own.

The cost: no discovery, no versioned hook contract, and advice can break in ways
a typed API would have caught.

---

## 6. Why one provider interface and not one adapter per API

**Pi.** One module per API family under `packages/ai/src/api/`
(`anthropic-messages`, `openai-responses`, `openai-completions`,
`google-generative-ai`, `google-vertex`, `bedrock-converse-stream`,
`mistral-conversations`, `cloudflare`, `llama-cpp`, and their lazy variants),
plus a generated model catalog and an auth layer.

**PEARL.** One interface, two implementations: a small SSE client and a fake one
that tests use. A second real provider arrives when a second one is needed.

**Why, and what it costs.** Most of Pi's provider surface is data and per-vendor
quirks — message placement, streaming tool-call deltas, stop reasons, reasoning
fields, the catalog — not agent semantics. Reproducing that means re-discovering
eleven vendors' edge cases and tracking their changes forever, for a package
whose claim is about the loop.

The cost: models behind providers we do not implement cannot be used, and users
with credentials configured elsewhere must configure them again.

**What would change our mind.** A second consumer that needs a provider we do not
have, twice.

---

## 7. What we take verbatim, and stop arguing about

Everything below is copied from Pi without a design of our own. It is listed so
that a port can be checked against it, not to justify it. If one of these turns
out wrong for PEARL it moves up into a section of its own.

| taken as is | what it pins down |
| --- | --- |
| event names and their order | `agent_start`, `turn_start`, `message_{start,update,end}`, `tool_execution_{start,update,end}`, `turn_end`, `agent_end` |
| queue insertion points | steering joins the inner loop's condition; follow-up is read only after the inner loop exits |
| tool argument shape | `edits[]`, each with an `oldText` that must match uniquely and not overlap |
| read offsets | `read` takes `offset` and `limit`, and says which range it returned |
| failures as results | a tool that fails returns a result marked as an error, it does not throw |
| output bounding | head or tail windows with an explicit truncated marker |
| truncated replies | a `length` stop fails every tool call of that turn instead of running them |
| abort | stop the run, keep the tool results already produced, hand queued messages back |
| one conversion point | `AgentMessage` to `Message` happens in one function and nowhere else |
| validation | tool arguments are checked once, at the trust boundary |
| tool order | calls within one turn run in order, and their results keep that order |

---

## 8. What PEARL deliberately does not build

**Pi.** Also has: an MCP client, a JavaScript sandbox (`codemode`), compaction,
four interface modes, skills, prompt templates, themes, packages, a model
catalog, a TUI library, and a plugin registry.

**PEARL.** None of it. Each one is a surface with its own maintenance, and each
one answers a question this package has not asked. A locator is in the same
category: the edit path does not resolve symbols, and section 3 says why.

Compaction is the one deferred rather than refused: it changes what the model
reads, so it hangs off the projection function described in
[architecture.md](architecture.md) and can be added without touching the tree.

---

## Index

| Document | Answers |
| --- | --- |
| [architecture.md](architecture.md) | how the parts fit: two rings, four rules, three layers |
| [glossary.md](glossary.md) | one word, one meaning |
| [roadmap.md](roadmap.md) | what exists today, and the order the rest arrives in |
