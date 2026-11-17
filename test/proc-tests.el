;;; proc-tests.el --- stories about a bounded command  -*- lexical-binding: t; -*-

;;; Commentary:

;; A command that waits for a terminal would otherwise wait for the
;; editor to die first, so every run has a deadline and a ceiling.

;;; Code:

(require 'support-tests)
(require 'pearl-proc)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "sleep" (list "5") :timeout 0.2)))
  (:then (plist-get result :timeout)))

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "yes" nil :timeout 1 :output-limit 100)))
  (:then (plist-get result :truncated))
  (:then (<= (length (plist-get result :stdout)) 100)))

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "sh" (list "-c" "echo hello"))))
  (:then (equal "hello\n" (plist-get result :stdout))))

;; A command's diagnostics are captured, not lost: stderr comes back
;; beside stdout, whatever the exit status.
(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "sh" (list "-c" "echo oops >&2"))))
  (:then (equal "oops\n" (plist-get result :stderr)))
  (:then (equal 0 (plist-get result :status))))

;; A nonzero exit is a status in the result, not a signal: the shell tool
;; decides what a failure means, not this module.
(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "sh" (list "-c" "exit 3"))))
  (:then (equal 3 (plist-get result :status)))
  (:then (null (plist-get result :timeout))))

;; The command runs where it was told to: the directory key is the
;; working directory, not a guess at the caller's.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (result nil)))
  (:when (setq result (pearl-proc-run "pwd" nil :directory root)))
  (:then (equal (file-truename root) (file-truename (string-trim (plist-get result :stdout))))))

;; Input reaches the command: a program that reads stdin is not left
;; waiting for a terminal.
(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-proc-run "cat" nil :input "hi\n" :timeout 2)))
  (:then (equal "hi\n" (plist-get result :stdout))))

;; Check is for callers that want a signal: a failed run signals by name,
;; carrying the status.
(ert-gwt-deftest
  (:given ((result (pearl-proc-run "sh" (list "-c" "exit 3")))))
  (:when (pearl-proc-check result))
  (:then (should-error (pearl-proc-check result) :type 'pearl-proc-failed)))

;; A clean run passes the check unchanged.
(ert-gwt-deftest
  (:given ((result (pearl-proc-run "sh" (list "-c" "true")))))
  (:when (pearl-proc-check result))
  (:then (eq result (pearl-proc-check result))))

(provide 'proc-tests)

;;; proc-tests.el ends here
