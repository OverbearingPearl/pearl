# 25. Why the boundary is a copy and not a cage

Date: 2026-11-18

## Status

Accepted

## Context

ADR 5 refused a sandbox and ADR 24 measured the macOS option and refused it
again, from evidence. The argument those two records left standing is this: the
promise holds wherever a write is inside the turn's snapshot, and everywhere
else the user is asked. That is weaker than "everything can be taken back", and
the gap is real — a confirmed command outside the snapshot is not revertible,
and ADR 24's measurements say macOS cannot close the gap with a kernel cage.

The gap is worth stating as a limit rather than a shortfall. To take a write
back, its pre-image has to exist; an unknown path has no pre-image. So
"everything is revertible" has exactly three routes: bound the write in advance
(a sandbox), copy before it happens (a clone or a pre-image), or admit the write
is beyond reach and report it. A sandbox is one of the three, and the only one
that needs the kernel. On macOS that route is closed; the second one is
unusually cheap there, and nothing in PEARL has been using it.

Measured on macOS 27 with APFS, `cp -c` clones by copy-on-write: a 200 MB file
clones in 3.8 ms and costs no additional space, a 2000-file tree in 156 ms, and
this repository with its `node_modules` in 706 ms. That is a copy nobody has to
think about, which changes which mechanism is affordable.

## Options considered

- Keep the sandbox question open and tell macOS users to live with the prompts.
  Honest, and leaves the gap exactly as wide as it is today.
- Adopt a container or a VM so the sandbox is someone else's kernel. Costs a
  daemon, a mounted tree, and 300 ms to a second per command, which changes how
  the tool feels; and it breaks the premise that the files the command writes
  are the files the editor is looking at.
- Copy instead of cage: redirect the environment so ordinary writes land inside
  the snapshot, take a pre-image of the paths a probe can name, and run an
  opaque command inside a clone of the tree and merge the difference back.

## Decision

No sandbox. The boundary is made of copies, in three parts, and each part has a
job:

1. **Redirect the environment.** A command runs with `cwd` at the project root
   and with `HOME`, `TMPDIR` and `XDG_*` pointed inside the project's store, so
   the ordinary accidents — a dotfile, a cache, a temporary file — land inside
   the snapshot that already covers them. Identity for `git` comes from
   `user.name` and `user.email` in environment variables; a command that needs
   real credentials fails and is confirmed, because copying `~/.ssh` or
   `~/.netrc` into a project to keep a command happy is a worse trade than a
   prompt.
2. **Take a pre-image of what can be named.** When the probe names paths the
   snapshot does not cover — outside the root, or ignored inside it — the
   command's own pre-image is cloned before it runs, so the confirmation buys an
   action that can still be taken back. A path that does not exist yet is
   recorded as absent, and taking it back deletes it. A path over the size limit
   is not captured, and the result then says the write is not revertible rather
   than implying it is.
3. **Run the invisible command in a clone.** A command whose scope cannot be
   named runs against a clone of the tree; the clone's difference is merged back
   as patches. Its writes become attributable by construction, which is the
   attribution problem ADR 23 could not solve with a worktree diff.

What none of this reaches is a command that writes an absolute path nobody
named — `rm -rf /Users/someone/Documents`, `sudo`, `/usr/local/bin`. That set
stays what it is today: confirmed before it runs, and not revertible after. No
copy can help, because there is nothing to copy until the path is named, and
macOS has no supported cage to put it in. It is a small set once the environment
is redirected, and it is the honest boundary of the promise.

Parts 2 and 3 are the parts this record commits PEARL to. Part 1 needs its own
record when it lands, because it changes which commands work and it has to say
so; part 3 needs one too, because running a command against a different absolute
path is a visible change to what the command sees.

## Consequences

A confirmed write stops being a write that cannot come back, which is the whole
point: the confirmation was previously trading revertibility for an action, and
now it trades a prompt for an action. A command whose scope is unknown becomes
attributable when part 3 lands, which is what lets two consoles share a root
without a shared undo. None of this needs a deprecated binary, a signature, a
daemon or a container, and the pre-image is a file copy, so the stories for it
are real file operations rather than mocks.

Cost: pre-images cost disk, which is why they are capped, and a capped path is a
write the user is told they cannot take back. Parts 1 and 3 change what a command
sees — its `HOME`, its absolute path — and some commands will break or behave
differently under that, which is why they are opt-out per command and why each
gets a record rather than a paragraph. And the residual set is not empty: a
destructive absolute path still gets a prompt and still cannot be undone, so
"everything is revertible" remains false and the console has to keep saying so.
