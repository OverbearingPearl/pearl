# 18. Why a retry stops at the first byte

Date: 2026-11-18

## Status

Accepted

## Context

The adapter has three limits and nothing to say about the two things a real
endpoint does all the time: it answers 429 when the account is going too fast,
and it answers 503 while a node is being replaced. Neither is a bug in the
request, and both are ordinary traffic on the first day someone uses PEARL.

Retrying looks like a one-line decision until the stream is involved. A
request that dies after three paragraphs and is quietly asked again renders
those three paragraphs a second time, and the console has no way to tell the
copy from the original. A request that dies after its tool calls have been
assembled is worse: the tool calls have already been recorded, and running the
turn again produces a second edit of the same file. Neither failure is visible
in a test that never loses a connection, and both look like a model that
changed its mind.

The same question is asked by a body that closes without `data: [DONE]`. A
proxy that resets, a TLS session that is torn down, an endpoint that dies
mid-token: the transport reports an orderly end of file, so a client that
trusts it renders half a turn as a complete one.

## Options considered

- No retry at all. Simplest, and honest about failure, but a 429 during the
  first hour is not a failure the user can act on, and the console would report
  a working account as broken.
- Retry any failed request, including one that failed mid-stream. Needs
  de-duplication the console cannot do, and re-running a turn whose tool calls
  already landed changes files twice.
- Retry only while nothing has been delivered, and treat a close before the
  terminator as an error with a name.

## Decision

The two rules are one policy: a request either produces a whole turn or an
error that says what happened.

Retryable are 429 and 503. Nothing else: a 401 is the user's key and a 400 is
the request, so asking again spends an attempt on an answer that will not
change. `RETRY-LIMIT` counts additional attempts, defaults to 2, and `Retry-After`
is honoured when it is an integer number of seconds; otherwise the delay
doubles from half a second. The idle, total and byte limits are per attempt,
so a retry gets its own clock and its own budget.

Retrying stops at the first delivered event. From the moment an event has been
emitted, a failure ends the request: the stream is closed, `on-error` receives
the reason, and what already arrived stays where it is. A body that closes
before `data: [DONE]` is `(:reason truncated)` rather than a clean end, for the
same reason: the request did not produce a whole turn, and saying so is
cheaper than guessing which half was meant.

## Consequences

A 429 on the first second of a turn costs nothing visible. A cut stream is
named instead of looking like a short answer, and the console can say which
of the two happened. There is no de-duplication machinery anywhere, because
there is no second copy to remove, and the loop never re-runs a turn whose
tool calls landed.

Cost: a stream cut after it started costs a turn. The user retypes, or the
console offers a retry button later; neither is in this record. A server that
answers 429 forever still ends in `http-error` after the limit, which is the
correct outcome and still wastes the retries. And `Retry-After` is honoured
only as an integer: the HTTP-date form is accepted by the RFC and ignored
here, so an endpoint that sends a date is retried on our clock instead of
its own.
