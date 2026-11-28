# 1. Record architecture decisions

Date: 2026-11-24

## Status

Accepted

## Context

PEARL makes choices a competent engineer would otherwise make differently, and
reverses some of them as the design is discovered. Without a record, each later
log re-argues the same point or quietly contravenes a settled one.

## Options considered

- Keep decisions in commit messages only: cheap, but unsearchable and lost once
  the commit scrolls away.
- A wiki or chat log: grows by one paragraph per log and stops answering
  anything.
- Nygard-style records in `docs/decisions/`, one decision each, read into the
  agent's prompt as a veto list.

## Decision

We record decisions as `NNNN-kebab-title.md` in `docs/decisions/`, issued by
`scripts/adr`, which repairs the numbering lock and rebuilds the table of
contents after every command. A record carries Context, Options considered,
Decision, and Consequences ending in a `Cost:` line. A superseded record moves
to `superseded/` and links both ways.

## Consequences

Every log pays for each top-level record until it moves to `superseded/`,
so a record must still be true when next read. The directory is an index, not a
work log.

Cost: each accepted record is context every later log carries.
