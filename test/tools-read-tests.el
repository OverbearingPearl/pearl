;;; tools-read-tests.el --- stories about reading a file  -*- lexical-binding: t; -*-

;;; Commentary:

;; A read has to say which window it returned, or the model asks again
;; for a range it has already seen.

;;; Code:

(require 'pearl-support-tests)
(require 'pearl-tools-read)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-read.el")) (root (file-name-directory file)) (result nil)))
  (:when (progn (with-temp-file file (insert "one\ntwo\nthree\n")) (setq result (pearl-tools-read-run (list (cons 'path file) (cons 'offset 2) (cons 'limit 1)) (list :root root)))))
  (:then (equal "two" (string-trim (plist-get result :content))))
  (:then (equal 3 (plist-get result :total-lines))))

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-tools-read-run (list (cons 'path "/no/such/file")) nil)))
  (:then (plist-get result :error)))

;; A relative path resolves against the project root, and nowhere else:
;; the context is the only place a root comes from.
(ert-gwt-deftest
  (:given ((root (pearl-support-temp-root)) (result nil)))
  (:when (progn (with-temp-file (expand-file-name "here.txt" root) (insert "found\n")) (setq result (pearl-tools-read-run '((path . "here.txt")) (list :root root)))))
  (:then (null (plist-get result :error)))
  (:then (string-match-p "found" (plist-get result :content))))

;; An offset past the end is a result the model can act on, not a signal
;; that fails the turn.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-short.el")) (result nil)))
  (:when (progn (with-temp-file file (insert "one\n")) (setq result (pearl-tools-read-run (list (cons 'path file) (cons 'offset 99)) nil))))
  (:then (plist-get result :error)))

;; The declaration names the same arguments Pi's read takes, so a model
;; trained on one works with the other (ADR 8).
(ert-gwt-deftest
  (:given ((declaration nil)))
  (:when (setq declaration (pearl-tools-read-declaration)))
  (:then (equal "read" (plist-get declaration :name)))
  (:then (alist-get 'path (plist-get declaration :arguments)))
  (:then (alist-get 'offset (plist-get declaration :arguments)))
  (:then (alist-get 'limit (plist-get declaration :arguments))))

;; A binary file is a result the model can act on, not a buffer of
;; undecodable bytes and not a signal that fails the turn.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-binary.bin")) (result nil)))
  (:when (progn (with-temp-file file (insert "text" ?\0 "more")) (setq result (pearl-tools-read-run (list (cons 'path file)) nil))))
  (:then (plist-get result :error)))

(provide 'tools-read-tests)

;;; tools-read-tests.el ends here
