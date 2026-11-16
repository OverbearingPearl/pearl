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
;; command that opens a console and the mode that console uses; everything
;; else lives in `lisp/', in three layers the check suite enforces (ADR 12):
;;
;;   core      contract, event, loop, session, tool, provider
;;   adapters  provider-sse, proc, session-jsonl
;;   product   console, tools-read, tools-edit, tools-shell, undo
;;
;; A user's entry point is `pearl': the rest is reached from the console.
;;
;; Planned API
;;
;; TODO `pearl' (ROOT) -> console buffer
;;      Ask for ROOT, open a console anchored there, and leave the model
;;      choice to the console's own commands.  Signals a `user-error' naming
;;      every missing external program at once rather than one per attempt.
;;
;; TODO `pearl-console-mode' -> autoloaded mode definition
;;      Defined in `pearl-console', autoloaded here, because a mode a user
;;      enables by hand has to exist before the file is loaded.

;;; Code:

;;;###autoload
(defun pearl (root)
  "Start a PEARL agent session in ROOT.
ROOT is a directory; the console is anchored there and every relative
path the agent uses is resolved against it."
  (interactive "DOpen PEARL console in: ")
  (let ((default-directory (file-name-as-directory (expand-file-name root))))
    (message "PEARL is not implemented yet: %s" default-directory)))

(provide 'pearl)

;;; pearl.el ends here
