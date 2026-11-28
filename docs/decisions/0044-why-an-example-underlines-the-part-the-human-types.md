# 44. Why an example underlines the part the human types

Date: 2026-11-28

## Status

Accepted

## Context

ADR 24 allows three attributes — dim, bold, reverse — and no colour, because a
colour is a guess about a palette PEARL has never seen. An address example is a
new case: it must show a whole URL and, inside it, the substring the human is
being asked for. Weight alone marks the base but does not separate it from the
furniture around it when the whole line is already bold in a terminal that
draws bold dimly; the eye reads the line, not the substring.

## Options considered

1. **Bold only** (ADR 24 as written). Legible in most terminals, and the
   substring is not separable in all of them.
2. **Reverse video.** Separates the base strongly, and reverses the reading of
   the line: the eye lands on a block, which reads as a selection rather than
   as "this part is yours".
3. **Bold and underlined.** The underline is drawn by the terminal from its own
   foreground, so it is not a colour and survives the same palettes ADR 24 was
   written for; together with weight it says "this part" without a block.

## Decision

`style::typed()` is a fourth attribute pair, bold and underlined, and it is
used only where an example contains the part a human must produce. It is not a
general emphasis: `style::strong()` remains the one style for what matters.

## Consequences

The three-style rule becomes "no colours, and four attributes, each with one
meaning". A surface that later needs a fifth has to amend this record rather
than invent a style beside it.

Cost: one more attribute to keep meaningful, and a terminal that renders
underline as a low line can make a URL look like a link; the example is still
correct without it, which is why the base also carries weight.
