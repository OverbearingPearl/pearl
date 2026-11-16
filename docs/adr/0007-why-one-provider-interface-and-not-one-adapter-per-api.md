---
title: "ADR-0007: Why one provider interface and not one adapter per API"
date: "2026-11-16"
status: accepted
summary: >-
  One request interface with a streaming implementation and a fake; a second vendor arrives with a second consumer.
---

# Why one provider interface and not one adapter per API

## Context and Problem Statement

Pi carries one module per API family, a generated model catalog and an auth
layer. Read closely, most of that surface is data and per-vendor quirk —
message placement, streaming tool-call deltas, stop reasons, reasoning
fields — rather than agent semantics.

## Considered Options

- Mirror Pi: an adapter per API family, plus a catalog and credentials
  management.
- One request interface, one implementation that speaks the OpenAI-compatible
  streaming protocol, and a fake implementation for tests.

## Decision Outcome

One interface, two implementations. A second real vendor arrives when a
second one is needed, and its quirks are then that vendor's adapter.

### Consequences

Reproducing Pi's provider layer means re-discovering eleven vendors' edge
cases and tracking their changes forever, for a package whose claim is about
the loop. Two implementations keep the interface honest: the fake is not a
mock of an abstraction, it is the abstraction used twice.

Cost: models behind providers we do not implement cannot be used, and users
with credentials configured elsewhere must configure them again. The single
implementation is also where every vendor quirk will land, so "one adapter"
is a claim about the interface, not about how little code the adapter holds.

What would change our mind: a second consumer that needs a provider we do not
have, twice.
