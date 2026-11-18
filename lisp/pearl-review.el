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
;; `pearl-review-install' (CONSOLE) -> unsubscribe function
;;      Subscribe to `tool-execution-end', parse the result, and keep the
;;      findings in the console buffer, so `next-error' has a list to walk.
;;      Returns a function that removes the subscription and the hooks.
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
(require 'pearl-console)
(require 'pearl-event)

(defconst pearl-review--line-rx
  "^[ \t]*\\([^:\n]+\\):\\([0-9]+\\)\\(?::\\([0-9]+\\)\\)?[: \t]+\\(error\\|warning\\|note\\):[: \t]+\\(.*\\)$"
  "A line that names a place, in the shape compilers emit.")

(defvar-local pearl-review--findings nil
  "The findings the tool output in this buffer named.
Buffer-local, because two consoles walk two builds.")

(defvar-local pearl-review--position 0
  "How far through the findings the last navigation went.")

(defvar-local pearl-review--hooked nil
  "Non-nil when this buffer already walks its findings.")

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
  "Return a marker at FINDING, or nil when its file is gone.
The file's buffer is left current, because whoever asked for the
location is about to look at it."
  (let ((file (expand-file-name (plist-get finding :file))))
    (when (file-readable-p file)
      (set-buffer (find-file-noselect file t))
      (save-restriction
        (widen)
        (goto-char (point-min))
        (forward-line (1- (max 1 (plist-get finding :line))))
        (move-to-column (max 0 (1- (or (plist-get finding :column) 1))))
        (point-marker)))))

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
                          ;; `xref-make-file-location' lands one line
                          ;; further down than the number it is given
                          ;; (Emacs 29.4), so a finding on line 12 is
                          ;; entered as 11.
                          (1- (plist-get finding :line))
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
          ;; `next-error' calls this inside `with-current-buffer' of the
          ;; console, so the buffer variable is put back whatever we do;
          ;; what stays is the selected window, and that is what the user
          ;; sees the file in.
          (select-window (display-buffer (marker-buffer marker)))
          (switch-to-buffer (marker-buffer marker))
          (goto-char marker)
          (set-window-point (selected-window) marker)
          (recenter))))))

(defun pearl-review--take (console result)
  "Take the findings RESULT named into CONSOLE's list.
A result with no place in it leaves the list alone: a build that printed
nothing new must not erase what it printed last time."
  (let ((findings (pearl-review-parse (plist-get result :content))))
    (when findings
      (with-current-buffer console
        (setq pearl-review--findings (append findings pearl-review--findings))))))

(defun pearl-review-install (console)
  "Walk the places CONSOLE's tool output names, and return the way out.
The list of findings lives in the console buffer, so two consoles walking
two builds do not read each other's; the return value removes what this
added and kills nothing the user can see."
  (let ((loop (pearl-console-loop console))
        (unsubscribe nil))
    (when loop
      (setq unsubscribe
            (pearl-event-subscribe
             (pearl-loop-emitter loop)
             (lambda (event)
               (when (eq (plist-get event :kind) 'tool-execution-end)
                 (pearl-review--take console
                                    (plist-get (plist-get event :data) :result)))))))
    (with-current-buffer console
      (setq pearl-review--findings nil
            pearl-review--position 0)
      ;; A console with no findings has nothing to walk, so the hooks go
      ;; in only once one exists: `next-error' would otherwise offer an
      ;; empty list, and `xref-find-backend' would ask which backend to
      ;; use in a buffer that has one.
      (unless pearl-review--hooked
        (setq pearl-review--hooked t
              ;; `next-error-function' holds one function, so it is set
              ;; rather than added to; `add-hook' would store a list and
              ;; `next-error' would call it as one.
              next-error-function #'pearl-review-next-error)
        (add-hook 'xref-backend-functions #'pearl-review-xref-backend nil t)))
    (lambda ()
      (when unsubscribe (funcall unsubscribe))
      (when (buffer-live-p console)
        (with-current-buffer console
          (setq pearl-review--findings nil
                pearl-review--position 0
                pearl-review--hooked nil)
          (setq next-error-function nil)
          (remove-hook 'xref-backend-functions #'pearl-review-xref-backend t))))))

(provide 'pearl-review)

;;; pearl-review.el ends here
