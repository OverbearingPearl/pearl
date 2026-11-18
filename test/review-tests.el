;;; review-tests.el --- stories about a failing build  -*- lexical-binding: t; -*-

;;; Commentary:

;; A build that fails inside the console has to be walkable, because reading a
;; stack trace and then hunting the file by hand is the tax this removes.

;;; Code:

(require 'support-tests)
(require 'pearl-review)
(require 'pearl-app)
(require 'pearl-console)
(require 'pearl-provider)
(require 'xref)
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

;; A warning is a finding too.
(ert-gwt-deftest
  (:given ((text "pearl.el:3:1: warning: unused variable") (findings nil)))
  (:when (setq findings (pearl-review-parse text)))
  (:then (eq 'warning (plist-get (car findings) :kind))))

;; A result with no findings does not fold: its text is the only copy.
(ert-gwt-deftest
  (:given ((result (list :content "plain output")) (folded t)))
  (:when (setq folded (pearl-review-fold-p result)))
  (:then (null folded)))

;; A finding resolves to a marker in the file's buffer, opening it if
;; needed: navigation lands on the line, not near it.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-find.el")) (finding nil) (marker nil)))
  (:when (progn (with-temp-file file (insert (mapconcat #'number-to-string (number-sequence 1 20) "\n"))) (setq finding (car (pearl-review-parse (format "%s:12:4: error: boom" file)))) (setq marker (pearl-review-locate finding))))
  (:then (markerp marker))
  (:then (equal (file-truename file) (file-truename (buffer-file-name (marker-buffer marker)))))
  (:then (equal 12 (line-number-at-pos marker))))

;; A finding whose file is gone degrades to text: nil, not an error at
;; navigation time.
(ert-gwt-deftest
  (:given ((finding nil) (marker nil)))
  (:when (progn
           (setq finding (car (pearl-review-parse "/no/such/file.el:12:4: error: boom")))
           (setq marker (pearl-review-locate finding))))
  (:then (equal 12 (plist-get finding :line)))
  (:then (null marker)))

;; The acceptance of M8, end to end: a failing build reported by a tool is
;; navigable with next-error, and the window the user is in shows the place.
;; The buffer is read through the selected window, because `next-error'
;; runs its function inside `with-current-buffer' and puts the buffer
;; variable back when it returns.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "pearl.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command "echo pearl.el:12:4: error: boom") (cons 'reason "run the build"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "review"))))
          (with-temp-file file (insert (concat (mapconcat #'number-to-string (number-sequence 1 20) "\n") "\n"))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "build it") (support-tests-wait (lambda () (with-current-buffer console (string-match-p "done" (buffer-string)))) 10) (next-error)))
  (:then (equal (file-truename file)
                (file-truename (buffer-file-name (window-buffer (selected-window))))))
  (:then (equal 12 (with-current-buffer (window-buffer (selected-window))
                     (line-number-at-pos (window-point (selected-window)))))))

;; The same findings are xref hits: the symbol the compiler named resolves
;; to the place the compiler named it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "pearl.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command "echo pearl.el:12:4: error: Unbound variable foo") (cons 'reason "run the build"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "xref")))
           (backend nil) (items nil))
          (with-temp-file file (insert (concat (mapconcat #'number-to-string (number-sequence 1 20) "\n") "\n"))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "build it") (support-tests-wait (lambda () (with-current-buffer console (string-match-p "done" (buffer-string)))) 10) (with-current-buffer console (setq backend (xref-find-backend)) (setq items (xref-backend-definitions backend "foo")))))
  (:then (eq 'pearl-review-xref-backend backend))
  (:then (equal 1 (length items)))
  (:then (equal (file-truename file)
                (file-truename
                 (buffer-file-name
                  (marker-buffer
                   (xref-location-marker (xref-item-location (car items))))))))
  (:then (equal 12
                (let ((found (xref-location-marker
                              (xref-item-location (car items)))))
                  (with-current-buffer (marker-buffer found)
                    (line-number-at-pos found))))))

;; Install returns the way out, and taking it is safe.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "unsub")))
           (unsubscribe nil) (installed nil)))
  (:when (progn
           (pearl-app-wire console)
           (setq unsubscribe (pearl-review-install console))
           (with-current-buffer console
             (setq installed (memq 'pearl-review-xref-backend xref-backend-functions)))
           (funcall unsubscribe)))
  (:then (functionp unsubscribe))
  (:then (and installed
              (null (with-current-buffer console
                      (memq 'pearl-review-xref-backend xref-backend-functions))))))

(provide 'review-tests)

;;; review-tests.el ends here
