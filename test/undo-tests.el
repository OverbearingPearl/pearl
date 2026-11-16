;;; undo-tests.el --- stories about taking a change back  -*- lexical-binding: t; -*-

;;; Commentary:

;; Two layers: the change group the user reverts with C-/, and the
;; snapshot that survives the buffer and the editor.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((buffer (generate-new-buffer "pearl-undo")) (handle nil)))
  (:when (progn (with-current-buffer buffer (insert "before\n") (undo-boundary) (setq handle (pearl-undo-group-prepare (current-buffer))) (insert "after\n") (pearl-undo-group-cancel handle))))
  (:then (equal "before\n" (with-current-buffer buffer (buffer-string)))))

(ert-gwt-deftest
  (:given ((buffer (generate-new-buffer "pearl-undo-accept")) (handle nil)))
  (:when (progn (with-current-buffer buffer (insert "before\n") (undo-boundary) (setq handle (pearl-undo-group-prepare (current-buffer))) (insert "after\n") (pearl-undo-group-accept handle))))
  (:then (equal "before\nafter\n" (with-current-buffer buffer (buffer-string))))
  (:then (with-current-buffer buffer (undo 1)))
  (:then (equal "before\n" (with-current-buffer buffer (buffer-string)))))

(ert-gwt-deftest
  (:given ((root (file-name-directory (ert-gwt--temp-file "pearl-snapshot"))) (file (expand-file-name "tracked.txt" root)) (snapshot nil)))
  (:when (progn (with-temp-file file (insert "original\n")) (setq snapshot (pearl-undo-snapshot root)) (with-temp-file file (insert "changed\n")) (pearl-undo-restore snapshot (list file))))
  (:then (equal "original\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

(ert-gwt-deftest
  (:given ((root (file-name-directory (ert-gwt--temp-file "pearl-conflict"))) (file (expand-file-name "tracked.txt" root)) (snapshot nil)))
  (:when (progn (with-temp-file file (insert "original\n")) (setq snapshot (pearl-undo-snapshot root)) (with-temp-file file (insert "someone else\n")) (pearl-undo-restore snapshot (list file))))
  (:then (should-error (pearl-undo-conflict-p snapshot file))))
;; The story a user tells after a restart: the turn is on the record, the
;; files moved, and the restore is the one that puts them back.
(ert-gwt-deftest
  (:given ((root (file-name-directory (ert-gwt--temp-file "pearl-turn")))
           (file (expand-file-name "written.txt" root))
           (snapshot nil)
           (restored nil)))
  (:when (progn (with-temp-file file (insert "before\n"))
                (setq snapshot (pearl-undo-snapshot root))
                (with-temp-file file (insert "after\n"))
                (setq restored (pearl-undo-restore snapshot (list file)))))
  (:then (equal (list file) restored))
  (:then (equal "before\n"
                (with-temp-buffer (insert-file-contents file)
                                  (buffer-string)))))

(provide 'undo-tests)

;;; undo-tests.el ends here
