# Glossary

The words this design uses, each pinned to what it means here.

### Queue

The task tree the human edits while the agent runs. A tree in data, a
single-focus flat list in view. Persisted outside the model context so compaction
cannot drop it. The core mechanism (ADR 2).

### Node

One task in the queue, with a state, a steering log, an optional frame, an
optional question, and children. A parent finishes only when its children settle.

### Steering

An annotation on a queued or running node, drained at the turn boundary. The
human's "also consider X", guaranteed not to be forgotten (ADR 10).

### Follow-up

A new node appended to the queue. Distinct from steering, which annotates an
existing node.

### Preempt

Pushing an urgent node ahead of the running one, saving the running node's frame
and pausing it. Resume restores the frame when the urgent node settles.

### Frame

The saved progress of a paused node, so resume continues rather than restarts.

### Boundary

The safe point between turns where steering is drained into the session. Never
mid-token.

### Projection

Compaction that renders a summary plus recent entries for the model while leaving
the full log untouched (ADR 7).

### Single-focus

The view discipline: one cursor for attention, many badges for awareness. The
human tends one focus while execution runs in parallel (ADR 11).

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