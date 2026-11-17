;;; pearl-tools-shell.el --- the shell tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The tool whose effects cannot be taken back.  A command can push a branch,
;; send a request or write outside the project, and no snapshot undoes that, so
;; this is the one place the user is asked first (ADR 5).
;;
;; The question is asked where it can be answered: a command that provably
;; writes only inside the project root, to paths the turn snapshot covers,
;; runs without a prompt (ADR 16), and a command whose write scope cannot be
;; established is confirmed.  Confirmation is not a sandbox and the status
;; line says which regime is active, so nobody has to guess which one they
;; are in.
;;
;; Planned API
;;
;; TODO `pearl-tools-shell-declaration' -> tool declaration
;;      `command' and `reason', as in Pi (ADR 8).  The reason is shown in the
;;      confirmation prompt, which is the only thing that makes the prompt
;;      answerable.
;;
;; TODO `pearl-tools-shell-run' (ARGUMENTS CONTEXT) -> tool result
;;      Resolve the write scope, ask when it cannot be proved, run through
;;      `pearl-proc-run', and return bounded output with the exit status.
;;      Never signals; a refused confirmation is a result the model can read.
;;
;; TODO `pearl-tools-shell-scope' (COMMAND ROOT) -> scope or nil
;;      What the command can write, as a list of paths, or nil when it cannot
;;      be established.  Nil is not an error: it selects the confirming
;;      regime.  A write to a gitignored path cannot be established even
;;      inside the root, and outside a git repository no write is proved at
;;      all: the snapshot is what makes a write revertible, and the regime
;;      asks exactly where it ends (ADR 16).  A command that writes nothing
;;      is proved everywhere.
;;
;; TODO `pearl-tools-shell-confirm' (COMMAND SCOPE REASON) -> boolean
;;      Ask the user, showing the command, its scope and the model's reason.
;;      Returns nil without asking when the buffer is in unattended mode, in
;;      which case the caller refuses instead of assuming yes.
;;
;; TODO `pearl-tools-shell-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

(provide 'pearl-tools-shell)

;;; pearl-tools-shell.el ends here
