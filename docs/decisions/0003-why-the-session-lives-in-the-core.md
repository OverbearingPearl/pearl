# 3. Why the session lives in the core

Date: 2026-11-16

## Status

Accepted

## Context

Pi's agent core is stateless: `Agent` holds messages in memory and the only
durable thing it knows is a session id forwarded to providers. Persistence
lives in the product: `AgentSession` subscribes to events and appends
entries, and the entry tree is `SessionManager`, where each entry carries a
`parentId`.

## Options considered

- Follow Pi: the core emits events and never reads a session; the product
  subscribes, appends, and replays the branch into the next request.
- Put the session in the core next to the loop, and keep storage in an
  adapter.

## Decision

The session is a core module: entries, their `parentId`, and the optional
patch each mutating entry carries. Writing them to a file is an adapter and
may be replaced.

## Consequences

PEARL's claim is that an applied change can be taken back. That claim cannot
be enforced across a layer boundary: if the loop hands a tool result to the
product and trusts the product to record what changed, the guarantee is a
promise rather than a property. In the core, the patch and the result are one
step.

The price Pi's split charges is visible in its own tree: a second consumer
that needs a recoverable agent cannot reuse the product's session object,
bound as it is to a working directory, settings, extensions and a buffer, so
it reimplements session and storage. One consumer pays once; the second pays
again.

Cost: the core is larger than Pi's, and every consumer inherits this session
model instead of choosing its own.
