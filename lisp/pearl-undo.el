;;; pearl-undo.el --- change groups and the turn snapshot  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The two layers under the promise.  The first is a change group per tool
;; call, amalgamated into a single undo step, which is the everyday path and
;; the reason there is no undo command to learn.  The second is a snapshot of
;; the working tree taken before a turn, which is what makes a change
;; revertible after the buffer is gone or Emacs has restarted.
;;
;; The snapshot is a dangling commit, written through a temporary index so the
;; real one is untouched: it covers edits made outside buffers, respects
;; `.gitignore', and needs no commit message.  A file that no longer matches
;; its baseline is a conflict, and a restore refuses by name rather than
;; guessing, because a wrong restore is worse than a refused one.  A binary
;; change carries no text patch: its durable restore is the snapshot's blob,
;; so once git has collected the dangling commit, a rollback can detect the
;; conflict but not restore the content.
;;
;; This module is the least replicable in the package: every other organ has an
;; upstream shape to copy, and this one is the promise itself.  M0.5 in the
;; roadmap exists to prove it before anything is built on it.
;;
;; Planned API
;;
;; TODO `pearl-undo-group-prepare' (BUFFER) -> handle
;;      Wrap `prepare-change-group' and remember the buffer, so the two ways a
;;      group can end cannot be confused with each other.
;;
;; TODO `pearl-undo-group-accept' (HANDLE) -> nil
;;      Accept the group and amalgamate it into one undo step.  After this,
;;      exactly one `C-/` reverts everything the tool call did.
;;
;; TODO `pearl-undo-group-cancel' (HANDLE) -> nil
;;      Cancel the group, leaving the buffer text and its undo list as they
;;      were before the call.  A failed tool is not an undo step.
;;
;; TODO `pearl-undo-snapshot' (ROOT) -> snapshot or nil
;;      A tree snapshot of ROOT taken before a turn, as a dangling commit
;;      written through a temporary index, or nil outside a git repository.
;;      Nil is a documented state, not a failure: the console says so.
;;
;; TODO `pearl-undo-restore' (SNAPSHOT PATHS) -> list of restored paths
;;      Restore each path from the snapshot after checking it against its
;;      baseline.  A path the snapshot does not contain was created after it
;;      and is deleted; a path the snapshot contains and the worktree does not
;;      is written back from it.  The conflict check covers both directions: a
;;      created file this turn did not write, or a deleted one it did not
;;      delete, is a conflict like any other.  Signals `pearl-undo-conflict'
;;      naming every conflict, and restores nothing when any path conflicts.
;;
;; TODO `pearl-undo-conflict-p' (SNAPSHOT PATH) -> boolean
;;      Non-nil when PATH changed since the snapshot in a way the patch on the
;;      entry cannot explain.
;;
;; TODO `pearl-undo-plan' (SESSION &optional ENTRY) -> plan
;;      What takes the session back to ENTRY, in reverse order, one path per
;;      step: `(:patch PATCH)' where a patch can reverse the change,
;;      `(:restore PATH)' where only the turn's snapshot can, and
;;      `(:preimage PATH)' where the snapshot never covered the path and the
;;      tool call's own pre-image is what takes it back (ADR 23, ADR 25).
;;      Never a single entry out of the middle: a later patch on the same path
;;      was written against this one.  A turn with nothing attributable
;;      contributes no step, because a step that cannot name what it reverted
;;      is a lie the console repeats.
;;
;;      The plan is here and not in the session.  Building it means choosing
;;      between three mechanisms, and a record that chooses is a record that
;;      interprets; the core keeps the patches and this module reads them
;;      (ADR 3, ADR 23).
;;
;; TODO `pearl-undo-patch-conflict-p' (PATCH CONTENT) -> boolean
;;      Non-nil when PATCH cannot be reversed against CONTENT, which is what
;;      makes a refused restore possible.  A patch whose written text is empty
;;      is always a conflict: an empty segment cannot be found uniquely.  This
;;      is the patch's question; `pearl-undo-conflict-p' is the snapshot's.
;;
;; TODO `pearl-undo-preimage-limit' -> defconst, in bytes
;;      How large a single path may be before PEARL stops copying it as a
;;      pre-image: 64 MiB.  A pre-image is paid for on every command that
;;      writes outside the turn's snapshot, and a copy that costs more than the
;;      write is a tax on the common case to cover the rare one (ADR 25).
;;
;; TODO `pearl-undo-preimage' (ROOT PATHS) -> preimage
;;      Clone PATHS into the project's pre-image area before a command writes
;;      them, so a write the turn's snapshot never covered can still be taken
;;      back.  Each path is recorded as copied, absent (nothing there to
;;      restore, so taking it back deletes whatever appeared), over the limit,
;;      or failed - and only the first two can be restored.  Never signals for
;;      a path it could not copy: a confirmed command still runs, and the
;;      result says what the undo can reach instead of aborting the action the
;;      user just approved.
;;
;; TODO `pearl-undo-preimage-restore' (PREIMAGE) -> list of restored paths
;;      Put back what it captured: a copied path from its clone, an absent one
;;      by deleting what is now there.  The paths it could not capture are left
;;      alone and are not in the return value, because a restore that names a
;;      path it did not restore is how a user learns not to trust the report.
;;
;; TODO `pearl-undo-preimage-revertible-p' (PREIMAGE) -> boolean
;;      Non-nil when every path was captured, so the command can be taken back
;;      whole.  One over-limit path makes it nil: half a rollback reported as a
;;      rollback is worse than a write that is honest about being permanent.
;;
;; TODO `pearl-undo-attach' (LOOP ROOT SESSION-ID) -> unsubscribe function
;;      Subscribe to the loop's `turn-start' and `turn-end' events and keep
;;      one snapshot per turn, dropping it when the turn changed nothing.  The
;;      last one is kept under (ROOT SESSION-ID) and not under ROOT, because
;;      two consoles on one root must not offer each other's turns (ADR 23).
;;      The loop may not require this module (ADR 12: product above core), so
;;      the console wires it in when it builds the loop, and the event stream
;;      is the only channel it needs.
;;
;; TODO `pearl-undo-turn-begin' (ROOT) -> turn
;; TODO `pearl-undo-turn-end' (TURN) -> list of paths changed
;;      The pair the event handler calls.  `turn-begin' takes the snapshot;
;;      `turn-end' returns the paths this turn wrote that actually moved, and
;;      records their post-turn state as the baseline a restore verifies
;;      against.  "Wrote" is attribution and not a diff: the paths are the
;;      patches the tools returned plus the `:scope' a shell call proved, and
;;      a tree diff only decides which of those moved.  The two questions
;;      differ the moment a second session writes in the same root, and the
;;      one this pair answers is "which of these did I change" (ADR 23).
;;
;; TODO `pearl-undo-turn-snapshot' (TURN) -> snapshot
;;      The snapshot a turn began with, for `pearl-undo-restore' and
;;      `pearl-undo-conflict-p' to read.
;;
;; TODO `pearl-undo-latest' (ROOT SESSION-ID) -> snapshot or nil
;;      The snapshot of the last turn of that session that changed something,
;;      or nil when its last turn changed nothing.  This is what the console's
;;      rollback offers when the user says "undo that turn": the turn they are
;;      looking at, not the last turn anybody ran under this root (ADR 23).

;;; Code:

(provide 'pearl-undo)

;;; pearl-undo.el ends here
