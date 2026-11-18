;;; contract-tests.el --- stories about the message shapes  -*- lexical-binding: t; -*-

;;; Commentary:

;; A tool call, a message and a request have to be one shape each, because
;; every other module trusts them.  What a vendor calls the fields is not
;; here: the adapter converts at the edge, and its own stories cover that.

;;; Code:

(require 'support-tests)
(require 'pearl-contract)
(require 'pearl-tool)
(require 'ert-gwt)

;; A request carries the conversation and the tool list and says nothing
;; about the wire, so a second vendor is an adapter and not a core change.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'user "hello")) (request nil)))
  (:when (setq request (pearl-contract-request (list message))))
  (:then (pearl-contract-request-p request))
  (:then (equal "hello" (pearl-contract-text (car (plist-get request :messages)))))
  (:then (null (plist-get request :model))))

(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'assistant "all done")) (text nil)))
  (:when (setq text (pearl-contract-text message)))
  (:then (equal "all done" text))
  (:then (eq 'assistant (plist-get message :role))))

(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-2" "edit" nil)) (result nil)))
  (:when (setq result (pearl-contract-tool-result call "ok")))
  (:then (equal "call-2" (plist-get (plist-get result :call) :id)))
  (:then (equal "ok" (plist-get result :content))))

;; A message with no content is not a message, and asking for its text is
;; an error rather than an empty string a caller would render as a turn.
(ert-gwt-deftest
  (:given ((message nil)))
  (:when (setq message (pearl-contract-message 'assistant nil)))
  (:then (should-not (pearl-contract-message-p message)))
  (:then (should-error (pearl-contract-text message))))

;; Content parts concatenate: the console and the projection read one text,
;; whatever shape the provider sent it in.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'assistant '((:type text :text "a") (:type text :text "b"))))
           (text nil)))
  (:when (setq text (pearl-contract-text message)))
  (:then (equal "ab" text)))

;; An error result is marked, so a transcript reader never mistakes a
;; failure for an answer.
(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-3" "read" nil)) (result nil)))
  (:when (setq result (pearl-contract-tool-result call "boom" t)))
  (:then (plist-get result :error))
  (:then (equal "boom" (plist-get result :content)))
  (:then (equal "call-3" (plist-get (plist-get result :call) :id))))

;; An entry names its kind and carries its patch; a conversational entry
;; carries none, because rollback must not depend on empty patches.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (message-entry (pearl-contract-entry 'message '(:content "hi")))
           (tool-entry nil)))
  (:when (setq tool-entry (pearl-contract-entry 'tool '(:name "edit") nil patch)))
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

;; An undo is a kind of entry: the record a rollback appends is shaped
;; like every other record, not a special case outside the vocabulary.
(ert-gwt-deftest
  (:given ((entry nil)))
  (:when (setq entry (pearl-contract-entry 'undo '(:reason "user rollback"))))
  (:then (eq 'undo (plist-get entry :kind)))
  (:then (equal "user rollback" (plist-get (plist-get entry :data) :reason)))
  (:then (null (plist-get entry :patch))))

;; The endpoint sends arguments as text, not as a shape: one function parses
;; it, and everything above that point sees an alist.
(ert-gwt-deftest
  (:given ((arguments nil)))
  (:when (setq arguments (pearl-contract-parse-arguments
                          "{\"path\":\"a.el\",\"line\":3}")))
  (:then (equal "a.el" (alist-get 'path arguments)))
  (:then (equal 3 (alist-get 'line arguments))))

;; A call that takes nothing sends the empty string, and an empty list comes
;; back: the common case must not look like a parse failure.
(ert-gwt-deftest
  (:given ((arguments nil)))
  (:when (setq arguments (pearl-contract-parse-arguments "")))
  (:then (null arguments)))

;; Half an object, an array and a bare number are all the same fault, named,
;; so the call can fail as a result instead of ending the turn.
(ert-gwt-deftest
  (:given ((truncated nil) (array nil) (scalar nil)))
  (:when (progn
           (setq truncated (condition-case err
                               (progn (pearl-contract-parse-arguments "{\"path\":") nil)
                             (pearl-tool-arguments-invalid (car err))))
           (setq array (condition-case err
                           (progn (pearl-contract-parse-arguments "[1,2]") nil)
                         (pearl-tool-arguments-invalid (car err))))
           (setq scalar (condition-case err
                            (progn (pearl-contract-parse-arguments "3") nil)
                          (pearl-tool-arguments-invalid (car err))))))
  (:then (eq 'pearl-tool-arguments-invalid truncated))
  (:then (eq 'pearl-tool-arguments-invalid array))
  (:then (eq 'pearl-tool-arguments-invalid scalar)))

(provide 'contract-tests)

;;; contract-tests.el ends here
