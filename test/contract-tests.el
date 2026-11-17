;;; contract-tests.el --- stories about the message shapes  -*- lexical-binding: t; -*-

;;; Commentary:

;; A tool call and a message have to survive the trip to a provider and
;; back, because every other module trusts these shapes.

;;; Code:

(require 'pearl-support-tests)
(require 'pearl-contract)
(require 'pearl-tool)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-1" "read" '((path . "pearl.el")))) (wire nil) (back nil)))
  (:when (setq wire (pearl-contract-to-wire (list call)) back (pearl-contract-from-wire (car wire))))
  (:then (equal "call-1" (plist-get back :id)))
  (:then (equal "read" (plist-get back :name)))
  (:then (equal "pearl.el" (alist-get 'path (plist-get back :arguments)))))

(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'assistant "all done"))))
  (:when (pearl-contract-message-p message))
  (:then (equal "all done" (pearl-contract-text message))))

(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-2" "edit" nil)) (result (pearl-contract-tool-result call "ok"))))
  (:when (plist-get result :call))
  (:then (equal "call-2" (plist-get (plist-get result :call) :id))))

;; A message with no content is not a message, and asking for its text is
;; an error rather than an empty string a caller would render as a turn.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'assistant nil))))
  (:when (pearl-contract-message-p message))
  (:then (not (pearl-contract-message-p message)))
  (:then (should-error (pearl-contract-text message))))

;; The wire trip preserves a message, not just a tool call: role and text
;; come back as they went in, through the one conversion point.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'user "hello")) (back nil)))
  (:when (setq back (pearl-contract-from-wire (car (pearl-contract-to-wire (list message))))))
  (:then (eq 'user (plist-get back :role)))
  (:then (equal "hello" (pearl-contract-text back))))

;; Content parts concatenate: the console and the projection read one text,
;; whatever shape the provider sent it in.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'assistant '((:type text :text "a") (:type text :text "b"))))))
  (:when (pearl-contract-text message))
  (:then (equal "ab" (pearl-contract-text message))))

;; An error result is marked, so a transcript reader never mistakes a
;; failure for an answer.
(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-3" "read" nil)) (result (pearl-contract-tool-result call "boom" t))))
  (:when (plist-get result :error))
  (:then (plist-get result :error))
  (:then (equal "boom" (plist-get result :content))))

;; An entry names its kind and carries its patch; a conversational entry
;; carries none, because rollback must not depend on empty patches.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (tool-entry (pearl-contract-entry 'tool '(:name "edit") nil patch))
           (message-entry (pearl-contract-entry 'message '(:content "hi")))))
  (:when (list tool-entry message-entry))
  (:then (eq 'tool (plist-get tool-entry :kind)))
  (:then (equal patch (plist-get tool-entry :patch)))
  (:then (null (plist-get message-entry :patch))))

;; A declaration is what a provider is told: the name and the description,
;; in list order, from the tool itself.
(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" '(:description "read a file") #'ignore)) (declaration nil)))
  (:when (setq declaration (pearl-contract-tool-declaration tool)))
  (:then (equal "read" (plist-get declaration :name)))
  (:then (equal "read a file" (plist-get declaration :description))))

(provide 'contract-tests)

;;; contract-tests.el ends here
