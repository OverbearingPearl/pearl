;;; prompt-tests.el --- stories about the text the model is given  -*- lexical-binding: t; -*-

;;; Commentary:

;; A prompt that offers a tool the loop cannot call is a prompt that makes the
;; model guess, and two mechanisms for adding text is one too many.  These
;; stories are about the order and about the loadout, not about the words.

;;; Code:

(require 'support-tests)
(require 'pearl-prompt)
(require 'pearl-tool)
(require 'ert-gwt)

;; The shipped prompt follows the loadout: a tool it does not carry is not
;; offered in prose, because prose is where the model decides to call one.  It
;; also states the rule the schema cannot, which is that asking for permission
;; is not how an edit proceeds.
(ert-gwt-deftest
  (:given ((tools (list (pearl-tool-define "read" '(:description "read") #'ignore)
                        (pearl-tool-define "edit" '(:description "edit") #'ignore)))
           (prompt nil)))
  (:when (setq prompt (pearl-prompt-default (pearl-tool-declarations tools))))
  (:then (string-match-p "read" prompt))
  (:then (string-match-p "edit" prompt))
  (:then (not (string-match-p "shell" prompt)))
  (:then (string-match-p "undo" prompt)))

;; The prompt's order is the code's, not the call site's: what PEARL ships,
;; then the user's file, then the project's, and nothing else can slip between
;; them, because sources are the only way in (ADR 21).
(ert-gwt-deftest
  (:given ((tools (list (pearl-tool-define "read" '(:description "read")
                                           #'ignore)))
           (sources (list (list :name "user" :path "/home/u/.emacs.d/pearl/AGENTS.md"
                                :text "a user's rule")
                          (list :name "project" :path "/repo/AGENTS.md"
                                :text "a project's rule")))
           (declarations (pearl-tool-declarations tools))
           (sections nil) (prompt nil)))
  (:when (setq sections (pearl-prompt-sections sources declarations)
               prompt (pearl-prompt sources declarations)))
  (:then (equal '("preamble" "user" "project")
                (mapcar (lambda (section) (plist-get section :name))
                        sections)))
  (:then (< (string-match "a user's rule" prompt)
            (string-match "a project's rule" prompt)))
  (:then (string-match-p "a project's rule\\'" prompt)))

;; A bare session has one section, the shipped text, because a source nobody
;; set cannot leave a hole where a section was expected.
(ert-gwt-deftest
  (:given ((sections nil)))
  (:when (setq sections (pearl-prompt-sections nil nil)))
  (:then (equal '("preamble")
                (mapcar (lambda (section) (plist-get section :name))
                        sections)))
  (:then (null (plist-get (car sections) :path))))

(provide 'prompt-tests)

;;; prompt-tests.el ends here
