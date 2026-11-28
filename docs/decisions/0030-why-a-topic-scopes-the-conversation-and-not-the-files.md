# 30. Why a topic scopes the conversation and not the files

Date: 2026-11-26

## Status

Accepted

Amended 2026-11-26: an entry now names the task it was spoken for, and the view
looks the topic up from that task's place on the list, so a message moves with
its row instead of staying in the topic it was written in. The stamp written
at append time remains as the fallback for entries whose task is gone.

## Context

One directory, two lines of work, one queue. A person runs "read the printer"
for a while, then starts "read the parser" beside it, alternating every few
tasks. The log held one conversation, so the parser's first request carried the
printer's messages: the model answered the wrong question with confidence, and the
human paid for the tokens to make it do so.

The obvious repair -- a tree of history, the way a general agent forks a
conversation -- does not survive contact with this program. The conversation
lives in a log that can be replayed; the work lives in one working tree that
cannot. Jumping back to an old branch would restore the words and leave the
files where the newer branch put them, and the files are partly the human's own
uncommitted edits. A tree would promise a return it cannot make.

## Options considered

- **Copy the workspace per topic.** The fan-out child already does this, and it
  answers the question completely -- but it answers it by making two trees,
  which is a different product than one directory with two lines of work.
- **A flat tag on each message.** Cheap, and it cannot express the thing the human
  wants next: the parts of one topic, in order, with a topic that says how far
  it has got.
- **A root that holds work and runs none of it.** A topic as a node: the queue
  already has roots, focus, nesting and a projection; a topic is one more form
  of node, and "which topic does this task belong to" is its root ancestor.
- **Let membership be edited after a round has run.** The log only appends, and a
  round that ran under a topic belongs to it; editing afterwards would make the
  record say the work was done somewhere it was not.

## Decision

A topic is a root of `Form::Topic`. Each log entry carries `topic`, the id of
the topic its messages belong to, or nothing for the shared line -- the flat list
and every global entry, which all topics read. The runner stamps entries with
the topic of the task it is about to ask, and builds the wire conversation from
that topic's own messages.

The filter runs before coverage. A fold in one topic covers a stretch of
sequence numbers, and those numbers run across every topic in the file; asking
what is covered first would let one topic's summary swallow another's messages.

Membership may be edited only while a node has not run: `take` and `place` move
unsent work freely, and a topic whose parts have run cannot be dissolved.


A topic scopes the *conversation*, not the *results*: every task is handed the
rows of the whole list -- what is done, in one sentence each, and what still
waits -- so a line of work that finished is read as a row and not as the talk
that produced it.
## Consequences

Two topics in one directory stop quoting each other, and each folds its own
history. A launch with no topics behaves exactly as before: every entry belongs
to the shared line.

What a topic does not scope is the files. Two topics still write the same tree,
so the surface owes the human a sentence naming what the other topic changed
(see the roadmap row that carries it); the ledger already knows which task wrote
which path.

Cost: one field on every entry, a filter that must stay before the coverage
step, and a rule the human has to learn -- a topic whose parts have run is a
fact, not a label.
