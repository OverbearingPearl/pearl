;;; pearl-tools-edit.el --- the edit tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The one write path that goes through a buffer, and therefore the one that
;; the user's `C-/` takes back.  An edit is a list of disjoint replacements,
;; each anchored by the exact text it removes; a stale anchor is a loud error
;; and a round trip, which is the price ADR 4 accepted for the proof that the
;; model read the file.
;;
;; The whole call is one change group: accepted and amalgamated when every
;; replacement lands, cancelled when one does not, so a half-applied edit is
;; not a state the user can reach.
;;
;; Planned API
;;
;; TODO `pearl-tools-edit-declaration' -> tool declaration
;;      `edits[]', each with `oldText' and `newText', matching Pi's schema
;;      (ADR 8): unique match, no overlaps, several edits per call.
;;
;; TODO `pearl-tools-edit-run' (ARGUMENTS CONTEXT) -> tool result
;;      Locate every anchor before writing anything; refuse the whole call if
;;      one is missing or ambiguous, naming the anchor that failed.  Returns
;;      the file, the replacements applied and, for each, the line it landed
;;      on, so the console can report what changed and where.
;;
;; TODO `pearl-tools-edit-apply' (BUFFER EDITS) -> (APPLIED . PATCH)
;;      Open a change group through `pearl-undo-group-prepare', apply each
;;      replacement, and accept it on success or cancel it on failure; return
;;      the number applied and the patch the entry will carry.  This is the
;;      only caller of the undo change-group API in the package, which is what
;;      makes the promise auditable: one call site, one group, one undo step.
;;
;; TODO `pearl-tools-edit-unsaved-p' (BUFFER) -> boolean
;;      Whether the buffer was modified before the edit; a buffer that was
;;      already dirty is not saved by the tool, and the result says so.
;;
;; TODO `pearl-tools-edit-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

(provide 'pearl-tools-edit)

;;; pearl-tools-edit.el ends here
