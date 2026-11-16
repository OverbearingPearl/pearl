;;; tools-read-tests.el --- stories about reading a file  -*- lexical-binding: t; -*-

;;; Commentary:

;; A read has to say which window it returned, or the model asks again
;; for a range it has already seen.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-read.el")) (root (file-name-directory file)) (result nil)))
  (:when (progn (with-temp-file file (insert "one\ntwo\nthree\n")) (setq result (pearl-tools-read-run (list (cons (quote path) file) (cons (quote offset) 2) (cons (quote limit) 1)) (list :root root)))))
  (:then (equal "two" (string-trim (plist-get result :content))))
  (:then (equal 3 (plist-get result :total-lines))))

(ert-gwt-deftest
  (:given ((result nil)))
  (:when (setq result (pearl-tools-read-run (list (cons (quote path) "/no/such/file")) nil)))
  (:then (plist-get result :error)))
(provide 'tools-read-tests)

;;; tools-read-tests.el ends here
