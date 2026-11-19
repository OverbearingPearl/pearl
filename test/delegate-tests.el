;;; delegate-tests.el --- what a delegate is asked and what it may cost  -*- lexical-binding: t; -*-

;;; Commentary:

;; A delegate is a session of its own (ADR 31).  These stories hold the two
;; things the parent is promised - a pointer and a file, and nothing else -
;; and the two things a delegate may not do: ask, and write.

;;; Code:

(require 'support-tests)
(require 'pearl-delegate)
(require 'pearl-loop)
(require 'pearl-provider)
(require 'pearl-session)
(require 'ert-gwt)

;; The pointer is the arrangement: a model that reads a report inline pays for
;; it on every later turn, and a model that gets a paragraph asks for the file
;; when it wants the rest (ADR 31).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "a.el is unused") (stop end))))
           (result nil)))
  (:when (setq result (pearl-delegate-run
                       '((role . "researcher") (task . "read a.el"))
                       (list :root root :provider provider))))
  (:then (stringp (plist-get result :content)))
  (:then (<= (length (plist-get result :content)) 700))
  (:then (string-match-p "Full result" (plist-get result :content)))
  (:then (file-exists-p (plist-get result :file)))
  (:then (string-match-p "delegations" (plist-get result :file)))
  (:cleanup (delete-directory root t)))

;; The child's reading is on disk and in its own record; the parent keeps a
;; paragraph, which is what stops a delegate costing the parent a projection
;; it did not ask for.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "a long reading") (stop end))))
           (result nil)
           (report nil)
           (child nil)))
  (:when (progn
           (setq result (pearl-delegate-run
                         '((role . "researcher") (task . "read a.el"))
                         (list :root root :provider provider)))
           (setq report (with-temp-buffer
                          (insert-file-contents (plist-get result :file))
                          (buffer-string)))
           (setq child (expand-file-name
                        (format "sessions/%s.jsonl" (plist-get result :delegate))
                        root))))
  (:then (string-match-p "a long reading" report))
  (:then (stringp (plist-get result :delegate)))
  (:then (file-exists-p child))
  (:cleanup (delete-directory root t)))

;; A background loop does not take the minibuffer from a user who is typing in
;; it, so nothing on this path may ask (ADR 31).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "done") (stop end))))
           (asked nil)
           (failed nil)))
  (:when (condition-case err
             (cl-letf (((symbol-function 'y-or-n-p)
                        (lambda (&rest _) (setq asked 'y-or-n-p)))
                       ((symbol-function 'read-string)
                        (lambda (&rest _) (setq asked 'read-string)))
                       ((symbol-function 'read-passwd)
                        (lambda (&rest _) (setq asked 'read-passwd))))
               (pearl-delegate-run '((role . "researcher") (task . "read a.el"))
                                   (list :root root :provider provider)))
           (error (setq failed err))))
  (:then (null failed))
  (:then (null asked))
  (:cleanup (delete-directory root t)))

;; A role that may write waits for a plan that can expand another session's
;; plan by name, which is the reason the roles are ordered this way.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "wrote it") (stop end))))
           (result nil)))
  (:when (setq result (pearl-delegate-run
                       '((role . "worker") (task . "fix a.el"))
                       (list :root root :provider provider))))
  (:then (plist-get result :error))
  (:then (string-match-p "worker" (format "%S" (plist-get result :error))))
  (:cleanup (delete-directory root t)))

;; Two delegates are two sessions, because one file has one writer (ADR 22).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (stop end))))
           (second-provider (pearl-provider-fake '((text "two") (stop end))))
           (first nil)
           (second nil)))
  (:when (progn
           (setq first (pearl-delegate-run '((role . "researcher") (task . "a"))
                                           (list :root root :provider provider)))
           (setq second (pearl-delegate-run
                         '((role . "researcher") (task . "b"))
                         (list :root root :provider second-provider)))))
  (:then (not (equal (plist-get first :delegate) (plist-get second :delegate))))
  (:then (not (equal (plist-get first :file) (plist-get second :file))))
  (:cleanup (delete-directory root t)))

;; The parent keeps a paragraph, so the cut has to hold even when the child
;; answered in one enormous one.
(ert-gwt-deftest
  (:given ((text (concat (make-string 900 ?x) "\n\nsecond paragraph"))
           (pointer nil)))
  (:when (setq pointer (pearl-delegate-pointer text)))
  (:then (<= (length pointer) 500))
  (:then (not (string-match-p "second paragraph" pointer))))

;; What the model sees: a role and a task, and no budget to set, because a
;; model choosing how many agents to start is a budget nobody set.
(ert-gwt-deftest
  (:given ((declaration nil)
           (registry nil)))
  (:when (progn
           (setq declaration (pearl-delegate-declaration))
           (setq registry (pearl-delegate-install nil))))
  (:then (equal '("delegate")
                (mapcar (lambda (tool) (plist-get tool :name)) registry)))
  (:then (equal '(role task) (mapcar #'car (plist-get declaration :arguments))))
  (:then (null (assq 'model (plist-get declaration :arguments)))))

;; A start hands back the child's session, its loop and the file it writes, so
;; a caller waits on the loop instead of on a clock.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "started") (stop end))))
           (handle nil)))
  (:when (progn
           (setq handle (pearl-delegate-start
                         '((role . "researcher") (task . "read a.el"))
                         (list :root root :provider provider)))
           (support-tests-wait
            (lambda () (not (pearl-loop-busy-p (plist-get handle :loop)))) 10)))
  (:then (pearl-session-p (plist-get handle :session)))
  (:then (string-match-p "delegations" (plist-get handle :file)))
  (:cleanup (delete-directory root t)))

;; A role is the permission, so the two roles say different things about
;; writing rather than different things about reading.
(ert-gwt-deftest
  (:given ((researcher (pearl-delegate-role "researcher"))
           (worker (pearl-delegate-role "worker"))))
  (:when (list researcher worker))
  (:then (null (plist-get researcher :writes)))
  (:then (plist-get worker :writes))
  (:then (null (plist-get researcher :asks))))

(provide 'delegate-tests)

;;; delegate-tests.el ends here
