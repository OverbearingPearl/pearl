;;; session-jsonl-tests.el --- stories about the record on disk  -*- lexical-binding: t; -*-

;;; Commentary:

;; An appended entry has to still be there when Emacs is not, and a
;; half-written tail is a fact to report rather than to skip.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-session.jsonl")) (store (pearl-session-jsonl-open path :create t)) (entry (pearl-contract-entry 'message '(:content "hi")))))
  (:when (progn (pearl-session-jsonl-append store entry) (pearl-session-jsonl-close store) (setq store (pearl-session-jsonl-open path))))
  (:then (equal 1 (length (pearl-session-jsonl-read store))))
  (:then (equal "hi" (plist-get (car (pearl-session-jsonl-read store)) :content))))

(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-bad.jsonl")) (store nil)))
  (:when (progn (with-temp-file path (insert "{not json}\n")) (setq store (pearl-session-jsonl-open path))))
  (:then (should-error (pearl-session-jsonl-read store))))
(provide 'session-jsonl-tests)

;;; session-jsonl-tests.el ends here
