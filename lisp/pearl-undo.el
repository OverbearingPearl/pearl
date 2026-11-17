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
;; guessing, because a wrong restore is worse than a refused one.
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
;;      baseline.  Signals `pearl-undo-conflict' naming every conflict, and
;;      restores nothing when any path conflicts.
;;
;; TODO `pearl-undo-conflict-p' (SNAPSHOT PATH) -> boolean
;;      Non-nil when PATH changed since the snapshot in a way the patch on the
;;      entry cannot explain.
;;
;; TODO `pearl-undo-attach' (LOOP ROOT) -> unsubscribe function
;;      Subscribe to the loop's `turn-start' and `turn-end' events and keep
;;      one snapshot per turn, dropping it when the turn changed nothing.  The
;;      loop may not require this module (ADR 12: product above core), so the
;;      console wires it in when it builds the loop, and the event stream is
;;      the only channel it needs.
;;
;; TODO `pearl-undo-turn-begin' (ROOT) -> turn
;; TODO `pearl-undo-turn-end' (TURN) -> list of paths changed
;;      The pair the event handler calls.  `turn-begin' takes the snapshot,
;;      `turn-end' compares the worktree against it and returns what moved;
;;      the patches themselves come from the tools and are recorded by the
;;      loop, not here.  `turn-end' also records the post-turn state, which
;;      is the baseline a restore verifies against: a file the user changed
;;      after the turn is a conflict, a file only the turn changed is not.
;;
;; TODO `pearl-undo-turn-snapshot' (TURN) -> snapshot
;;      The snapshot a turn began with, for `pearl-undo-restore' and
;;      `pearl-undo-conflict-p' to read.
;;
;; TODO `pearl-undo-latest' (ROOT) -> snapshot or nil
;;      The snapshot of the last turn that changed something under ROOT,
;;      or nil when the last turn changed nothing.  This is what the
;;      console's rollback offers when the user says "undo that turn".

;;; Code:

(provide 'pearl-undo)

;;; pearl-undo.el ends here
