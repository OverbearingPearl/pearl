;;; session-tests.el --- stories about the record  -*- lexical-binding: t; -*-

;;; Commentary:

;; What the model reads next is decided here: the active branch, the
;; projection, and a rollback plan scoped to a path.

;;; Code:

(require 'support-tests)
(require 'pearl-session)
(require 'pearl-contract)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (entry (pearl-contract-entry 'message '(:content "hi")))))
  (:when (pearl-session-append session entry))
  (:then (eq entry (pearl-session-head session))))

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (first (pearl-contract-entry 'message '(:content "one"))) (left (pearl-contract-entry 'message '(:content "two"))) (right (pearl-contract-entry 'message '(:content "three")))))
  (:when (progn (pearl-session-append session first) (pearl-session-append session left) (pearl-session-set-head session first) (pearl-session-append session right)))
  (:then (equal 3 (length (pearl-session-entries session))))
  (:then (equal 2 (length (pearl-session-branch session)))))

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (kept (pearl-contract-entry 'message '(:content "kept"))) (projected nil)))
  (:when (progn (pearl-session-append session kept) (pearl-session-append session (pearl-contract-entry 'message '(:content "other"))) (pearl-session-set-head session kept) (setq projected (pearl-session-projection session nil))))
  (:then (null (seq-find (lambda (message) (equal "other" (plist-get message :content))) projected))))

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b")) (plan nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil patch)) (pearl-session-append session (pearl-contract-entry 'message '(:content "hi"))) (setq plan (pearl-session-rollback-plan session))))
  (:then (equal '("a.el") (delete-dups (mapcar (lambda (p) (plist-get p :file)) plan)))))

(ert-gwt-deftest
  (:given ((empty-patch '(:tool "delete" :file "a.el" :old-text "" :new-text ""))))
  (:when (pearl-session-conflict-p empty-patch ""))
  (:then (eq t (pearl-session-conflict-p empty-patch ""))))

;; The plan reverses in reverse order: the last write on a path is the
;; first one taken back, because it was written against the one before it.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (first '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second '(:tool "edit" :file "a.el" :old-text "b" :new-text "c"))
           (plan nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil first)) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil second)) (setq plan (pearl-session-rollback-plan session))))
  (:then (equal 2 (length plan)))
  (:then (equal "c" (plist-get (car plan) :new-text)))
  (:then (equal "b" (plist-get (cadr plan) :new-text))))

;; Rollback is path-scoped to the active branch: a patch on an abandoned
;; branch is never in the plan, whatever the head once was.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (root-entry (pearl-contract-entry 'message '(:content "root")))
           (left-patch '(:tool "edit" :file "left.el" :old-text "a" :new-text "b"))
           (right-patch '(:tool "edit" :file "right.el" :old-text "a" :new-text "b"))
           (plan nil)))
  (:when (progn (pearl-session-append session root-entry) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil left-patch)) (pearl-session-set-head session root-entry) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil right-patch)) (setq plan (pearl-session-rollback-plan session))))
  (:then (equal '("right.el") (mapcar (lambda (p) (plist-get p :file)) plan))))

;; The plan stops at the entry it is given: taking a turn back never
;; reaches past the turn the user named.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (first-patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second-patch '(:tool "edit" :file "b.el" :old-text "a" :new-text "b"))
           (boundary nil) (plan nil)))
  (:when (progn (setq boundary (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil first-patch))) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil second-patch)) (setq plan (pearl-session-rollback-plan session boundary))))
  (:then (equal '("b.el") (mapcar (lambda (p) (plist-get p :file)) plan))))

;; Only an entry that changed something outside the session carries a
;; patch; a conversational entry carries none.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (tool-entry nil) (message-entry nil)))
  (:when (progn (setq tool-entry (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil patch))) (setq message-entry (pearl-session-append session (pearl-contract-entry 'message '(:content "hi"))))))
  (:then (equal patch (pearl-session-patch tool-entry)))
  (:then (null (pearl-session-patch message-entry))))

;; A patch whose written text is still in the file can be reversed: no
;; conflict, and the restore may proceed.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))))
  (:when (pearl-session-conflict-p patch "b"))
  (:then (not (pearl-session-conflict-p patch "b"))))

;; A file the user changed since the write is a conflict: the written text
;; is gone, so reversing the patch would guess.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))))
  (:when (pearl-session-conflict-p patch "the user was here"))
  (:then (eq t (pearl-session-conflict-p patch "the user was here"))))

;; The projection folds tool output a later turn has already read: a tool
;; entry with an assistant response after it projects as a short marker,
;; while the entry itself keeps the whole text.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (bulky (make-string 500 ?x))
           (tool-entry nil) (projected nil) (folded nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'message '(:content "go"))) (setq tool-entry (pearl-session-append session (pearl-contract-entry 'tool (list :name "echo" :id "c1" :content bulky)))) (pearl-session-append session (pearl-contract-entry 'message '(:content "read it"))) (pearl-session-append session (pearl-contract-entry 'message '(:content "again"))) (setq projected (pearl-session-projection session nil)) (setq folded (seq-find (lambda (message) (equal "c1" (plist-get message :id))) projected))))
  (:then folded)
  (:then (< (length (pearl-contract-text folded)) 500))
  (:then (equal bulky (plist-get (plist-get tool-entry :data) :content))))

;; The newest tool output, which no turn has read yet, stays whole: the
;; model must see what it has not seen.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (bulky (make-string 500 ?x))
           (projected nil) (whole nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'message '(:content "go"))) (pearl-session-append session (pearl-contract-entry 'tool (list :name "echo" :id "c1" :content bulky))) (setq projected (pearl-session-projection session nil)) (setq whole (seq-find (lambda (message) (equal "c1" (plist-get message :id))) projected))))
  (:then (equal bulky (pearl-contract-text whole))))

;; The shipped prompt follows the loadout: a tool the request does not carry
;; is not offered in prose, because prose is where the model decides to call
;; one.  It also states the rule the schema cannot, which is that asking for
;; permission is not how an edit proceeds.
(ert-gwt-deftest
  (:given ((tools (list (pearl-tool-define "read" '(:description "read") #'ignore)
                        (pearl-tool-define "edit" '(:description "edit") #'ignore)))
           (prompt (pearl-session-default-prompt (pearl-tool-declarations tools)))))
  (:when prompt)
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
           (session (pearl-session-new))
           (sections nil) (prompt nil))
          (pearl-session-set-prompt-sources
           session
           (list (list :name "user" :path "/home/u/.emacs.d/pearl/AGENTS.md"
                       :text "a user's rule")
                 (list :name "project" :path "/repo/AGENTS.md"
                       :text "a project's rule"))))
  (:when (progn
           (setq sections (pearl-session-prompt-sections
                           session (pearl-tool-declarations tools)))
           (setq prompt (pearl-session-prompt
                         session (pearl-tool-declarations tools)))))
  (:then (equal '("preamble" "user" "project")
                (mapcar (lambda (section) (plist-get section :name))
                        sections)))
  (:then (< (string-match "a user's rule" prompt)
            (string-match "a project's rule" prompt)))
  (:then (string-match-p "a project's rule\\'" prompt)))

;; A bare session has one section, the shipped text, because a source nobody
;; set cannot leave a hole where a section was expected.
(ert-gwt-deftest
  (:given ((sections (pearl-session-prompt-sections (pearl-session-new) nil))))
  (:when sections)
  (:then (equal '("preamble")
                (mapcar (lambda (section) (plist-get section :name))
                        sections))))

(provide 'session-tests)

;;; session-tests.el ends here
