;;; session-tests.el --- stories about the record  -*- lexical-binding: t; -*-

;;; Commentary:

;; What the model reads next is decided here: the active branch and the
;; projection.  What takes a change back is the undo module's.

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
  (:then (seq-find (lambda (message) (equal "kept" (plist-get message :content))) projected))
  (:then (null (seq-find (lambda (message) (equal "other" (plist-get message :content))) projected))))

;; Only an entry that changed something outside the session carries a
;; patch; a conversational entry carries none.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (tool-entry nil) (message-entry nil)))
  (:when (progn (setq tool-entry (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil patch))) (setq message-entry (pearl-session-append session (pearl-contract-entry 'message '(:content "hi"))))))
  (:then (equal patch (pearl-session-patch tool-entry)))
  (:then (null (pearl-session-patch message-entry))))

;; The projection folds tool output a later turn has already read: a tool
;; entry with an assistant response after it projects as a short marker,
;; while the entry itself keeps the whole text.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (bulky (make-string 500 ?x))
           (tool-entry nil) (projected nil) (folded nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'message '(:content "go"))) (setq tool-entry (pearl-session-append session (pearl-contract-entry 'tool (list :name "echo" :id "c1" :content bulky)))) (pearl-session-append session (pearl-contract-entry 'message '(:content "read it"))) (pearl-session-append session (pearl-contract-entry 'message '(:content "again"))) (setq projected (pearl-session-projection session nil)) (setq folded (seq-find (lambda (message) (equal "c1" (plist-get message :id))) projected))))
  (:then (equal "c1" (plist-get folded :id)))
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
  (:then (eq 'system (plist-get (car first) :role)))
  (:then (equal 4 (length (cdr first))))
  (:then (equal "read it" (pearl-contract-text (car (last first)))))
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
  (:then (equal '("one" "two" "three")
                (mapcar (lambda (entry) (plist-get (plist-get entry :data) :content))
                        (pearl-session-branch restored))))
  (:then (equal "three"
                (plist-get (plist-get (pearl-session-head restored) :data) :content)))
  (:then (equal before (pearl-session-projection restored nil))))

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
  (:then (equal '("one" "two" "three")
                (mapcar (lambda (entry) (plist-get (plist-get entry :data) :content))
                        (pearl-session-branch reopened))))
  (:then (equal "three"
                (plist-get (plist-get (pearl-session-head reopened) :data) :content))))

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

;; The sources a product read are held by the session and read back with their
;; paths, because the core never loads a file (ADR 20).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (sources '((:name "project" :path "/tmp/AGENTS.md" :text "be brief")))
           (read nil)))
  (:when (progn (pearl-session-set-prompt-sources session sources)
                (setq read (pearl-session-prompt-sources session))))
  (:then (equal sources read))
  (:then (equal "/tmp/AGENTS.md" (plist-get (car read) :path))))

(provide 'session-tests)

;;; session-tests.el ends here
