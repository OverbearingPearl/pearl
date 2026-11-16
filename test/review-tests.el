;;; review-tests.el --- stories about a failing build  -*- lexical-binding: t; -*-

;;; Commentary:

;; A build that fails inside the console has to be walkable, because reading a
;; stack trace and then hunting the file by hand is the tax this removes.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((text "pearl.el:12:4: error: Unbound variable `foo'") (findings nil)))
  (:when (setq findings (pearl-review-parse text)))
  (:then (equal 1 (length findings)))
  (:then (equal 12 (plist-get (car findings) :line)))
  (:then (eq 'error (plist-get (car findings) :kind))))
(ert-gwt-deftest
  (:given ((text "just some output, no file named") (findings nil)))
  (:when (setq findings (pearl-review-parse text)))
  (:then (null findings)))
(ert-gwt-deftest
  (:given ((result (list :content "pearl.el:12:4: error: boom")) (folded nil)))
  (:when (setq folded (pearl-review-fold-p result)))
  (:then (eq t folded)))
(provide 'review-tests)

;;; review-tests.el ends here
