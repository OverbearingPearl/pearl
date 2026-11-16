# 12. Layer rules are checked, not documented

Date: 2026-11-16

## Status

Accepted

## Context

The architecture has three layers and one rule: core is used by adapters,
adapters are used by product, and nothing points the other way. Written in a
document, that rule decays the first time a product module needs a helper
that lives in core and the helper is reached from the wrong side.

The check suite that ships with this repository classifies modules by
directory — main, business, test, tooling — and enforces which classes may
require which. It knows nothing about layers, so for this package it would
accept a `require` in any direction inside `lisp/`.

## Decision

The layer of each module is declared once, in `config/ci.el`, as a table of
layer to module names plus the layers each may require. A check in the suite
reads that table and fails any `require` that points against the arrows.

## Consequences

A reviewer argues about the table, not about a `require` line, and the
argument has one place to land. Moving a module between layers is a one-line
change that immediately reports every caller it breaks.

Cost: the table duplicates the module list, so a new module has to be added
in two places or it is unchecked. A module missing from the table is treated
as unlayered rather than as exempt, which is why the check reports it.
