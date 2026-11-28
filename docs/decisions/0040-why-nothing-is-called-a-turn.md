# 40. Why nothing is called a turn

Date: 2026-11-28

## Status

Accepted

## Context

`turn` was three different things in this codebase at once:

- one message on the wire: `NOT_TURNS` listed the roles a request does not
  carry, and `entry.rs` said of thinking "it is not a turn";
- one round's cost: `cost::Turn { cost, added, note }`, the node's
  `turns: Vec<Turn>`, `turns_since` and `charge`;
- one iteration of the loop: `rounds > MAX_ROUNDS`, and the `Role::Bill`
  documentation, which said in the same breath that a bill itemises "the
  rounds".

The field name and its documentation had already parted ways: `Cost.turns`
was documented as "how many requests this covers". A rename that leaves the
most ambiguous word in the codebase is not a rename, and this one had already
misled a reader of the design page during the discussion that produced this
record.

`turn` in LLM code most often means one message, and `User`/`Assistant` sat in
the neighbouring file, so prose could not have blocked the misreading even if
everyone read the prose.

## Options considered

**Keep `turn`, delete `round`.** Zero code change, and it keeps the word that
had already been read wrong. The loop's bound would then be a turn count next
to a message count in the same struct.

**Keep both and document which is which.** The note is the defect: a reader
who needs it has already lost.

**`message` for all three.** A bill is not a message and a round is not a
message; collapsing them would make `Cost` count messages it never sent.

## Decision

One word per thing, and the three that carry the load are:

- **entry** -- a line in the log, which is `Entry` with a `Role`;
- **message** -- what `replay` puts on the wire, which is what the model saw;
- **round** -- one ask and the tools it asked for, which is the loop's unit and
  the thing the limit bounds (`MAX_ROUNDS`);
- **bill** -- that round's cost and its one-line note, `Bill`, written when the
  round is over, which is what `Role::Bill` records.

`cost::Turn` is `Bill`, `node.turns` is `node.bills`, `Cost.turns` is
`Cost.rounds`, `Cost::turn` is `Cost::round`, `NOT_TURNS` is `NOT_MESSAGES`,
and the screen prints `round 2` where it printed `turn 2`.

The money is not renamed to round: a round is the piece of work and a bill is
what it cost, and a field named `rounds` holding money would recreate the same
confusion one level down.

## Consequences

The report's line and the field that counts the lines finally say the same
word, and `Cost.rounds` counts what its name says. A bill is now findable by
the same word on the screen, in the log's role and in the queue's field, which
is what makes `round 2  read crates/x.rs  ↑1.2k` readable without a legend.

A snapshot written by an older build has no `bills` (the field is skipped on
serialization and read back from the log), and an older `Cost` has a `turns`
key that is ignored, so nothing is restored wrong -- nothing is restored at
all, which is the same outcome as a first launch without a log.

Cost: a serialized field name changed, and every prose sentence about a
conversation had to be rewritten to say message or round -- the places where
the old word was vague are exactly the places this rewrite is most likely to
have put the wrong one.
