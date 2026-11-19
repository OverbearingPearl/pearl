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
  (:given ((command "curl -s https://example.com -o /tmp/x") (scope nil) (approved nil)
           ((symbol-function 'yes-or-no-p)
            (lambda (&rest _) (error "Asked to confirm an unattended command")))))
  (:when (setq approved (pearl-tools-shell-confirm command scope "download a page")))
  (:then (not approved)))

;; A command whose write scope cannot be established is not proved, and
;; unattended that means refused without asking: a push is the case no parser
;; can account for, so nobody is asked and nothing is run.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)
           ((symbol-function 'yes-or-no-p)
            (lambda (&rest _) (error "Asked about a push")))))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command "git push origin main") (cons 'reason "publish the branch")) (list :root root))))
  (:then (plist-get result :error)))

;; When the user is there, the question shows the command and the model's
;; reason: the reason is the only thing that makes the prompt answerable.
(ert-gwt-deftest
  (:given ((pearl-tools-shell-attended t) (prompt nil) (approved nil)
           ((symbol-function 'yes-or-no-p) (lambda (question) (setq prompt question) t))))
  (:when (setq approved (pearl-tools-shell-confirm "git push origin main" nil "publish the branch")))
  (:then (eq t approved))
  (:then (string-match-p "git push origin main" prompt))
  (:then (string-match-p "publish the branch" prompt))
  (:then (string-match-p "undo\\|take it back" prompt)))

;; A proved write carries the paths it was proved to write, and says the turn
;; can take it back; that pair is what lets a turn attribute its own writes
;; instead of diffing the tree (ADR 23).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil)))
  (:when (setq result
               (pearl-tools-shell-run
                (list (cons 'command "echo hi > out.txt")
                      (cons 'reason "write the output"))
                (list :root root))))
  (:then (equal 0 (plist-get result :status)))
  (:then (equal (list (expand-file-name "out.txt" root))
                (mapcar (lambda (path) (expand-file-name path root))
                        (plist-get result :scope))))
  (:then (eq t (plist-get result :revertible))))

;; A redirect into the project is a provable write scope when the snapshot
;; covers the target: the paths it names are all under a git root (ADR 16).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (scope nil)))
  (:when (setq scope (pearl-tools-shell-scope "echo hi > out.txt" root)))
  (:then (equal (list (expand-file-name "out.txt" root))
                (mapcar (lambda (path) (expand-file-name path root)) scope))))

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
  (:given ((root (support-tests-temp-root)) (target (ert-gwt--temp-file "pearl-outside")) (result nil))
          (with-temp-file target (insert "keep\n")))
  (:when (setq result (pearl-tools-shell-run (list (cons 'command (format "echo x > %s" target)) (cons 'reason "write outside the project")) (list :root root))))
  (:then (plist-get result :error))
  (:then (equal "keep\n" (with-temp-buffer (insert-file-contents target) (buffer-string)))))

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
  (:then (equal '(command reason snapshot)
                (mapcar #'car (plist-get declaration :arguments)))))

;; ADR 16: a write to an ignored path is not proved even inside the root,
;; because the snapshot cannot take it back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (scope nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n")))
  (:when (setq scope (pearl-tools-shell-scope "echo x > secret.env" root)))
  (:then (null scope)))

;; The question goes to root's repository and not to the one the environment
;; names: a launcher that passes repository config (a git hook, or a wrapper
;; that pushes with `-c core.worktree=DIR') must not turn an ignored path
;; into a proved write.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (other (support-tests-temp-root))
           (scope nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n")))
  (:when (let ((process-environment
                (cons (format "GIT_CONFIG_PARAMETERS=core.worktree=%s" other)
                      process-environment)))
           (setq scope (pearl-tools-shell-scope "echo x > secret.env" root))))
  (:then (null scope)))

;; A git that cannot answer is not a git that says no.  check-ignore exits 0
;; for ignored and 1 for not ignored; every other status is a refusal to parse
;; the question, and reading it as permission is how an ignored file gets
;; overwritten by a command nobody confirmed.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (broken (support-tests-temp-root))
           (scope nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n"))
          (let ((script (expand-file-name "git" broken)))
            (with-temp-file script (insert "#!/bin/sh\nexit 128\n"))
            (set-file-modes script #o755)))
  (:when (let ((exec-path (list broken)))
           (setq scope (pearl-tools-shell-scope "echo x > secret.env" root))))
  (:then (null scope)))

;; Magit exports GIT_LITERAL_PATHSPECS, so every path handed to git is read as
;; a literal pathspec.  `check-ignore' supports no literal magic and exits 128
;; with "pathspec magic not supported by this command", which an answer of
;; "not ignored" is indistinguishable from: the editor session that asked got
;; permission to overwrite a file its own repository ignores.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (scope nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "secret.env\n")))
  (:when (let ((process-environment
                (cons "GIT_LITERAL_PATHSPECS=1" process-environment)))
           (setq scope (pearl-tools-shell-scope "echo x > secret.env" root))))
  (:then (null scope)))

;; A repository that carries another tree in core.worktree -- the residue a
;; redirected `git init' leaves behind -- must still answer about root: the
;; ignore file that matters is the one in root, and git otherwise calls every
;; path here outside the tree it was told to use.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (other (support-tests-git-root))
           (scope nil))
          (progn (support-tests-git root "config" "core.worktree" other)
                 (with-temp-file (expand-file-name ".gitignore" root)
                   (insert "secret.env
"))))
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

;; A command runs while another process writes the tree: the result says the
;; tree moved and names the path, instead of reporting only the exit status.
;; The writer is a process and not a timer because a command holds Emacs while
;; it runs; another session is another process, and that is what this is.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (result nil))
          (make-process
           :name "pearl-story-writer"
           :noquery t
           :buffer nil
           :sentinel #'ignore
           :command (list shell-file-name "-c"
                          (format "sleep 0.3; printf 'two\\n' > %s"
                                  (expand-file-name "b.txt" root)))))
  (:when (setq result (pearl-tools-shell-run
                       (list (cons 'command "sleep 1")
                             (cons 'reason "give the other writer time"))
                       (list :root root))))
  (:then (equal 0 (plist-get result :status)))
  (:then (equal '("b.txt") (plist-get result :drift)))
  (:then (string-match-p "b\\.txt" (plist-get result :content)))
  (:then (string-match-p "changed" (plist-get result :content))))

;; A command that changes nothing says nothing: a sentence on every run would
;; train the reader to ignore the one that matters.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (result nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\\n")))
  (:when (setq result (pearl-tools-shell-run
                       (list (cons 'command "true") (cons 'reason "do nothing"))
                       (list :root root))))
  (:then (equal 0 (plist-get result :status)))
  (:then (null (plist-get result :drift)))
  (:then (equal (plist-get result :epoch)
                (pearl-tree-epoch-from-note (plist-get result :gate))))
  (:then (null (string-match-p "changed" (plist-get result :content)))))

;; The shell tool joins the same list, once, beside whatever is already in it.
(ert-gwt-deftest
  (:given ((registry nil) (twice nil)))
  (:when (progn (setq registry (pearl-tools-shell-install nil))
                (setq twice (pearl-tools-shell-install registry))))
  (:then (equal 1 (length registry)))
  (:then (equal 1 (length twice)))
  (:then (equal 1 (length (pearl-tool-declarations twice)))))

;; A gate is a verdict about a tree, and a tree that is still being written is
;; not one tree.  The command reads the copy it was given, so the answer names
;; the snapshot it is about instead of whatever the file became meanwhile.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (result nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\n"))
          (support-tests-git root "add" "a.txt")
          (support-tests-git root "commit" "-q" "-m" "start")
          (make-process
           :name "pearl-story-writer"
           :noquery t
           :buffer nil
           :sentinel #'ignore
           :command (list shell-file-name "-c"
                          (format "sleep 0.3; printf 'two\\n' > %s"
                                  (expand-file-name "a.txt" root)))))
  (:when (setq result (pearl-tools-shell-run
                       (list (cons 'command "sleep 1; cat a.txt")
                             (cons 'reason "the gate the commit cites")
                             (cons 'snapshot t))
                       (list :root root))))
  (:then (equal "one\n" (plist-get result :stdout)))
  (:then (eq t (plist-get result :snapshot)))
  (:then (string-match-p "snapshot" (plist-get result :content)))
  (:cleanup (delete-directory root t)))

(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (result nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\n"))
          (support-tests-git root "add" "a.txt")
          (support-tests-git root "commit" "-q" "-m" "start"))
  (:when (setq result (pearl-tools-shell-run
                       (list (cons 'command "printf 'two\\n' > a.txt")
                             (cons 'reason "write")
                             (cons 'snapshot t))
                       (list :root root))))
  (:then (string-match-p "snapshot" (plist-get result :error)))
  (:then (equal "one\n" (with-temp-buffer
                           (insert-file-contents (expand-file-name "a.txt" root))
                           (buffer-string))))
  (:cleanup (delete-directory root t)))

(provide 'tools-shell-tests)

;;; tools-shell-tests.el ends here
