# 3. Why Rust

Date: 2026-11-24

## Status

Accepted

## Context

PEARL is a CLI/TUI agent whose fluidity and resource footprint matter at the
render and tool layers, and whose sandbox needs OS primitives. The language is
a substrate choice, not a performance claim about the network-bound loop.

## Options considered

- TypeScript: fastest iteration and fork velocity, but a carried runtime, GC
  jitter at the frame budget, and weaker syscall access for confinement.
- Go: single binary and concurrent GC, but a thinner TUI ecosystem.
- Rust: no GC, deterministic teardown, small static binary, direct FFI to
  Seatbelt/Landlock/Win32, and ratatui for differential rendering.

## Decision

Rust, edition 2024, MSRV 1.85. The loop is I/O-bound and language-agnostic; Rust
is chosen for the TUI and tool layers, the sandbox FFI, and a single binary per
platform without a carried runtime.

## Consequences

Startup and memory are an order of magnitude under a carried-runtime agent; the
sandbox reaches the OS directly. Iteration and fork velocity are slower than
TypeScript, which we accept because the core is bespoke anyway.

Cost: slower iteration and a smaller contributor pool than TypeScript.
