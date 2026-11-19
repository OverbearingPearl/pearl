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
;; `pearl-proc-run' (PROGRAM ARGUMENTS &key DIRECTORY TIMEOUT
;;      OUTPUT-LIMIT INPUT) -> result plist
;;      Keys `:status', `:stdout', `:stderr', `:timeout', `:truncated'.  Output
;;      beyond OUTPUT-LIMIT is kept up to the limit and the rest is counted,
;;      not buffered; TIMEOUT kills the process and sets `:timeout'.
;;      Borrow: scalpel's `scalpel-proc`, which already handled the
;;      stalled-mount case by degrading to a timeout and captured diagnostics
;;      when a command misbehaved.  Scaffolding: the body that lands here
;;      replaces this comment.
;;
;; `pearl-proc-check' (RESULT) -> RESULT
;;      Signals `pearl-proc-failed' with the exit status or the limit that
;;      fired.  Callers that want to report rather than signal do not call it.
;;
;; `pearl-proc-kill' (PROCESS) -> nil
;;      Kill without prompting, and reap the buffer.

;; `pearl-proc-run-async' (PROGRAM ARGUMENTS &key DIRECTORY TIMEOUT
;;      OUTPUT-LIMIT INPUT CALLBACK) -> process
;;      The same run, without the wait: CALLBACK receives the result plist
;;      when the process ends or the deadline fires, and the caller keeps
;;      Emacs.  Cancelling is `pearl-proc-kill' on the process it returns.

;;; Code:

