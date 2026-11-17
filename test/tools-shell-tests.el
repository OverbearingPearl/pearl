;;; tools-shell-tests.el --- stories about a command  -*- lexical-binding: t; -*-

;;; Commentary:

;; A command's effects cannot be taken back, so this is the one tool that
;; asks, and it asks only where it cannot prove the write scope.

;;; Code:

(require 'support-tests)
(require 'pearl-tools-shell)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((context (list :root default-directory)) (result nil)))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command "echo hi") (cons 'reason "show the greeting")) context)))
  (:then (equal 0 (plist-get result :status)))
  (:then (equal "hi\n" (plist-get result :stdout))))

;; Unattended, a command whose scope cannot be established is refused
;; without asking: nil scope selects the confirming regime, and nobody
;; is there to confirm.
(ert-gwt-deftest
  (:given ((command "curl -s https://example.com -o /tmp/x") (scope nil) (approved nil)))
  (:when (setq approved (pearl-tools-shell-confirm command scope "download a page")))
  (:then (not approved)))

(ert-gwt-deftest
  (:given ((scope nil)))
  (:when (pearl-tools-shell-scope "git push origin main" default-directory))
  (:then (null (pearl-tools-shell-scope "git push origin main" default-directory))))

;; When the user is there, the question shows the command and the model's
;; reason: the reason is the only thing that makes the prompt answerable.
(ert-gwt-deftest
  (:given ((prompt nil) (approved nil)
           ((symbol-function 'yes-or-no-p) (lambda (question) (setq prompt question) t))))
  (:when (setq approved (pearl-tools-shell-confirm "git push origin main" nil "publish the branch")))
  (:then (eq t approved))
  (:then (string-match-p "git push origin main" prompt))
  (:then (string-match-p "publish the branch" prompt)))

;; A redirect into the project is a provable write scope when the snapshot
;; covers the target: the paths it names are all under a git root (ADR 16).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (scope nil)))
  (:when (setq scope (pearl-tools-shell-scope "echo hi > out.txt" root)))
  (:then scope)
  (:then (seq-every-p (lambda (path) (string-prefix-p (file-name-as-directory root) (expand-file-name path root))) scope)))

;; A provably in-root write runs without a prompt: the first two safety
;; layers cover it, so the third does not tax it (ADR 5).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)
           ((symbol-function 'yes-or-no-p) (lambda (&rest _) (error "Asked about an in-root write")))))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command "echo hi > out.txt") (cons 'reason "write inside the project")) (list :root root))))
  (:then (equal 0 (plist-get result :status)))
  (:then (equal "hi\n" (with-temp-buffer (insert-file-contents (expand-file-name "out.txt" root)) (buffer-string)))))

;; A write outside the project is refused when nobody can confirm, and
;; the refusal is a result the model can read, not a signal.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (target (ert-gwt--temp-file "pearl-outside")) (result nil)))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command (format "echo x > %s" target)) (cons 'reason "write outside the project")) (list :root root))))
  (:then (plist-get result :error))
  (:then (equal "" (with-temp-buffer (insert-file-contents target) (buffer-string)))))

;; A nonzero exit is a status the model reads, not a failed turn.
(ert-gwt-deftest
  (:given ((context (list :root default-directory)) (result nil)))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command "exit 7") (cons 'reason "show a failing build")) context)))
  (:then (equal 7 (plist-get result :status))))

;; The declaration matches Pi's shell schema (ADR 8): command and reason,
;; the reason being what makes the confirmation answerable.
(ert-gwt-deftest
  (:given ((declaration nil)))
  (:when (setq declaration (pearl-tools-shell-declaration)))
  (:then (equal "shell" (plist-get declaration :name)))
  (:then (alist-get 'command (plist-get declaration :arguments)))
  (:then (alist-get 'reason (plist-get declaration :arguments))))

;; ADR 16: a write to an ignored path is not proved even inside the root,
;; because the snapshot cannot take it back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (scope nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n")))
  (:when (setq scope (pearl-tools-shell-scope "echo x > secret.env" root)))
  (:then (null scope)))

;; Unattended, the ignored write is refused and the file keeps its contents:
;; the accident the promise exists to stop does not walk through the net.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (target (expand-file-name "secret.env" root))
           (result nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n"))
          (with-temp-file target (insert "keep\n")))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command "echo x > secret.env") (cons 'reason "rewrite the environment")) (list :root root))))
  (:then (plist-get result :error))
  (:then (equal "keep\n" (with-temp-buffer (insert-file-contents target) (buffer-string)))))

;; Outside a git repository there is no snapshot, so no write is proved:
;; the regime asks exactly where revertibility ends (ADR 16).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (scope nil)))
  (:when (setq scope (pearl-tools-shell-scope "echo hi > out.txt" root)))
  (:then (null scope)))

(provide 'tools-shell-tests)

;;; tools-shell-tests.el ends here
