;;; pearl-tools-shell.el --- the shell tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The tool whose effects cannot be taken back.  A command can push a branch,
;; send a request or write outside the project, and no snapshot undoes that, so
;; this is the one place the user is asked first (ADR 5).
;;
;; The question is asked where it can be answered: a command that provably
;; writes only inside the project root, to paths the turn snapshot covers,
;; runs without a prompt (ADR 16), and a command whose write scope cannot be
;; established is confirmed.  Confirmation is not a sandbox and the status
;; line says which regime is active, so nobody has to guess which one they
;; are in.
;;
;; Planned API
;;
;; `pearl-tools-shell-declaration' -> tool declaration
;;      `command' and `reason', as in Pi (ADR 8).  The reason is shown in the
;;      confirmation prompt, which is the only thing that makes the prompt
;;      answerable.
;;
;; `pearl-tools-shell-run' (ARGUMENTS CONTEXT) -> tool result
;;      Resolve the write scope, ask when it cannot be proved, run through
;;      `pearl-proc-run', and return bounded output with the exit status.
;;      Never signals; a refused confirmation is a result the model can read.
;;      The result also carries `:scope', the paths the command was proved to
;;      write, and `:revertible', which is nil when the scope was not proved:
;;      a turn attributes its writes from `:scope', so a call that cannot name
;;      its paths cannot be taken back, and the console marks that turn when
;;      it lands instead of letting the user find out at undo time (ADR 23).
;;
;;      A scope that is named but not covered by the turn snapshot - outside
;;      the root, or ignored inside it - is captured before the command runs:
;;      the pre-image goes to `pearl-undo-preimage', the result carries it, and
;;      `:revertible' follows `pearl-undo-preimage-revertible-p'.  The
;;      confirmation is asked first, so a refused command has copied nothing,
;;      and the prompt can say what the answer buys (ADR 25).
;;
;; `pearl-tools-shell-scope' (COMMAND ROOT) -> scope or nil
;;      What the command can write, as a list of paths, or nil when it cannot
;;      be established.  Nil is not an error: it selects the confirming
;;      regime.  A write to a gitignored path cannot be established even
;;      inside the root, and outside a git repository no write is proved at
;;      all: the snapshot is what makes a write revertible, and the regime
;;      asks exactly where it ends (ADR 16).  A command that writes nothing
;;      is proved everywhere.
;;
;; `pearl-tools-shell-confirm' (COMMAND SCOPE REASON) -> boolean
;;      Ask the user, showing the command, its scope and the model's reason.
;;      The prompt says what the answer costs: a command outside the proved
;;      scope leaves the turn's snapshot, so the turn cannot take it back, and
;;      confirming it is choosing an action over an undo (ADR 16, ADR 23).
;;      Returns nil without asking when the buffer is in unattended mode, in
;;      which case the caller refuses instead of assuming yes.
;;
;; `pearl-tools-shell-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

;;; Code:

(require 'cl-lib)
(require 'seq)

(require 'pearl-contract)
(require 'pearl-proc)
(require 'pearl-tool)
(require 'pearl-undo)

(defvar pearl-tools-shell-attended (not noninteractive)
  "Non-nil when a person can answer a question about a command.
A batch run, a timer and a derived session set it to nil, and a command
whose write scope is not proved is then refused rather than assumed
approved: nobody is there to read the reason the model gave.")

(defcustom pearl-tools-shell-timeout 600
  "Seconds a command may run before it is killed.
A build can take minutes and a download can take longer; a command that
hangs holds the whole turn, so the ceiling is generous rather than
nonexistent."
  :type 'natnum
  :group 'pearl)

(defconst pearl-tools-shell--unknowable
  '("git" "curl" "wget" "cp" "mv" "rm" "sed" "mkdir" "touch" "ln"
    "install" "chmod" "chown" "dd" "tar" "rsync" "find" "xargs" "make"
    "npm" "npx" "python" "python3" "perl" "ruby" "node" "deno" "sh"
    "bash" "zsh" "fish" "awk" "tee" "docker" "kubectl" "ssh" "scp")
  "Programs whose writes this parser does not claim to know.
The list is conservative on purpose: being wrong about a program that
does not write costs a question, and being wrong about one that does
costs a file nobody can take back.")

(defconst pearl-tools-shell--redirect-rx
  "\\(?:>>\\|>\\)[ \t]*\\([^ \t\n;|&<>]+\\)"
  "A redirection and the path it writes.")

(defconst pearl-tools-shell--tee-rx
  "\\btee\\b[ \t]+\\([^ \t\n;|&<>]+\\)"
  "A tee and the file it writes.")

(defun pearl-tools-shell--targets (command)
  "Return the paths COMMAND redirects output into."
  (let ((paths nil)
        (start 0))
    (dolist (regexp (list pearl-tools-shell--redirect-rx
                          pearl-tools-shell--tee-rx))
      (setq start 0)
      (while (and (< start (length command))
                  (string-match regexp command start))
        (push (string-trim (match-string 1 command) "['\"]" "['\"]") paths)
        (setq start (match-end 0))))
    (nreverse paths)))

(defun pearl-tools-shell--writes (command)
  "Return `:none', `:unknown' or `(:paths PATHS)' for COMMAND.
The three cases are three regimes and not two: a command that writes
nothing is proved everywhere, and one this parser does not understand is
proved nowhere."
  (let ((words (split-string command "[ \t\n;&|()]+" t))
        (targets (pearl-tools-shell--targets command)))
    (cond
     ((cl-some (lambda (word)
                 (member (file-name-nondirectory word) pearl-tools-shell--unknowable))
               words)
      '(:unknown))
     (targets
      (cons :paths targets))
     ('(:none)))))

(defun pearl-tools-shell--repository-p (root)
  "Non-nil when a git repository lives in ROOT.
The snapshot is what makes a write revertible, so a directory without one
has no proved writes at all."
  (file-exists-p (expand-file-name ".git" root)))

(defun pearl-tools-shell-scope (command root)
  "Return the paths COMMAND is proved to write under ROOT, or nil.
Nil means the scope could not be established: the command names a program
this parser does not understand, writes outside ROOT, writes a path git
ignores, or there is no repository to take a snapshot of.  A path git
cannot answer about -- an ignore rule it was unable to read, say -- is not
established either, so it leaves the scope unproved rather than proved."
  (let ((writes (pearl-tools-shell--writes command)))
    (when (and root
               (eq (car writes) :paths)
               (pearl-tools-shell--repository-p root))
      (let ((paths (mapcar (lambda (target) (expand-file-name target root))
                           (cdr writes))))
        (condition-case nil
            (when (seq-every-p
                   (lambda (path)
                     (and (file-in-directory-p path root)
                          (not (pearl-undo-ignored-p path root))))
                   paths)
              paths)
          (error nil))))))

(defun pearl-tools-shell-confirm (command scope reason)
  "Ask whether to run COMMAND, showing SCOPE and the model's REASON.
The question says what the answer costs, because the cost is the whole
point: a command outside the proved scope leaves the turn's snapshot, so
confirming it chooses the action over the undo."
  (when pearl-tools-shell-attended
    (yes-or-no-p
     (concat (format "Run this command?\n\n  %s\n\n" command)
             (format "The model's reason: %s\n\n"
                     (or reason "no reason was given"))
             (if scope
                 (format (concat "It writes %s, which the turn's snapshot"
                                 " does not cover, so undoing the turn will"
                                 " not take it back.\n\n")
                         (mapconcat #'identity scope ", "))
               (concat "Its write scope cannot be established, so undoing"
                       " the turn will not take it back.\n\n"))
             "Run it anyway? "))))

(defun pearl-tools-shell--failed (message &rest extra)
  "Return a result whose error is MESSAGE, carrying EXTRA."
  (append (list :call nil :content "" :error message) extra))

(defun pearl-tools-shell--result (run scope preimage proved)
  "Return the tool result for RUN, with SCOPE, PREIMAGE and PROVED."
  (let ((out (or (plist-get run :stdout) "")))
    (list :call nil
          :content out
          :error nil
          :status (plist-get run :status)
          :stdout out
          :stderr (or (plist-get run :stderr) "")
          :scope scope
          :revertible (if proved
                          t
                        (and preimage (pearl-undo-preimage-revertible-p preimage)))
          :preimage preimage)))

(defun pearl-tools-shell-run (arguments context)
  "Run the shell ARGUMENTS describe, in the project CONTEXT names."
  (let* ((arguments (pearl-tool-normalize-arguments arguments))
         (command (plist-get arguments :command))
         (reason (plist-get arguments :reason))
        (root (plist-get context :root)))
    (cond
     ((not (stringp command))
      (pearl-tools-shell--failed "No command was given."))
     ((null root)
      (pearl-tools-shell--failed
       (concat "No project root: a command has to run somewhere a"
               " snapshot can see it.")))
     (t
      (let* ((writes (pearl-tools-shell--writes command))
             (scope (pearl-tools-shell-scope command root))
             (proved (or (eq (car writes) :none) scope)))
        (if (not (or proved (pearl-tools-shell-confirm command scope reason)))
            (pearl-tools-shell--failed
             (concat "This command could not be proved to stay inside the"
                     " project, and nobody confirmed it, so nothing ran.")
             :scope scope
             :revertible nil)
          (let ((preimage (when (and (not proved) scope)
                            (pearl-undo-preimage root scope)))
                (run (condition-case err
                         (pearl-proc-run (or shell-file-name "/bin/sh")
                                         (list "-c" command)
                                         :directory root
                                         :timeout pearl-tools-shell-timeout)
                       (error (list :status 127
                                    :stdout ""
                                    :stderr (error-message-string err))))))
            (pearl-tools-shell--result run scope preimage proved))))))))

(defun pearl-tools-shell-declaration ()
  "Return the declaration the shell tool is registered with."
  (pearl-tool-define
   "shell"
   '(:description
     "Run a shell command in the project root and return its output. A command that writes outside the project is refused unless the user confirms it."
     :arguments
     ((command :required t :description "The command to run")
      (reason :required t :description "Why this command, in one sentence the user can answer")))
   #'pearl-tools-shell-run))

(defun pearl-tools-shell-install (registry)
  "Add the shell tool to REGISTRY, once."
  (if (seq-some (lambda (tool) (equal "shell" (plist-get tool :name))) registry)
      registry
    (append registry (list (pearl-tools-shell-declaration)))))

(provide 'pearl-tools-shell)

;;; pearl-tools-shell.el ends here
