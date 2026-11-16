;;; contract-tests.el --- stories about the message shapes  -*- lexical-binding: t; -*-

;;; Commentary:

;; A tool call and a message have to survive the trip to a provider and
;; back, because every other module trusts these shapes.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-1" "read" (quote ((path . "pearl.el"))))) (wire nil) (back nil)))
  (:when (setq wire (pearl-contract-to-wire (list call)) back (pearl-contract-from-wire (car wire))))
  (:then (equal "call-1" (plist-get back :id)))
  (:then (equal "read" (plist-get back :name)))
  (:then (equal "pearl.el" (alist-get (quote path) (plist-get back :arguments)))))

(ert-gwt-deftest
  (:given ((message (pearl-contract-message (quote assistant) "all done"))))
  (:when (pearl-contract-message-p message))
  (:then (equal "all done" (pearl-contract-text message))))

(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-2" "edit" nil)) (result (pearl-contract-tool-result call "ok"))))
  (:when (plist-get result :call))
  (:then (equal "call-2" (plist-get (plist-get result :call) :id))))

(ert-gwt-deftest
  (:given ((message (pearl-contract-message (quote assistant) nil))))
  (:when (pearl-contract-text message))
  (:then (should-error (pearl-contract-message-p message))))
(provide 'contract-tests)

;;; contract-tests.el ends here
