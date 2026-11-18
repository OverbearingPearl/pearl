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
;; `pearl-review-parse' (TEXT) -> list of findings
;;      One finding per line that names a path, a line and a column, in the
;;      shapes compilers emit; `:kind' is `error', `warning' or `note'.  A
;;      line that does not match is not a finding, and a result with no
;;      findings is an ordinary result.
;;
;; `pearl-review-fold-p' (RESULT) -> boolean
;;      Whether the console may collapse RESULT to one line.  A result with
;;      findings folds, because the findings are now navigable elsewhere; a
;;      result with none does not, because its text is the only copy.
;;
;; `pearl-review-locate' (FINDING) -> marker or nil
;;      Resolve a finding to a marker in the file's buffer, opening it if
;;      needed.  Returns nil rather than signalling when the file is gone, so
;;      a stale report degrades to text instead of an error at navigation
;;      time.
;;
;; `pearl-review-xref-backend' -> xref backend
;;      The backend `pearl-review-install' adds to `xref-backend-functions'
;;      in the console buffer: an identifier a finding named resolves to the
;;      place the finding named, so `xref' walks tool output the same way
;;      `next-error' does.

;;; Code:

;;; Code:

(require 'cl-lib)
(require 'xref)

(defconst pearl-review--line-rx
  "^[ \t]*\\([^:\n]+\\):\\([0-9]+\\)\\(?::\\([0-9]+\\)\\)?[: \t]+\\(error\\|warning\\|note\\):[: \t]+\\(.*\\)$"
  "A line that names a place, in the shape compilers emit.")

(defvar-local pearl-review--findings nil
  "The findings the tool output in this buffer named.
Buffer-local, because two consoles walk two builds.")

(defvar-local pearl-review--position 0
  "How far through the findings the last navigation went.")

(defun pearl-review-parse (text)
  "Return one finding per line of TEXT that names a path and a line."
  (let ((findings nil))
    (dolist (line (split-string (or text "") "\n"))
      (when (string-match pearl-review--line-rx line)
        (push (list :file (match-string 1 line)
                    :line (string-to-number (match-string 2 line))
                    :column (if (match-string 3 line)
                                (string-to-number (match-string 3 line))
                              1)
                    :kind (intern (match-string 4 line))
                    :message (match-string 5 line))
              findings)))
    (nreverse findings)))

(defun pearl-review-fold-p (result)
  "Non-nil when the console may collapse RESULT to one line.
A result whose text names places folds, because those places are now
walkable somewhere else; a result with none does not, because its text is
the only copy."
  (and (pearl-review-parse (plist-get result :content)) t))

(defun pearl-review-locate (finding)
  "Return a marker at FINDING, or nil when its file is gone."
  (let ((file (expand-file-name (plist-get finding :file))))
    (when (file-readable-p file)
      (let ((buffer (find-file-noselect file t)))
        (with-current-buffer buffer
          (save-restriction
            (widen)
            (goto-char (point-min))
            (forward-line (1- (max 1 (plist-get finding :line))))
            (move-to-column (max 0 (1- (or (plist-get finding :column) 1))))
            (point-marker)))))))

(defun pearl-review-xref-backend ()
  "Return the backend findings are walked with, in a buffer with any."
  (when pearl-review--findings
    'pearl-review-xref-backend))

(cl-defmethod xref-backend-definitions ((_backend (eql pearl-review-xref-backend))
                                        identifier)
  "Return the findings that name IDENTIFIER, as xref items."
  (let ((items nil))
    (dolist (finding pearl-review--findings)
      (when (string-match-p (regexp-quote identifier)
                            (or (plist-get finding :message) ""))
        (push (xref-make (plist-get finding :message)
                         (xref-make-file-location
                          (expand-file-name (plist-get finding :file))
                          (plist-get finding :line)
                          (1- (or (plist-get finding :column) 1))))
              items)))
    (nreverse items)))

(defun pearl-review-next-error (arg reset)
  "Move to the ARGth finding after the last one, or the first when RESET."
  (let* ((count (length pearl-review--findings))
         (index (if reset
                    (1- (or arg 1))
                  (+ pearl-review--position (1- (or arg 1))))))
    (when (and (< 0 count) (<= 0 index) (< index count))
      (setq pearl-review--position index)
      (let* ((finding (nth index pearl-review--findings))
             (marker (pearl-review-locate finding)))
        (when marker
          (xref-push-marker-stack)
          (switch-to-buffer (marker-buffer marker))
          (goto-char marker)
          (recenter))))))

(provide 'pearl-review)

;;; pearl-review.el ends here
