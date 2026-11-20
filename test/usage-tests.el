;;; usage-tests.el --- stories about what a request cost  -*- lexical-binding: t; -*-

;;; Commentary:

;; A count a restart loses is a count nobody can trust, so the record is
;; where a request's price is kept: one entry, written with the turn that
;; paid it, and never sent back to the model.

;;; Code:

(require 'support-tests)
(require 'pearl-app)
(require 'pearl-provider)
(require 'pearl-session)
(require 'pearl-session-jsonl)
(require 'cl-lib)
(require 'ert-gwt)

(defun usage-tests--wait-for (console text)
  "Wait for TEXT to appear in CONSOLE; return non-nil on success."
  (support-tests-wait
   (lambda ()
     (with-current-buffer console
       (string-match-p (regexp-quote text) (buffer-string))))
   10))

(defun usage-tests--session (console)
  "Return the session CONSOLE writes."
  (buffer-local-value 'pearl-console--session console))

(defun usage-tests--entries (session)
  "Return SESSION's branch, filtered to the entries that carry a count."
  (cl-remove-if-not (lambda (entry) (eq (plist-get entry :kind) 'usage))
                    (pearl-session-branch session)))

(defun usage-tests--files (root)
  "Return the session files under ROOT."
  (directory-files-recursively (expand-file-name "sessions" root) "\\.jsonl\\z"))

(defun usage-tests--kinds (file)
  "Return the entry kinds FILE hold, in the order they were written."
  (let ((text (with-temp-buffer
                (insert-file-contents file)
                (buffer-string))))
    (mapcar (lambda (line)
              (plist-get (json-parse-string line :object-type 'plist) :kind))
            (split-string text "\n" t))))

;; A stub that reports its count leaves one entry behind, and the entry says
;; what the provider said and how long the answer took.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider
            (pearl-provider-fake '((delay 0.05) (text "done") (usage 120 7) (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "usage")))
           (session nil)
           (entries nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (usage-tests--wait-for console "done")
                (setq session (usage-tests--session console))
                (setq entries (usage-tests--entries session))))
  (:then (= 1 (length entries)))
  (:then (let ((data (plist-get (car entries) :data)))
           (and (= 120 (plist-get data :prompt))
                (= 7 (plist-get data :completion)))))
  (:then (plist-get (plist-get (car entries) :data) :reported))
  (:then (let ((data (plist-get (car entries) :data)))
           (and (numberp (plist-get data :total))
                (<= 0.05 (plist-get data :total))
                (<= 0 (plist-get data :first-delta)))))
  (:then (member 'usage (usage-tests--kinds (car (usage-tests--files root))))))

;; A stub that reports nothing still leaves a price, and the entry says the
;; price is the estimator's rather than the provider's.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "done") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "silent")))
           (entries nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (usage-tests--wait-for console "done")
                (setq entries (usage-tests--entries (usage-tests--session console)))))
  (:then (= 1 (length entries)))
  (:then (not (plist-get (plist-get (car entries) :data) :reported)))
  (:then (let ((data (plist-get (car entries) :data)))
           (and (< 0 (plist-get data :prompt))
                (< 0 (plist-get data :completion))))))

;; The count is a fact about the request, not part of it: the next request
;; carries the same messages it would have carried without it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (usage 40 3) (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "again")))
           (requests nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (usage-tests--wait-for console "one")
                (pearl-console-send console "again")
                (support-tests-wait (lambda () (= 2 (length (pearl-provider-fake-requests provider)))) 10)
                (setq requests (pearl-provider-fake-requests provider))))
  (:then (= 2 (length requests)))
  (:then (= 4 (length (plist-get (nth 1 requests) :messages))))
  (:then (= 1 (cl-count 'system (mapcar (lambda (message)
                                          (plist-get message :role))
                                        (plist-get (nth 1 requests) :messages))))))

(provide 'usage-tests)

;;; usage-tests.el ends here
