;;; proc-tests.el --- stories about a bounded command  -*- lexical-binding: t; -*-

;;; Commentary:

;; A command that waits for a terminal would otherwise wait for the
;; editor to die first, so every run has a deadline and a ceiling.

;;; Code:

(require 'pearl-support-tests)
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
(provide 'proc-tests)

;;; proc-tests.el ends here
