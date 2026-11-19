;;; pearl.el --- Pi's agent loop in Emacs Lisp  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl

;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; Version: 0.0.1
;; Package-Requires: ((emacs "29.4"))
;; Keywords: tools, processes
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; PEARL is a Lisp-native implementation of Pi's agent loop: a minimal,
;; model-agnostic loop that calls tools and feeds results back.
;;
;; This file is the package entry point and nothing else.  It autoloads the
;; command that opens a console; everything else lives in `lisp/', in three
;; layers the check suite enforces (ADR 12):
;;
;;   core      contract, event, turn, prompt, loop, session, compaction, tool,
;;             provider
;;   adapters  provider-sse, proc, session-jsonl, tree
;;   product   app, console, tools-read, tools-edit, tools-shell, tools-git,
;;             delegate, review, undo
;;
;; A user's entry point is `pearl': the rest is reached from the console.

;;; Code:

(require 'pearl-app)

(defconst pearl--required-programs '("git")
  "The programs PEARL needs before a session starts.
All of them are looked for at once, because a dependency discovered one
request at a time is a dependency the user pays for twice.")

;;;###autoload
(defun pearl (root)
  "Start a PEARL agent session in ROOT.
ROOT is a directory; the console is anchored there and every relative
path the agent uses is resolved against it.  A missing external program
is named before anything is opened, and every missing one at once."
  (interactive "DOpen PEARL console in: ")
  (let* ((root (file-name-as-directory (expand-file-name root)))
         (missing (seq-remove #'executable-find pearl--required-programs)))
    (when missing
      (user-error "PEARL needs %s, which %s not on the path"
                  (mapconcat #'identity missing " and ")
                  (if (cdr missing) "are" "is")))
    (let* ((app (pearl-app-open root))
           (console (pearl-app-console app)))
      (pearl-app-wire console)
      console)))

(provide 'pearl)

;;; pearl.el ends here
