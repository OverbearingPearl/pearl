# 41. Why folding is the only word for a shorter context

Date: 2026-11-28

## Status

Accepted

## Context

Four words described one act: `fold` in the log, the glossary and the `Role`;
`compaction` in the module, ADR 7, ADR 16 and the architecture page;
`compact` as the runner's method; and `compress` wherever prose wanted a verb
that was not already on the page. A reader could not tell whether these were
one mechanism with four names or four mechanisms.

Two of the four also suggested the opposite of what happens. Nothing is
compressed in place and nothing is rewritten: a summary is appended and a
projection reads less of the log than it holds. `compaction` is not wrong -- it
is the ecosystem's word -- but it was the third name for one thing, and this
codebase had no second mechanism for it to distinguish.

The vocabulary rule this comes from is the same one that produced ADR 38, 39
and 40: a word from the ecosystem is kept where it is true and dropped where it
would import a claim PEARL does not make. Here it imported nothing false; it
was simply a duplicate.

## Options considered

**Keep `compaction` because it is the ecosystem's word.** Then `fold`, which is
the `Role` value already written into every log, becomes the private word for a
public concept, and the glossary has to define one in terms of the other.

**Split them: folding is the act, compaction is the policy.** A distinction
nobody keeps at two in the morning, and the code already used `fold` for both.

**Keep `compress` for the non-model shortening.** There is no such operation;
the only in-place-looking shortening is a projection of tool outputs, which
also folds nothing away.

## Decision

**Fold** is the verb, the act, the entry and the point:

- `crates/pearl-log/src/fold.rs` (was `compaction.rs`), `project`, `split`,
  `view`, `covered`, `fold_results`;
- `Role::Fold` (was `Role::Compaction`), `Entry::fold`, `Store::append_fold`,
  `Runner::fold` and `Runner::fold_if_needed` (was `compact`);
- `FoldPolicy`, `Settings::fold_at`, and the status line's `fold at`.

Two words survive beside it, and neither is the act: **projection** is the view
a request is built from, because the request reads it rather than performs it;
**summarized** is what the notice says on the screen, because a fold is what the
code did and a summary is what the human sees.

## Consequences

A log written by an older build carries `"role":"compaction"`, and a resumed
view does not recognise it -- the entry is read as an unknown role and left out
of the projection, so the conversation comes back without its summaries rather
than with a wrong one. The old file was not migrated, for the same reason the
log's name was not (ADR 39).

The glossary now carries one entry for the mechanism, and the architecture
page one heading, where there were three sentences naming it differently.

Cost: `"compaction"` in the JSONL becomes `"fold"`, so a log from before this
record is not resumable, and the word `summarized` on the screen is still a
synonym the reader has to map to `fold` -- kept because it describes the
outcome the human cares about rather than the operation.
