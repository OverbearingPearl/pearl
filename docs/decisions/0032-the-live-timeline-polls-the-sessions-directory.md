# 32. The live timeline polls the sessions directory

Date: 2026-11-19

## Status

Accepted

## Context

The timeline is a projection of every session under a root (ADR 30), and the
roadmap promises a page that follows a live writer without being rebuilt. Two
consoles under one root are two buffers writing two files: the reading console
draws the root once, when `C-c C-t` is run, and nothing tells it that the other
session appended a row. `pearl-console-note` and `pearl-console-pending` were
written for that moment and, before this record, had no caller outside the
tests: the page's mechanism was proved and wired to nothing.

The obvious mechanism is `file-notify`: Emacs watches the sessions directory and
the handler notes the nodes. It was measured first, and on the macOS build this
project is developed on it does not deliver. With the kqueue backend, a watch
was added to a directory and to a file, the file was written, and three seconds
of `accept-process-output` returned `NOTIFY=nil` in both cases. A story cannot
prove a mechanism the test runner cannot see, and code that behaves one way on
inotify and another on kqueue is a promise that depends on where it is read.

## Options considered

- `file-notify-add-watch` on the sessions directory and a handler that reads the
  changed files. The right shape, and not provable in batch under kqueue, so the
  story would assert nothing and the guarantee would differ by platform.
- No live following, with `C-c C-t` the only way a page learns about another
  session. That is what the code did, and it makes the roadmap's acceptance
  false.
- Poll the sessions directory on a timer, re-read it only when the files
  changed, and hand the nodes to `pearl-console-note`. This is what the record
  chooses.

## Decision

The console owns a repeating timer, at `pearl-console-follow-interval` (one
second), started by `pearl-app--wire` and stopped by `pearl-console-close`. Each
tick compares a signature - the name, size and modification time of every
`sessions/*.jsonl` under the root - with the one the page recorded when it last
read the root; when they differ, the console reads the sessions and calls
`pearl-console-note` with the nodes. Only a buffer that is showing the timeline
is touched: `pearl-console-timeline` sets that flag, and a live event or a
record draw clears it, so a conversation buffer never receives another session's
rows.

What the timer supplies is the "a caller saw the files change" that
`pearl-console-note` already documents. Whether the row is inserted or counted
stays the page's decision, made from following, the streaming state and point.

## Consequences

A writer's row reaches a following page within one interval, with no command,
and it still arrives as nodes from the files: the page is a projection and never
a second record. A reader inside a fold or away from the tail sees the count
instead, and the next send shows the rows.

A tick on an unchanged root reads no entry, so an idle console costs one
`file-attributes` per session file per interval. A tick that finds a change
re-reads every session file in full, so a very large root pays a full read to
show one appended row, and a root with several consoles pays the poll once per
console.

Cost: a one-second poll is a latency floor on the page and a permanent timer per
wired console, in exchange for a mechanism that behaves the same wherever Emacs
runs the suite; a file watch would have been event-driven, and the platform's
batch behaviour is what it costs not to have one.