(require 'cl-lib)
(require 'seq)

(define-error 'pearl-proc-failed "A command failed")

(defconst pearl-proc-timeout 600
  "How long a command may run when the caller names no deadline.")

(defconst pearl-proc-output-limit 1000000
  "How much output is kept when the caller names no ceiling.")

(defvar pearl-proc--runs 0
  "How many commands this Emacs has started, so names stay distinct.")

(defun pearl-proc--silence (process)
  "Give the stderr pipe of PROCESS a silent sentinel.

The pipe is a process of its own, and its default sentinel writes
`Process ... stderr finished' into the buffer a caller reads as the
command's diagnostics."
  (let ((pipe (seq-find (lambda (candidate)
                          (string-prefix-p (concat (process-name process) " stderr")
                                           (process-name candidate)))
                        (process-list))))
    (when pipe (set-process-sentinel pipe #'ignore))))

(defvar-local pearl-proc--ceiling nil
  "How much output the run owning this buffer keeps.")

(defvar-local pearl-proc--filled nil
  "Non-nil once this buffer has reached its ceiling.")

(defun pearl-proc--filled-p (out)
  "Whether OUT reached the ceiling of the run that owns it."
  (and (buffer-live-p out) (buffer-local-value 'pearl-proc--filled out)))

(defun pearl-proc--filter (process string)
  "Insert STRING into PROCESS's buffer up to its ceiling.
The ceiling and the fill are buffer-local, so one filter serves every run
and a caller reads the flag without carrying a closure around."
  (let ((out (process-buffer process)))
    (when (buffer-live-p out)
      (with-current-buffer out
        (let ((room (- pearl-proc--ceiling (buffer-size))))
          (when (> room 0)
            (insert (substring string 0 (min room (length string)))))
          (when (>= (buffer-size) pearl-proc--ceiling)
            (setq pearl-proc--filled t)))))))

(defun pearl-proc--text (buffer)
  "Return BUFFER's contents, or the empty string once BUFFER is gone."
  (if (buffer-live-p buffer)
      (with-current-buffer buffer (buffer-string))
    ""))

(cl-defun pearl-proc--start (program arguments directory limit)
  "Start PROGRAM with ARGUMENTS in DIRECTORY under LIMIT.
Return (PROCESS OUT ERR).  The two buffers belong to the run and OUT
remembers LIMIT and whether it was reached, so the caller reads the
truncation instead of capturing it."
  (let ((out (generate-new-buffer " *pearl-proc-out*"))
        (err (generate-new-buffer " *pearl-proc-err*")))
    (with-current-buffer out
      (setq-local pearl-proc--ceiling limit))
    (let ((process
           ;; `make-process' documents a `:directory' argument and ignores
           ;; it for a local command on Emacs 29, so the working directory
           ;; is the one the call happens in.
           (let ((default-directory (or directory default-directory)))
             (make-process
              :name (format "pearl-proc-%d" (cl-incf pearl-proc--runs))
              :filter #'pearl-proc--filter
              :command (cons program arguments)
              :buffer out
              :stderr err
              :noquery t
              :connection-type 'pipe
              :coding 'utf-8-unix
              :sentinel #'ignore))))
      (pearl-proc--silence process)
      (list process out err))))

(cl-defun pearl-proc-run (program arguments &key directory timeout
                                          output-limit input)
  "Run PROGRAM with ARGUMENTS in DIRECTORY and return what it did.

The run is bounded by TIMEOUT and OUTPUT-LIMIT, and INPUT is written to
the process's stdin before it is closed."
  (let* ((timeout (or timeout pearl-proc-timeout))
         (limit (or output-limit pearl-proc-output-limit))
         (started (float-time))
         (timed-out nil))
    (cl-destructuring-bind (process out err)
        (pearl-proc--start program arguments directory limit)
      (unwind-protect
          (progn
            (when input
              (process-send-string process input))
            (process-send-eof process)
            ;; `sleep-for' rather than `accept-process-output': the latter
            ;; drains the pipe before it looks at the clock, and a command
            ;; that never stops writing never gives it the chance.
            (while (and (process-live-p process)
                        (not (pearl-proc--filled-p out))
                        (< (- (float-time) started) timeout))
              (sleep-for 0.005))
            ;; A command that fills the ceiling has said all it is going to
            ;; be asked to keep, and letting it keep writing is how a bounded
            ;; run turns into an unbounded wait.
            (when (and (pearl-proc--filled-p out) (process-live-p process))
              (delete-process process))
            (when (process-live-p process)
              (setq timed-out t)
              (delete-process process))
            (accept-process-output process 0.05)
            (while (process-live-p process) (accept-process-output nil 0.01))
            (list :status (if timed-out 1 (process-exit-status process))
                  :stdout (pearl-proc--text out)
                  :stderr (pearl-proc--text err)
                  :timeout timed-out
                  :truncated (pearl-proc--filled-p out)))
        (when (process-live-p process) (delete-process process))
        (when (buffer-live-p out) (kill-buffer out))
        (when (buffer-live-p err) (kill-buffer err))))))

(cl-defun pearl-proc-run-async (program arguments &key directory timeout
                                                    output-limit input callback)
  "Run PROGRAM with ARGUMENTS in DIRECTORY and hand the result to CALLBACK.
The result has the same shape `pearl-proc-run' returns, bounded by TIMEOUT
and OUTPUT-LIMIT and fed INPUT, so a caller can treat the two runs the same
way.  The deadline is a timer, and a command that fills its ceiling is
killed as it fills it, so a caller that never hears from the command still
gets an answer."
  (let* ((limit (or output-limit pearl-proc-output-limit))
         (timeout (or timeout pearl-proc-timeout))
         (finished nil)
         (deadline nil))
    (cl-destructuring-bind (process out err)
        (pearl-proc--start program arguments directory limit)
      (let ((finish (lambda (&optional timed)
                      (unless finished
                        (setq finished t)
                        (when deadline (cancel-timer deadline))
                        (accept-process-output process 0.05)
                        (let ((status (if timed 1 (process-exit-status process))))
                          (let ((result (list :status status
                                              :stdout (pearl-proc--text out)
                                              :stderr (pearl-proc--text err)
                                              :timeout timed
                                              :truncated
                                              (pearl-proc--filled-p out))))
                            (when (buffer-live-p out) (kill-buffer out))
                            (when (buffer-live-p err) (kill-buffer err))
                            (funcall callback result)))))))
        (set-process-sentinel
         process
         (lambda (live _event)
           (unless (process-live-p live)
             (funcall finish nil))))
        (when input (process-send-string process input))
        (process-send-eof process)
        (setq deadline (run-at-time timeout nil
                                    (lambda ()
                                      (when (process-live-p process)
                                        (delete-process process)))))
        process))))

(defun pearl-proc-check (result)
  "RESULT when it succeeded, or a `pearl-proc-failed' signal naming why."
  (if (and (equal 0 (plist-get result :status))
           (null (plist-get result :timeout)))
      result
    (signal 'pearl-proc-failed
            (list :status (plist-get result :status)
                  :timeout (plist-get result :timeout)
                  :stderr (plist-get result :stderr)))))

(defun pearl-proc-kill (process)
  "Kill PROCESS without asking, and leave no buffer behind."
  (let ((buffer (process-buffer process)))
    (when (process-live-p process)
      (delete-process process))
    (when (and buffer (buffer-live-p buffer))
      (kill-buffer buffer))
    (when (process-buffer process)
      (set-process-buffer process nil)))
  nil)

(provide 'pearl-proc)

;;; pearl-proc.el ends here
