# 19. Why the shell is the platform's own

Date: 2026-11-24

## Status

Accepted

## Context

The bash tool ran `sh -c` on every OS, including Windows. That was not a
decision anyone wrote down; it was a Unix default that reached a platform where
`sh` is not part of the system. On the GitHub Windows runner the tool's own
stories answered for it: one `sh` invocation took between 60 and 330 seconds,
and the commands it was asked to run left the workspace untouched. The same
confinement, asked to run `cmd /C`, wrote its file in a fraction of a second.
MSYS needs named kernel objects the AppContainer does not grant, so `sh` pays a
timeout and then does nothing. A Windows user without Git Bash would not even
pay the timeout; they would get "could not spawn".

The tests had hidden this because the only Windows shell stories spawned `cmd`
directly, while the product spawned `sh`.

## Options considered

- **Bundle a POSIX shell** (Git Bash, busybox) and ship it: keeps one syntax
  everywhere, but adds a vendored binary, a licence question, and the same
  AppContainer namespace problem that makes MSYS fail today. A bundled shell
  that cannot start is not a shell.
- **PowerShell**: present on every supported Windows, but a second start-up
  cost on every op and a syntax further from what the model writes than the
  shell the user's own terminal runs.
- **Keep `sh` and require Git Bash on Windows**: documents a dependency the
  project promised it did not have (M3: confinement executed for real on
  Windows), and leaves the failing behaviour in place.

## Decision

The shell is the one the platform ships: `sh -c` on Unix, `cmd /C` on Windows.
Pearl does not bundle a shell and does not require one to be installed. The
tool keeps its `bash` name, because the model's act does not change; what the
name resolves to does.

The command is not passed through `cmd`'s argv. `CreateProcessW` quotes with the
C runtime's rules, while `cmd.exe` parses the raw line itself and does not undo
those escapes, so a command containing a quote would arrive mangled. On Windows
the command is written to a per-op batch file in the workspace and that file is
invoked; the only text on the command line is a relative script name and the
output redirection. On Unix the existing `{ command ; } > capture` wrapper is
used. The capture file is named relative to the workspace on both, so no path
ever has to survive a parser.

## Consequences

A stock Windows machine can run the agent's shell, the confinement stories and
the product now spawn the same shell, and the shell a user sees is the one
their own terminal answers to. A Windows agent writes `.cmd` where a Unix agent
writes `.sh`; the model is told which shell it has.

Cost: two shell syntaxes to support and document, a script written on Windows
cannot be carried to Unix unchanged, one transient batch file per op in the
workspace, and a Windows story that writes a POSIX one-liner will fail on
Windows instead of quietly passing -- which is the point.