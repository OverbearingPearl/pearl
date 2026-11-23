# 5. Why the Windows sandbox is AppContainer and a Job Object

Date: 2026-11-24

## Status

Accepted

## Context

The user requires a sandbox experience usable on Windows, not an abstraction
that quietly does nothing there. Windows has no Seatbelt and no Landlock, and no
clean stable pure-Rust crate wraps its confinement primitives. Microsoft's MXC
(MIT) implements them, but its Windows backend is a large subsystem, not a small
snippet, and a git dependency on it is heavy and young.

## Options considered

- Depend on MXC via git: purpose-built, but a large young dependency for one
  platform's path.
- Copy MXC's Windows code wholesale: MIT permits it, but the backend is hundreds
  of kilobytes of intricate FFI we would then own.
- Write a thin native confinement against `windows-sys`, following MXC's
  approach: an AppContainer profile for the low-privilege SID and a Job Object
  with kill-on-close for the process tree.

## Decision

We write our own thin Windows confinement in `pearl-tools/src/windows.rs`
against `windows-sys`: an AppContainer profile gives the process a unique SID
that can touch nothing ungranted, a Job Object with kill-on-close takes the tree
down, and the workspace is granted to the SID for `WorkspaceWrite`. The approach
follows MXC's MIT-licensed `job_object.rs`; the FFI is ours. macOS uses Seatbelt
(`sandbox-exec`), Linux uses Landlock. Each is the OS's own primitive.

## Consequences

A real sandbox on all three platforms with no heavy dependency, cross-checked
for `x86_64-pc-windows-msvc`. AppContainer is not a VM-grade cage; it is the
boundary-as-copy stance, a confinement the user can see and kill.

Cost: we own the Win32 FFI and its correctness, and AppContainer is weaker
isolation than a container or a VM.
