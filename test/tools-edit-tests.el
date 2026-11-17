;;; tools-edit-tests.el --- stories about writing through a buffer  -*- lexical-binding: t; -*-

;;; Commentary:

;; One edit call is one undo step: that is the promise, and the change
;; group is where it is kept.

;;; Code:

(require 'support-tests)
(require 'pearl-tools-edit)
(require 'ert-gwt)

;; The everyday path: an applied edit is reverted by exactly one C-/, and
;; a clean buffer is saved, so what the model changed is what a build sees.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-edit.el")) (buffer nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new")))) nil)))
  (:then (equal "new\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "new\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (with-current-buffer buffer (undo 1)))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string)))))

;; A stale anchor is a loud error and a round trip: the buffer text, the
;; undo list and the file are all as they were, because a failed tool is
;; not an undo step (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-stale.el")) (buffer nil) (undo-before nil) (result nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (setq undo-before buffer-undo-list)) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "missing" :new-text "new")))) nil))))
  (:then (plist-get result :error))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal undo-before (with-current-buffer buffer buffer-undo-list)))
  (:then (equal "old\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; Several replacements in one call are still one undo step, under a real
;; major mode: the whole call is one change group (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-many.el")) (buffer nil)
           (edits (list (list :old-text "one" :new-text "1") (list :old-text "two" :new-text "2")))))
  (:when (progn (with-temp-file file (insert "one\ntwo\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits edits)) nil)))
  (:then (equal "1\n2\n" (with-current-buffer buffer (buffer-string))))
  (:then (with-current-buffer buffer (undo 1)))
  (:then (equal "one\ntwo\n" (with-current-buffer buffer (buffer-string)))))

;; An anchor that matches twice is ambiguous, and the whole call is
;; refused naming the anchor: a guess is worse than a round trip (ADR 4).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-ambiguous.el")) (buffer nil) (result nil)))
  (:when (progn (with-temp-file file (insert "x\nx\n")) (setq buffer (find-file-noselect file)) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "x" :new-text "y")))) nil))))
  (:then (plist-get result :error))
  (:then (string-match-p "x" (format "%S" (plist-get result :error))))
  (:then (equal "x\nx\n" (with-current-buffer buffer (buffer-string)))))

;; Overlapping anchors are refused before anything is written: a
;; half-applied edit is not a state the user can reach.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-overlap.el")) (buffer nil)
           (edits (list (list :old-text "ab" :new-text "1") (list :old-text "bc" :new-text "2")))
           (result nil)))
  (:when (progn (with-temp-file file (insert "abc\n")) (setq buffer (find-file-noselect file)) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits edits)) nil))))
  (:then (plist-get result :error))
  (:then (equal "abc\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "abc\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; A buffer the user had already changed is not saved by the tool: the
;; user's unsaved work is theirs, and the result says so.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-dirty.el")) (buffer nil) (result nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (goto-char (point-min)) (insert ";; note\n")) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new")))) nil))))
  (:then (pearl-tools-edit-unsaved-p buffer))
  (:then (plist-get result :unsaved))
  (:then (buffer-modified-p buffer))
  (:then (string-match-p "new" (with-current-buffer buffer (buffer-string))))
  (:then (equal "old\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; Apply is the one caller of the change-group API: it returns how many
;; replacements landed and the patch the entry will carry.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-apply.el")) (buffer nil)
           (edits (list (list :old-text "one" :new-text "1") (list :old-text "two" :new-text "2")))
           (applied nil)))
  (:when (progn (with-temp-file file (insert "one\ntwo\n")) (setq buffer (find-file-noselect file)) (setq applied (with-current-buffer buffer (pearl-tools-edit-apply buffer edits)))))
  (:then (equal 2 (car applied)))
  (:then (equal file (plist-get (cdr applied) :file))))

;; The declaration matches Pi's edit schema (ADR 8): edits[], each an
;; oldText and a newText, so a model trained on one works with the other.
(ert-gwt-deftest
  (:given ((declaration nil)))
  (:when (setq declaration (pearl-tools-edit-declaration)))
  (:then (equal "edit" (plist-get declaration :name)))
  (:then (alist-get 'edits (plist-get declaration :arguments))))

;; C-/ is buffer-scoped, and the spec says so out loud: after the undo the
;; text is the user's again, the saved file still holds the edit until the
;; next save, and the buffer is modified so the difference is visible.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-scope.el")) (buffer nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new")))) nil) (with-current-buffer buffer (undo 1))))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "new\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (buffer-modified-p buffer)))

(provide 'tools-edit-tests)

;;; tools-edit-tests.el ends here
