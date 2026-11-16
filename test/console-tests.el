;;; console-tests.el --- stories about the surface  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console is the only place the user sees a run, so it has to be
;; cheap per delta and honest about what it shows.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((console (pearl-console-open (ert-gwt--temp-file "pearl-root") "story"))))
  (:when (progn (pearl-console-send console "hello") (pearl-console-redraw console)))
  (:then (equal "hello" (with-current-buffer console (buffer-substring-no-properties (point-min) (point-max))))))

(ert-gwt-deftest
  (:given ((console (pearl-console-open (ert-gwt--temp-file "pearl-root") "abort")) (queued nil)))
  (:when (progn (pearl-console-send console "first") (setq queued (pearl-console-abort console))))
  (:then (listp queued)))

(ert-gwt-deftest
  (:given ((console (pearl-console-open (ert-gwt--temp-file "pearl-root") "status"))))
  (:when (pearl-console-status console "reading a.el"))
  (:then (equal "reading a.el" (pearl-console-status console))))
(provide 'console-tests)

;;; console-tests.el ends here
