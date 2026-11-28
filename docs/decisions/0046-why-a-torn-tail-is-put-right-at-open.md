# 46. Why a torn tail is put right at open

Date: 2026-11-28

## Status

Accepted

## Context

The log is append-only and every read refuses a line it cannot parse
(`StoreError::Malformed`). ADR 31 named the cost of that in passing: a capture
taken mid-append could leave a torn last line, and one torn line made the whole
log unreadable. The hazard arrived another way. A write was a line and a
newline in two calls, so two threads appending at once could put one record's
data inside another's line; the human's own log then opened as `malformed log
line 1563: trailing characters at line 1 column 192`, and the product would not
start.

That writer is now one write per record, so two records can no longer share a
line. What remains is the kill: a process stopped between the bytes of a record
and the newline that ends it. A record ends with a newline, so bytes after the
last newline are not a record — they are half of one, or a whole one whose
ending never landed.

## Options considered

- Refuse the log, as now: correct about the file's bytes and wrong about the
  human. A 3 MiB conversation and the list built at the keyboard become
  unreachable because one append was interrupted; the message names a line
  number and offers nothing to do about it.
- Repair any tail that does not parse, splitting it where a value seems to end:
  invents a boundary the writer never wrote, and a wrong guess loses two records
  instead of none.
- Keep the log's last line and let the next append continue it: the next record
  is then appended to a half-written one, which is the corruption this record is
  about, made permanent.
- Put right only what the file's own invariant decides: a whole record missing
  its newline is kept, half a record is dropped.

## Decision

`Store::open` reads the bytes after the last newline before it counts anything
(`Store::repair_tail`). Three answers, and only the first two touch the file:

- The tail parses as an entry: the record landed and its newline did not, so the
  newline is written and the record kept.
- The tail fails as "end of input": the append stopped in the middle of the
  record, so the tail is truncated and the record that never finished is gone.
- Anything else — a line with trailing characters, a byte sequence that is not
  JSON: left alone and reported by the read, because a repair that guesses at
  an unreadable line is how records are lost quietly.

Nothing is appended to say the repair happened. The tail was never a record, so
there is no entry to stand in for it, and a note written by the open would be
the first thing in the conversation a resume reads.

## Consequences

A killed process costs the record it was writing and nothing else. A log that a
crash left unterminated opens, and its next record lands on a line of its own.
The repair is idempotent: a file that already ends in a newline is not read past
its end and not written at all.

The writer's half of the class is fixed too: one append is one `write_all` of
the record and its newline together, so the queue watcher and a run sharing a
log cannot interleave inside a line.

The boundary is real: a joined line in the middle of a file, or a tail with
trailing characters, is still an error. Two windows cannot share a log (the
claim), so a joined line in the middle can only come from a process that wrote
that way, and the log says so rather than hiding it.

Cost: a record the kill interrupted is dropped without a word on the screen, so
the only evidence it existed is the sequence number that is now missing from the
file, and a reader who wants that evidence has to notice the gap.
