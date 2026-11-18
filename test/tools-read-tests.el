;;; tools-read-tests.el --- stories about reading a file  -*- lexical-binding: t; -*-

;;; Commentary:

;; A read has to say which window it returned, or the model asks again
;; for a range it has already seen.

;;; Code:

(require 'support-tests)
(require 'pearl-tools-read)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-read.el")) (root (file-name-directory file)) (result nil)))
  (:when (progn (with-temp-file file (insert "one\ntwo\nthree\n")) (setq result (pearl-tools-read-run (list (cons 'path file) (cons 'offset 2) (cons 'limit 1)) (list :root root)))))
  (:then (string-match-p "two" (plist-get result :content)))
  (:then (not (string-match-p "one\\|three" (plist-get result :content))))
  (:then (string-match-p "3" (plist-get result :content))))

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-tools-read-run (list (cons 'path "/no/such/file")) nil)))
  (:then (plist-get result :error)))

;; A relative path resolves against the project root, and nowhere else:
;; the context is the only place a root comes from.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (result nil)))
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
  (:then (equal '(path offset limit)
                (mapcar #'car (plist-get declaration :arguments)))))

;; A binary file is a result the model can act on, not a buffer of
;; undecodable bytes and not a signal that fails the turn.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-binary.bin")) (result nil)))
  (:when (progn (with-temp-file file (insert "text" ?\0 "more")) (setq result (pearl-tools-read-run (list (cons 'path file)) nil))))
  (:then (plist-get result :error)))

;; Reading is not writing: the three layers in ADR 5 are about writes, so the
;; root anchors a relative path and does not fence an absolute one.  The
;; asymmetry with edit is deliberate, and a story holds it in place.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (ert-gwt--temp-file "pearl-outside-read.txt"))
           (result nil))
          (with-temp-file file (insert "outside the root\n")))
  (:when (setq result (pearl-tools-read-run (list (cons 'path file))
                                            (list :root root))))
  (:then (null (plist-get result :error)))
  (:then (string-match-p "outside the root" (plist-get result :content))))

(provide 'tools-read-tests)

;;; tools-read-tests.el ends here
