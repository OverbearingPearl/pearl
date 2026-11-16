;;; session-tests.el --- stories about the record  -*- lexical-binding: t; -*-

;;; Commentary:

;; What the model reads next is decided here: the active branch, the
;; projection, and a rollback plan scoped to a path.

;;; Code:

(require 'pearl-support-tests)
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
  (:when (progn (pearl-session-append session kept) (pearl-session-append session (pearl-contract-entry (quote message) (quote (:content "other")))) (pearl-session-set-head session kept) (setq projected (pearl-session-projection session nil))))
  (:then (null (seq-find (lambda (message) (equal "other" (plist-get message :content))) projected))))

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (patch (quote (:tool "edit" :file "a.el" :old-text "a" :new-text "b"))) (plan nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil patch)) (pearl-session-append session (pearl-contract-entry 'message '(:content "hi"))) (setq plan (pearl-session-rollback-plan session))))
  (:then (equal (quote ("a.el")) (delete-dups (mapcar (lambda (p) (plist-get p :file)) plan)))))

(ert-gwt-deftest
  (:given ((empty-patch (quote (:tool "delete" :file "a.el" :old-text "" :new-text "")))))
  (:when (pearl-session-conflict-p empty-patch ""))
  (:then (eq t (pearl-session-conflict-p empty-patch ""))))
(provide 'session-tests)

;;; session-tests.el ends here
