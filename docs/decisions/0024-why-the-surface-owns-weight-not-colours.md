# 24. Why the surface owns weight, not colours

Date: 2026-11-25

## Status

Amended by [ADR 44](0044-why-an-example-underlines-the-part-the-human-types.md)

## Context

PEARL runs inside the human's terminal. Its palette is theirs: a named
foreground like `Cyan` or `Color::White` is a request for one of sixteen
colours the terminal maps to whatever that theme says. On a theme whose
brightest white is cream, a white topic reads as grey and lands dimmer than
the body it heads; on a theme that paints its background cyan, a cyan rule is
invisible. Both happened in this repository: a topic drawn in `Color::White`
was reported as the answer's titles coming out grey, and the same class of guess
had already coloured a failed badge red on a theme whose errors are orange.

## Options considered

1. **Read the palette from the terminal.** OSC 4/10/11 queries can ask a
   terminal for its colours, but the answer is asynchronous, not every terminal
   answers, multiplexers and SSH terminals rewrite it, and a surface that has to
   wait for a reply before it can draw its first frame is not one that starts
   instantly. It also buys nothing: an attribute already derives from the
   colours the human chose.
2. **Ship a palette of our own** — pick colours that "work anywhere" (256-colour
   cube values, a fixed theme). This is the same guess with more care, and it
   fails the same way: the terminal still renders those indices through its
   own map.
3. **Own no colour at all** — weight, dimness and reverse, which every terminal
   derives from its own foreground and background, plus glyphs and words for
   meaning.

## Decision

The surface names no colour. `crates/pearl-tui/src/style.rs` owns three styles
and the whole crate draws through them: `dim()` for furniture (rules, hints,
keys, the chrome around the work), `strong()` for emphasis (a topic, a notice,
the badge of a task that wants a look), `marked()` for the cursor's row and for
inline code, both rendered in the terminal's own two colours swapped.

Meaning that would once have been a colour is carried by a glyph or by the
words already present: `▶ ✓ ✗ ‖ ⊘` say which state a task is in, and the notice
line says what happened. A state that needed a colour to be readable is a state
that needs a better glyph, and that is the work.

A story keeps the rule honest: `given_every_screen_when_it_is_drawn_then_no_cell_carries_a_colour_of_ours`
walks every cell of a busy surface — queue with running, done and failed tasks,
a notice, a report, a picker and the help — and fails if any `fg` or `bg` is
anything but `Reset`.

## Consequences

The surface is legible in a palette we have never seen, including a
black-and-white terminal, and it cannot drift back: a colour sneaks in once and
the cell story fails. Emphasis has to be spent carefully, because weight and
dimness are fewer tools than colour and hue: three levels of meaning is the
budget, and a fourth asks for a glyph instead. Reverse is kept for exactly one
job, a `Failed` badge: it is the terminal's own two colours swapped, not a
background we chose, and the state that must not be mistaken for `Done` is worth
the one cell where the rule bends.

Cost: a design that once used hue to separate "running" from "paused" now
separates them with a glyph and a weight, which is one more thing to read on a
row; and a human who wants colour cannot get it from PEARL, because a
configuration knob for a palette would be the same guess with an escape hatch.