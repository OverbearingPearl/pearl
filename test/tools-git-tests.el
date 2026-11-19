;;; tools-git-tests.el --- stories about committing what a turn wrote   -*- lexical-binding: t; -*-

;;; Commentary:

;; A commit that stages what it was not told to stage is the one write no
;; rollback reaches: the turn's snapshot is gone and another session's
;; half-written file is in a commit under this turn's message.

;;; Code:

(require 'support-tests)
(require 'pearl-tools-git)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)))
  (:when (progn
           (with-temp-file (expand-file-name "a.txt" root) (insert "one\n"))
           (with-temp-file (expand-file-name "b.txt" root) (insert "their one\n"))
           (support-tests-git root "add" "a.txt" "b.txt")
           (support-tests-git root "commit" "-q" "-m" "start")
           (with-temp-file (expand-file-name "a.txt" root) (insert "two\n"))
           (with-temp-file (expand-file-name "b.txt" root) (insert "theirs\n"))
           (setq result (pearl-tools-git-commit
                         (list (cons 'message "mine")
                               (cons 'paths (list "a.txt")))
                         (list :root root)))))
  (:then (null (plist-get result :error)))
  (:then (equal "a.txt" (support-tests-git root "show" "--name-only" "--format=")))
  (:then (string-match-p "b.txt" (support-tests-git root "status" "--porcelain")))
  (:then (string-match-p "\\`[0-9a-f]\\{40\\}\\'" (plist-get result :commit)))
  (:then (plist-get result :epoch))
  (:cleanup (delete-directory root t)))

(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)))
  (:when (progn
           (with-temp-file (expand-file-name "a.txt" root) (insert "one\n"))
           (with-temp-file (expand-file-name "b.txt" root) (insert "their one\n"))
           (support-tests-git root "add" "a.txt" "b.txt")
           (support-tests-git root "commit" "-q" "-m" "start")
           (with-temp-file (expand-file-name "a.txt" root) (insert "two\n"))
           (with-temp-file (expand-file-name "b.txt" root) (insert "theirs\n"))
           (support-tests-git root "add" "b.txt")
           (setq result (pearl-tools-git-commit
                         (list (cons 'message "mine")
                               (cons 'paths (list "a.txt")))
                         (list :root root)))))
  (:then (plist-get result :error))
  (:then (string-match-p "b.txt" (plist-get result :content)))
  (:then (equal "1" (support-tests-git root "rev-list" "--count" "HEAD")))
  (:then (string-match-p "b.txt" (support-tests-git root "diff" "--cached" "--name-only")))
  (:cleanup (delete-directory root t)))

(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)))
  (:when (progn
           (with-temp-file (expand-file-name "a.txt" root) (insert "one\n"))
           (support-tests-git root "add" "a.txt")
           (support-tests-git root "commit" "-q" "-m" "start")
           (with-temp-file (expand-file-name "a.txt" root) (insert "two\n"))
           (setq result (pearl-tools-git-commit
                         (list (cons 'message "mine")) (list :root root)))))
  (:then (plist-get result :error))
  (:then (string-match-p "paths" (plist-get result :content)))
  (:then (equal "1" (support-tests-git root "rev-list" "--count" "HEAD")))
  (:cleanup (delete-directory root t)))

(provide 'tools-git-tests)

;;; tools-git-tests.el ends here
