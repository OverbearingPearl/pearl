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

;; A turn that only ran commands has no patch to reverse, so its plan cannot
;; be made of patches: it names the paths the command's scope proved and asks
;; the snapshot to restore them (ADR 23).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (path "/repo/out.txt")
           (entry (pearl-contract-entry
                   'tool (list :name "shell" :scope (list path) :content "ok")))
           (plan nil)))
  (:when (progn (pearl-session-append session entry)
                (setq plan (pearl-session-rollback-plan session))))
  (:then (equal (list (list :restore path)) plan)))

;; A command that scribbled outside the snapshot is taken back from its own
;; pre-image, and the plan says which kind of step that is, so the console can
;; tell "this line comes back" from "this file comes back" from "this write was
;; captured before it happened" (ADR 25).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (path "/tmp/outside.txt")
           (entry (pearl-contract-entry
                   'tool (list :name "shell"
                               :scope (list path)
                               :preimage (list :id "p1")
                               :content "ok")))
           (plan nil)))
  (:when (progn (pearl-session-append session entry)
                (setq plan (pearl-session-rollback-plan session))))
  (:then (equal (list (list :preimage path)) plan)))

;; Two projections of an unchanged branch are the same list.  A projection that
;; recomputed its head every turn would be correct and would pay full price on
;; every request, because the provider caches the prefix (ADR 27).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (first nil) (second nil))
          (dolist (text '("one" "two"))
            (pearl-session-append
             session (pearl-contract-entry 'message (list :content text))))
          (pearl-session-append
           session (pearl-contract-entry
                    'tool (list :name "read" :id "c1"
                                :content (make-string 400 ?x))))
          (pearl-session-append
           session (pearl-contract-entry 'message '(:content "read it"))))
  (:when (progn
           (setq first (pearl-session-projection session nil))
           (setq second (pearl-session-projection session nil))))
  (:then (equal first second)))

;; The record becomes a session again, and the session it becomes is the one
;; that wrote it: same branch, same head, same projection.  Resuming is this
;; function and the adapter that fed it (ADR 29).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (entries nil) (restored nil) (before nil))
          (dolist (text '("one" "two" "three"))
            (pearl-session-append
             session (pearl-contract-entry 'message (list :content text))))
          (setq before (pearl-session-projection session nil))
          (setq entries (pearl-session-entries session)))
  (:when (setq restored (pearl-session-restore entries)))
  (:then (equal (pearl-session-branch restored) (pearl-session-branch session)))
  (:then (equal (pearl-session-projection restored nil) before)))

;; Kill Emacs, reopen, and the conversation continues rather than merely being
;; readable: what the file holds is enough to say what the model saw last.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (path nil) (store nil) (session nil)
           (reopened nil))
          (setq path (pearl-session-jsonl-path-for root "s1"))
          (setq store (pearl-session-jsonl-open path :create t :lock t))
          (setq session (pearl-session-new))
          (dolist (text '("one" "two" "three"))
            (pearl-session-jsonl-append
             store (pearl-session-append
                    session (pearl-contract-entry 'message (list :content text)))))
          (pearl-session-jsonl-close store))
  (:when (setq reopened (pearl-session-restore
                         (pearl-session-jsonl-read
                          (pearl-session-jsonl-open path)))))
  (:then (equal (pearl-session-branch reopened) (pearl-session-branch session)))
  (:then (equal (pearl-session-head reopened) (pearl-session-head session))))

;; The head can be nothing, which is what re-editing the first message means:
;; the second attempt is a sibling and the first stays in the record (ADR 29).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)))
          (pearl-session-append
           session (pearl-contract-entry 'message '(:content "first")))
          (pearl-session-append
           session (pearl-contract-entry 'message '(:content "second")))
          (pearl-session-set-head session nil))
  (:when (pearl-session-append
          session (pearl-contract-entry 'message '(:content "first, again"))))
  (:then (= 1 (length (pearl-session-branch session))))
  (:then (= 3 (length (pearl-session-entries session))))
  (:then (null (pearl-session-parent session (pearl-session-head session)))))

;; A fork leaves the files where they are, so the console asks what the
;; abandoned turns wrote *before* moving the head.  The move itself applies
;; nothing, which is why asking afterwards answers nothing (ADR 29).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (root-entry nil) (before nil)
           (patch '(:tool "edit" :file "src/a.el" :old-text "a" :new-text "b")))
          (setq root-entry (pearl-session-append
                            session (pearl-contract-entry 'message '(:content "one"))))
          (pearl-session-append
           session (pearl-contract-entry 'tool '(:name "edit") nil patch))
          (setq before (pearl-session-rollback-plan session root-entry)))
  (:when (pearl-session-set-head session root-entry))
  (:then (equal '("src/a.el") (mapcar (lambda (p) (plist-get p :file)) before)))
  (:then (null (pearl-session-rollback-plan session root-entry))))

(provide 'session-tests)

;;; session-tests.el ends here
