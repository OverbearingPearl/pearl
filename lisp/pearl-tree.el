;;; pearl-tree.el --- what the working tree looked like  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A check, an edit and a commit all talk to a directory another session may
;; be writing at the same time, and none of them can say which tree its
;; conclusion is about.  A green check cannot be told from a check that ran
;; while somebody else was mid-edit, and a commit cannot tell its own paths
;; from the ones another session left half-written.
;;
;; This module gives that tree a name: the commit at the head, plus every path
;; git reports as changed, each with its size and modification time.  Two
;; epochs are equal exactly when nothing git can see has moved, so a command
;; that ran between two different epochs can say its verdict is about a tree
;; that is no longer there instead of reporting a stale pass or a stale fail.
;;
;; The fingerprint is deliberately coarse: it answers "did anything move", not
;; "was my change overwritten".  A refusal to write, undo or commit compares
;; the anchored patch (ADR 4), because two sessions may change different
;; regions of one file and both be right; refusing on "this file was touched"
;; would refuse the safe case (ADR 33).

;; Planned API
;;
;; TODO `pearl-tree-epoch' (ROOT) -> epoch or nil
;;      A fingerprint of the working tree at ROOT: `(:head COMMIT
;;      :changed ((PATH SIZE MTIME) ...))'.  Nil when the tree has no
;;      repository or git refused to answer, which is a state the caller
;;      reports rather than reads as "nothing changed".
;;
;; TODO `pearl-tree-drift' (BEFORE AFTER) -> list of paths or nil
;;      The paths whose entry differs between two epochs, sorted.  Nil when
;;      either epoch is nil: a comparison with no name on one end cannot say
;;      the tree held still, and claiming it did is the failure this module
;;      exists to prevent.

;;; Code:

(provide 'pearl-tree)

;;; pearl-tree.el ends here
