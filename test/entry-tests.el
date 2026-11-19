;;; entry-tests.el --- stories about the entry point  -*- lexical-binding: t; -*-

;;; Commentary:

;; One command opens the console, and a missing external program is named
;; before the session starts rather than on the first tool call.

;;; Code:

(require 'support-tests)
(require 'pearl)
(require 'pearl-console)
(require 'cl-lib)
(require 'ert-gwt)

;; M-x pearl on a directory opens the console anchored there, so every path
;; the agent resolves is a path the user chose.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (buffer nil)))
  (:when (setq buffer (pearl root)))
  (:then (buffer-live-p buffer))
  (:then (with-current-buffer buffer (derived-mode-p 'pearl-console-mode)))
  (:then (equal (file-name-as-directory root)
                (with-current-buffer buffer default-directory))))

;; M-x pearl leaves the console in front of the user: a console created
;; behind the buffer being edited is a command that did nothing, and that is
;; what this one did until the story said otherwise.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (buffer nil) (front nil)))
  (:when (save-window-excursion
           (setq buffer (pearl root))
           (setq front (window-buffer (selected-window)))))
  (:then (eq front buffer)))

;; A missing program is reported before anything is opened, and every missing
;; one at once: a dependency discovered one request at a time is a dependency
;; the user pays for twice.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (message nil)
           (real (symbol-function 'executable-find))))
  (:when (setq message (condition-case err
                           (cl-letf (((symbol-function 'executable-find)
                                      (lambda (program &rest _)
                                        (unless (equal program "git")
                                          (funcall real program)))))
                             (pearl root)
                             nil)
                         (user-error (error-message-string err)))))
  (:then (string-match-p "git" message))
  (:then (null (seq-filter (lambda (buffer)
                              (with-current-buffer buffer
                                (derived-mode-p 'pearl-console-mode)))
                            (buffer-list)))))

(provide 'entry-tests)

;;; entry-tests.el ends here
