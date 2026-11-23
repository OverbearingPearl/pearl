# 9. Why one provider interface

Date: 2026-11-24

## Status

Accepted

## Context

The agent calls a model, and the first endpoint is DeepSeek. A per-API adapter
zoo multiplies maintenance; the loop should not learn a new wire per provider.

## Options considered

- One adapter per API: each wire bespoke, but the loop and tests multiply.
- One OpenAI-compatible interface: a single `Provider` trait, the base URL
  selects the endpoint, DeepSeek and any compatible server are the same code.

## Decision

One `Provider` trait with `async fn complete`. `OpenAiProvider` takes a base URL
and resolves its key at use, not at construction; DeepSeek is the default
endpoint. The model is chosen per request. A mock provider stands in for the
network so the loop is testable offline.

## Consequences

Adding a compatible endpoint is a base URL, not a new adapter. Non-compatible
wires would need a new implementation of the same trait, not a new interface.

Cost: providers that are not OpenAI-compatible need their own trait impl.
