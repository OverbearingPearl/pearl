# 4. Why no LSP

Date: 2026-11-24

## Status

Accepted

## Context

LSP gives go-to-definition, hover, and diagnostics, but it shifts complexity to
the user: a language server must be installed and configured per language. The
user dislikes that shift, and PEARL's population lets the agent generate code
rather than navigate it by hand.

## Options considered

- Bundle LSP clients and require language servers: rich navigation, but the
  setup burden lands on the user and most of its value serves human navigation.
- Skip LSP and use the compiler as the feedback channel: `cargo check`/`clippy`
  in the loop gives the agent diagnostics with no per-language setup.

## Decision

We do not build LSP. The compiler is the diagnostic source for Rust; the agent
reads and edits through tools and learns from compiler output. Editor
intelligence that does not shift setup to the user may flow in later, but LSP as
a user-configured dependency is out.

## Consequences

No language-server setup burden; the agent's feedback is the compiler. Human
hand-navigation is not the served population, so its loss is acceptable.

Cost: no go-to-definition or hover for the human who does want to navigate.
