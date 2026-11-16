;;; pearl-proc.el --- one bounded subprocess run  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; Everything that starts a process goes through here, so everything that
;; starts a process has a deadline, an output ceiling and a way to be killed.
;; A command that waits for a terminal, a pager or a lock would otherwise stop
;; Emacs itself for as long as it waits.
;;
;; The waiting is done with `accept-process-output' in a loop rather than with
;; `call-process', which is what keeps timers, input and redisplay alive while
;; a command runs.  This module knows nothing about what a command means; the
;; shell tool decides that.
;;
;; Planned API
;;
;; TODO `pearl-proc-run' (PROGRAM ARGUMENTS &key DIRECTORY TIMEOUT
;;      OUTPUT-LIMIT INPUT) -> result plist
;;      Keys `:status', `:stdout', `:stderr', `:timeout', `:truncated'.  Output
;;      beyond OUTPUT-LIMIT is kept up to the limit and the rest is counted,
;;      not buffered; TIMEOUT kills the process and sets `:timeout'.
;;      Borrow: the earlier prototype's deadline-aware runner, which already
;;      handled the stalled-mount case by degrading to a timeout.
;;
;; TODO `pearl-proc-check' (RESULT) -> RESULT
;;      Signals `pearl-proc-failed' with the exit status or the limit that
;;      fired.  Callers that want to report rather than signal do not call it.
;;
;; TODO `pearl-proc-kill' (PROCESS) -> nil
;;      Kill without prompting, and reap the buffer.

;;; Code:

(provide 'pearl-proc)

;;; pearl-proc.el ends here
