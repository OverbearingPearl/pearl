# Glossary

The words this design uses, each pinned to what it means here.

### Queue

The task tree the human edits while the agent runs. A tree in data, a
single-focus flat list in view. Persisted outside the model context so compaction
cannot drop it. The core mechanism (ADR 2).

### Runner

The agent loop as a background task over the queue: it runs whatever is placed
and waits when nothing is, so the surface never has to start work itself. It
holds the same queue the surface holds, because a queue is a handle over one
tree (ADR 17).

### Node

One task in the queue, with a state, a steering log, an optional frame, an
optional question, and children. A parent finishes only when its children settle.

### Steering

An annotation on a queued or running node, drained at the turn boundary. The
human's "also consider X", guaranteed not to be forgotten (ADR 10).

### Follow-up

A new node at the end of the queue, so the list reads in the order the human
typed it. Distinct from steering, which annotates an existing node, and from
placing a taken request after the focus, which is a deliberate reorder.

### Take and place

The editing model: `take(id)` lifts an unexecuted node into the hand as a
`Request`, the human edits it, and `place(request, position)` puts it back at a
new position or as steering. Append, insert and reorder are one move; a
rewrite changes a live task's objective without taking it out of the list.

### Hand

The lifted node: `t` takes a row off the list and keeps it whole, parts and all.
`a` puts it back where the cursor is, `i` at the front, `r` renames it, and
`␛` gives it back the place it came from -- a lift is a move half made, not a
discard. A topic with something in flight cannot be lifted.

### Topic

A root node that holds work and runs none of it: `A` starts one, its parts run in
their own time, holding it holds them all, and it is done when they are. `c`
dissolves it while none of its parts has run. Dropping one topic on another
merges them: the parts move in and the empty topic goes.

### Draft

What the human is writing into the surface: a buffer with a caret, of any
number of lines, keyed by the chosen style (vim's modes or emacs's chords).
It takes its rows before the panes do, because it is the one thing on the
screen nobody can rebuild. `⏎` places the task; `Tab` gives the keys to the
queue and brings them back; `Esc` puts the draft down and a second `Esc` drops
it.

### Request

A task not yet placed: a title, a `Form` (`Task` or `Steering`), and an optional
target. The disassembled form of a node; there is no separate draft type.

### Hold and run

Hold pauses a running node and saves its frame without an urgent successor;
running restores the frame. One key toggles between them, because which of the
two a press means is the node's state, not the human's to remember. Preempt is
hold plus an urgent node pushed ahead.

### Preempt

Pushing an urgent node ahead of the running one, saving the running node's frame
and pausing it. Resume restores the frame when the urgent node settles.

### Frame

The saved progress of a paused node, so running it on continues rather than restarts.

### Boundary

The safe point between turns where steering is drained into the session. Never
mid-token.

### Projection

Compaction that renders a summary plus recent entries for the model while leaving
the full log untouched (ADR 7).

### Fold

One compaction: the model call that turns a range of old entries into a summary,
and the entry that records it. A fold is skipped when it would reclaim less than
the minimum, so the context is allowed to be long rather than churned for free.
The status line calls it `summarized Nx`, because "fold" is what the code calls
it and not what happened.

### Coverage

The `first..=last` seq range a summary entry stands in for. It is what makes a
second fold reach back to the start of the conversation instead of only to the
last summary (ADR 16).

### Meter

The estimate of how full the context is, and the factor it learns. It counts CJK
characters one for one — a character count divided by four underestimates them
by up to four times — and divides the rest by four, then scales by the ratio it
observed between its estimate and the billed `prompt_tokens`.

### Single-focus

The view discipline: one cursor for attention, many badges for awareness. The
human tends one focus while execution runs in parallel (ADR 11).

### Subagent

A derived session spawned under a queue node (`parent-{child}.jsonl`), running
its own loop inside its own copy of the workspace (`children/<child>/`) and
settling as a queue event. The parent finishes only when every child settles; the
human keeps one focus.

### Workspace copy

The tree a child works in, made when it is spawned. Its writes and reverts are
its own by construction, so two children editing the same file name never meet
(ADR 15).

### Flow void

The idle waiting during agent generation that breaks the human's flow. The
editable queue is the answer: a continuous single-thread role while the agent
works.

### ADE

Agent Development Environment. The agent is the subject; the editor, if any, is
a light peripheral. PEARL is queue-centric, not editor-centric.

### AppContainer

A Windows low-privilege profile: a unique SID that can touch nothing it was not
granted. Half of the Windows sandbox (ADR 5).

### Job Object

A Windows kernel object that groups processes; with kill-on-close it takes the
whole tree down when its handle closes. The other half of the Windows sandbox.

### Landlock

The Linux kernel's unprivileged sandbox: a process restricts itself and its
children without root. The Linux sandbox.

### Seatbelt

The macOS sandbox, driven by `sandbox-exec` with a profile. The macOS sandbox.
### Baseline

