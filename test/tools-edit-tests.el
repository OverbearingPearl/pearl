;;; tools-edit-tests.el --- stories about writing through a buffer  -*- lexical-binding: t; -*-

;;; Commentary:

;; One edit call is one undo step: that is the promise, and the change
;; group is where it is kept.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-edit.el")) (buffer nil) (edits (list (list :old-text "old" :new-text "new"))) (undo-before nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (setq undo-before (length buffer-undo-list))) (pearl-tools-edit-run (list (cons (quote path) file) (cons (quote edits) edits)) nil) (with-current-buffer buffer (setq undo-before buffer-undo-list))))
  (:then (equal "new\n" (with-current-buffer buffer (buffer-string))))
  (:then (with-current-buffer buffer (undo 1)))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string)))))

(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-stale.el")) (edits (list (list :old-text "missing" :new-text "new"))) (result nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq result (pearl-tools-edit-run (list (cons (quote path) file) (cons (quote edits) edits)) nil))))
  (:then (plist-get result :error))
  (:then (equal "old\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))
(provide 'tools-edit-tests)

;;; tools-edit-tests.el ends here
