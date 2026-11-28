# 23. Why a launch begins a conversation

Date: 2026-11-25

## Status

Accepted

## Context

A log file is every run in a directory: each launch opened
`pearl-log.jsonl`, appended to it, and built every request from the whole
log (ADR 7). Resumption was the point — ADR 7's summary, the replay rule, and
the resume promise all rest on the file being the conversation.

The cost showed up as a feeling rather than an error. A person typed "总结一下这个
工程" in a repository whose file held a week of runs and a pty test's `hello`,
and the answer referred to a different conversation's word. Nothing was broken:
the request really did carry those messages. But the launch had started with an
empty queue and a full context, and no screen said so. Two objects with
different lifetimes — the work, which is per process, and the memory, which was
per directory forever — were treated as one.

## Options considered

- Leave the file as one growing conversation: the cheapest, and it keeps
  `pearl` resuming with no flag, but a run's context is then every run's
  context, and the only cure is to rename the file by hand.
- Give each launch a file of its own (`pearl-log-<time>.jsonl`): a clean
  start is structural, but the directory fills with files nobody names, the
  log total and the undo history stop being per directory, and resuming
  means finding the right file first.
- A key on the surface that starts a new conversation: right where the pain is
  felt, but the surface is not the log's writer (ADR 17, one writer), so it
  needs a channel into the runner and a second path for the aside's reader —
  machinery for a boundary that a launch already marks.
- A mark in the log that a conversation begins, written by the launch, with
  `--resume` as the launch that writes none: the file keeps every run, the
  view starts at the last mark, and the person says when the memory is the
  previous memory.

## Decision

`Store::append_launch` appends `role = Launch` before a launch's first
request, and `view` returns the entries after the last one. `pearl` and
`pearl run` both write it unless `--resume` is given; that flag writes no mark
and so carries the conversation the last run left off.

The log stays one append-only file per directory, and a log written before the
mark existed is still the conversation it holds — there is no marker of where a
run began, so nothing is dropped from it.

## Consequences

A launch starts with an empty queue and an empty context, which is the same
promise twice. The junk a driven test writes is behind a mark the next launch
puts down, so it cannot reach a human's request. Folding, the aside and the
cost estimate all read the same `view`, so they agree about which conversation
they are in without knowing about the mark.

Cost: continuing yesterday's conversation is a flag rather than the default, and
`--resume` continues the *last* conversation in the file, which is the last run
whatever wrote it — so a test that ran after the human's work is the
conversation a resume continues. Both are the price of one file holding every
run.