A before-image of every work file in the workspace, captured before a shell
op. Restoring it undoes the op: changed files go back, created files are
removed, deleted files return. Pearl's own state is not work and is left out
(ADR 31); the bytes wait on disk under content-derived names, so a run keeps
one copy of each unchanged file rather than one per command (ADR 32). The
shell's revert (ADR 13).

### Before-image store

The directory in the system's temporary space where a run's captured bytes
wait, named by what they are. Opened on the first capture, shared by every op,
removed when the run ends. A blob the system cleaned away makes that op's
revert refuse with words rather than restore half a tree (ADR 32).

### Copy

`y`, handing the focused task to the terminal as an OSC 52 clipboard write: its
words, its notices and its answer, without the bills the report adds. A
terminal that refuses gets the text in a file whose path the notice names.

### Op id

The number a shell op reports, naming its baseline. `revert_bash(op)` restores
that op's before-image; each op's baseline is independent of every other's.

### Thinking

What the model worked out before answering, when the endpoint sends it. Kept in
the log and on the node, folded on screen, never replayed as input.

### Nesting

Putting a part under a task (`n`). The parts run first, depth-first, and the
whole reads what they found; a held part holds its subtree.

### Fan-out

A running node asking for children: the `subagent` tool takes titles, each child
runs in its own session and workspace copy, and their answers return to the
parent as one tool result.

### Page

One window's height in rows, for either window. The plain key pages the queue
and carries its cursor; Alt pages the report.

### Layout

Which window the screen is given to: queue, report, or both. A direct choice
(`1`/`2`/`3`), not a focus -- every key already names the window it acts on.

### Scratch roots

The directories a confined child may write besides the workspace: `$TMPDIR` and
`/tmp`, both spellings. Where a script, a log or an intermediate goes.

### Place

The ground the work stands on: the workspace directory, the branch and its
changed-file count, and the sandbox policy in force. Shown on its own row.

### Cost

Tokens up, tokens down, elapsed time and a turn count for one request, summed
into a task's total and the session's. Read at three scopes from one ledger.

### Bill

One round's cost as the log records it, written once the round is over: the
model's wait plus the tools it asked for. The report itemises the rounds, so a
resume reads them from here rather than from a copy inside every snapshot.

### Footprint

The resident memory the OS reports for this process, shown as `mem 14M`. The
kernel's figure, not an estimate. What it holds is the list and the request in
flight, never the night behind them: a request reads the conversation it will
send and drops it, a shell op's before-image and a write's snapshot wait on
disk, and a log a night long is streamed rather than read into memory. So the
figure is a floor of a few megabytes plus the list's own text -- a couple of
kilobytes a row, measured by holding two forty-row lists and comparing. It
climbing means the list got longer, which is the work the human asked to see,
and it climbing with the number of steps rather than the number of rows is a
bug: the stories in `crates/pearl/tests/memory-tests.rs` are the ones that say
so.

### Credential

A key and the house it belongs to, resolved at use from the house's environment
variable or an authinfo line naming the endpoint's machine -- `~/.authinfo`, or
`~/.authinfo.gpg` through the user's gpg. Never stored.

### Keys

The motion style — plain, vim or emacs — cycled with `K` and saved with the
other defaults. Actions keep their letters in every style.

### Cache hit rate

The share of the session's prompt tokens the provider served from its cache,
accumulated over requests rather than read from the last one. A cached token
costs a tenth of an uncached one on DeepSeek.

### Gauge

The live reading of the context window: what the last request was billed for
and how many times history has folded, shared by the runner and the surface.

### Ops ledger

The writes that can still be taken back: each shell op's before-image and the
command that produced it, shared by the runner and the surface. Taking one back
consumes it, so a second attempt says there is nothing there.

### Settings

The knobs in force: the model, the thinking effort, the context window and the
fraction of it at which history folds. A handle shared by the surface and the
runner, so `:` changes them for the next request rather than the next process.

### Notice

Why the last key did nothing, in the task's own words: the state it is in and
the state the move needed. Shown above the key line until the next action works
or `␛` dismisses it. A key offered and silently refused leaves the human
guessing whether the tool is broken or they are.

### Activity

What a running task is doing right now, in the runner's words: `asking the
model`, or the tool and the thing it touches. The newest act replaces the last,
and the report takes its place when the task ends. It is what the pane shows
during the wait, so the wait is not a blank.

### Notice

A line on a task saying what the environment did to it, rather than what the
model said: a write the sandbox refused, with the words the OS used and the
ground writes may land on; or history folded into a summary, with how many older
entries it stands in for and the size before and after (`26.4k -> 7.1k`). Notices are drawn above the answer and outlive it -- the
activity line is replaced when the task ends, a notice is not, because an answer
shaped by one is not the whole record without it. The header's fold count is the
session's figure; a notice is the task's.

### Report

What a task produced: its reply, or the reason it failed. Shown in the pane below
the queue for the focused task, tail-first, so the human reads the work while
shaping the next task. A badge says a task settled; the report says what came of
it.

### Aside

A side question held outside the queue: asked against the session read-only,
answered without entering the log, shown on the surface until dismissed. The
`/btw` move. Not work, so the scheduler never sees it (ADR 14).
