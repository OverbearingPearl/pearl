;;; pearl-review.el --- tool output in the editor's own grammar  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A build that fails inside a console is a wall of text, and the user's next
;; move is to read it and then go and find the file by hand.  This module is
;; the difference between an agent that prints and an agent that lives in the
;; editor: a compiler message becomes a `next-error' target, a symbol becomes
;; an `xref' hit, and the report is folded rather than replayed.
;;
;; It reads tool results and writes into the editor's machinery, not into the
;; console.  It knows nothing about which tool produced the text, which is why
;; a new tool needs no change here: what it emits either matches a compiler
;; message or it does not.
;;
;; Planned API
;;
;; TODO `pearl-review-install' (CONSOLE) -> unsubscribe function
;;      Subscribe to `tool-execution-end', parse the result, and keep the
;;      findings in a buffer of our own rather than in the console, so
;;      `next-error' has a list to walk.  Returns a function that removes the
;;      subscription and kills the buffer.
;;
;; TODO `pearl-review-parse' (TEXT) -> list of findings
;;      One finding per line that names a path, a line and a column, in the
;;      shapes compilers emit; `:kind' is `error', `warning' or `note'.  A
;;      line that does not match is not a finding, and a result with no
;;      findings is an ordinary result.
;;
;; TODO `pearl-review-fold-p' (RESULT) -> boolean
;;      Whether the console may collapse RESULT to one line.  A result with
;;      findings folds, because the findings are now navigable elsewhere; a
;;      result with none does not, because its text is the only copy.
;;
;; TODO `pearl-review-locate' (FINDING) -> marker or nil
;;      Resolve a finding to a marker in the file's buffer, opening it if
;;      needed.  Returns nil rather than signalling when the file is gone, so
;;      a stale report degrades to text instead of an error at navigation
;;      time.

;;; Code:

(provide 'pearl-review)

;;; pearl-review.el ends here
