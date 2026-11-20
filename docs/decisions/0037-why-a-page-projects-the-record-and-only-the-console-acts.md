# 37. Why a page projects the record and only the console acts

Date: 2026-11-20

## Status

Accepted

## Context

M12 added the second surface: `C-c C-p` draws the request in a buffer of its
own. The architecture page had been saying that the console was the one
interactive surface, and that sentence stopped being true the day the page
landed. Two questions came with it and neither had a written answer: what a
surface is allowed to do, and how a third one is added without becoming a
second place that writes.

The pieces were already there and the rule was not. `pearl-session-projection`
is pure, `pearl-context-sections` prices a projection without touching a
buffer, and the console's own renderer takes an event and returns sections. But
`context` sat in the `product` layer beside the console, and `product` may
require `product` — so a page that required the console, or the app, was a
require the suite would have accepted. [ADR 26](0026-why-the-console-owns-presentation-and-the-session-owns-none.md)
settled what a face and a fold are and that the session owns no presentation;
it did not settle how many buffers that presentation is drawn into.

Pi is no help here: it has one TUI surface and never had to decide, and its
renderers sit beside its tools, which [ADR 12](0012-layer-rules-are-checked-not-documented.md)
forbids in this package. Scalpel, as ADR 26 reads it, uses Emacs's vocabulary
and a deliberate absence of animation; whether a second surface may act is
still this package's question.

## Options considered

- One buffer with regions. Every extra page becomes view state the console has
  to keep in sync, and a fold in one region turns into a fact about another;
  the console's redisplay budget pays for text it does not otherwise draw.
- A page that performs the action it displays. Cheap to write, and it makes the
  page a second writer of the record, which
  [ADR 22](0022-why-a-session-file-has-one-writer.md) refuses by construction.
- No new layer: keep pages in `product` and rely on review to notice a page
  requiring the console. A rule that only a careful reader enforces is a rule
  that decays; ADR 12 exists because this package has already paid for that.
- A fourth layer, `page`, between `adapters` and `product`.

## Decision

A page may require core, adapters and another page, and nothing from the
product; the layer table decides that rather than a convention. A page draws a
projection of the record, computes its figures from the record, and is rebuilt
on demand, so it holds no state that can go stale.

Only the console acts. It is the one surface with an input: the only one that
submits, confirms, rolls back, compacts or moves a head. A page that shows
something worth acting on says where the action lives and leaves the reader to
do it in the console. The console may require a page; that is the one direction
between them.

## Consequences

- A new page is a module, a layer row and a key, and its story can be written
  before the console learns that key.
- The rule is checked, not remembered: a page requiring `pearl-app` or
  `pearl-console` fails `layer-order`, so the design survives a session that
  has not read this record.
- The console keeps its one buffer, its folds and its buffer-locals. Pages do
  not share them, so the redisplay cost M4 pinned does not move when a page
  is added.
- A page can only show what the record contains, which is the limit worth
  having: it is what stops a page from printing a number the user would trust
  over the truth.

Cost: a figure the record does not carry is a figure no page may print. A
provider's own token count is the first such figure, so showing it costs a new
entry kind, a decision about who counts, and a story proving the entry never
reaches the model.
