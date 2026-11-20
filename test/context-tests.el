;;; context-tests.el --- stories about the context page  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console shows what happened; a request is built from something else,
;; and the user who asks "why did it say that" needs the second one: a page
;; that is a projection of the record and nothing the user can edit.

;;; Code:

(require 'support-tests)
(require 'pearl-app)
(require 'pearl-compaction)
(require 'pearl-console)
(require 'pearl-contract)
(require 'pearl-provider)
(require 'pearl-session)
(require 'pearl-session-jsonl)
(require 'cl-lib)
(require 'ert-gwt)

(defun context-tests--answer (console)
  "Wait until CONSOLE carries the fake provider's reply."
  (support-tests-wait
   (lambda ()
     (with-current-buffer console
       (string-match-p "one" (buffer-string))))
   10))

(defun context-tests--press (console)
  "Press the context key in CONSOLE, as the user would, and return its page."
  (with-current-buffer console
    (call-interactively (key-binding (kbd "C-c C-p")))))

;; What the model is told is a different question from what the console drew,
;; so the page answers it: the prompt first, then the conversation in the
;; order the provider receives it, every section priced.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "story")))
           (page nil)
           (text nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (context-tests--answer console)
                (setq page (context-tests--press console))
                (setq text (with-current-buffer page (buffer-string)))))
  (:then (buffer-live-p page))
  (:then (eq (with-current-buffer console (key-binding (kbd "C-c C-p")))
             'pearl-console-context))
  (:then (with-current-buffer page (derived-mode-p 'pearl-context-mode)))
  (:then (with-current-buffer page buffer-read-only))
  (:then (string-match-p "system prompt" text))
  (:then (< (string-match "system prompt" text) (string-match "hello" text)))
  (:then (string-match-p "[0-9]+ tokens" text)))

;; A compaction is the one place the record says less than the conversation
;; did, so the page has to say so where it happens: the summary stands where
;; the turns it covers stood, and those turns are no longer on the page.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "compact")))
           (page nil)
           (text nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "alpha")
                (context-tests--answer console)
                (pearl-compaction-append
                 (buffer-local-value 'pearl-console--session console)
                 "earlier work, in short"
                 :keep 0)
                (setq page (context-tests--press console))
                (setq text (with-current-buffer page (buffer-string)))))
  (:then (string-match-p "compaction summary" text))
  (:then (string-match-p "earlier work, in short" text))
  (:then (not (string-match-p "alpha" text))))

;; A page behind another buffer is a question nobody can read the answer to,
;; which is the mistake the console itself made once.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "front")))
           (page nil)
           (front nil)))
  (:when (save-window-excursion
           (pearl-app-wire console)
           (setq page (context-tests--press console))
           (setq front (window-buffer (selected-window)))))
  (:then (buffer-live-p page))
  (:then (eq front page)))

(provide 'context-tests)

;;; context-tests.el ends here
