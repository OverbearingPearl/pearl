;;; console-tests.el --- stories about the surface  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console is the only place the user sees a run, so it has to be
;; cheap per delta and honest about what it shows.

;;; Code:

(require 'support-tests)
(require 'pearl-app)
(require 'pearl-provider)
(require 'pearl-contract)
(require 'pearl-event)
(require 'pearl-session-jsonl)
(require 'cl-lib)
(require 'ert-gwt)

(defun console-tests--wait-for (console text)
  "Wait for TEXT to appear in CONSOLE; return non-nil on success."
  (support-tests-wait
   (lambda ()
     (with-current-buffer console
       (string-match-p (regexp-quote text) (buffer-string))))
   10))

(defun console-tests--prompt (request)
  "Return REQUEST's system message text, the prompt the model was given."
  (pearl-contract-text (car (plist-get request :messages))))

(defun console-tests--file-text (file)
  "Return FILE's contents as a string."
  (with-temp-buffer
    (insert-file-contents file)
    (buffer-string)))

;; A conversation happens in one buffer: the user's message and the reply
;; are both there, in the order they happened, in the console's own mode.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "story")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "hello") (console-tests--wait-for console "one")))
  (:then (with-current-buffer console (derived-mode-p 'pearl-console-mode)))
  (:then (with-current-buffer console (string-match-p "hello" (buffer-string))))
  (:then (with-current-buffer console (string-match-p "one" (buffer-string))))
  (:then (with-current-buffer console (< (string-match "hello" (buffer-string)) (string-match "one" (buffer-string))))))

;; Abort puts the queue back in the editor, in the order the loop handed
;; it back, and nothing arrives after it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((delay 0.5) (text "late") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "abort")))
           (queued nil)))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "first") (console-tests--wait-for console "first") (pearl-console-send console "second") (setq queued (pearl-console-abort console)) (sleep-for 0.3)))
  (:then (member "second" (mapcar (lambda (message) (plist-get message :content)) queued)))
  (:then (with-current-buffer console (string-match-p "second" (buffer-string))))
  (:then (with-current-buffer console (not (string-match-p "late" (buffer-string))))))

(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "status")))))
  (:when (pearl-console-status console "reading a.el"))
  (:then (equal "reading a.el" (pearl-console-status console))))

;; The status line is output, not a cage: it is read-only where it sits,
;; and the user can still send after it was written.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "readonly")))))
  (:when (progn (pearl-app-wire console) (pearl-console-status console "working") (pearl-console-send console "hello") (console-tests--wait-for console "one")))
  (:then (with-current-buffer console (text-property-any (point-min) (point-max) 'read-only t)))
  (:then (with-current-buffer console (string-match-p "one" (buffer-string)))))

;; A message sent while a run is in flight steers instead of interrupting:
;; the console decides once, and the reply order shows the decision.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((delay 0.2) (text "one") (stop "end") (text "two") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "steer")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "first") (pearl-console-send console "second") (console-tests--wait-for console "two")))
  (:then (with-current-buffer console (< (string-match "one" (buffer-string)) (string-match "second" (buffer-string)))))
  (:then (with-current-buffer console (< (string-match "second" (buffer-string)) (string-match "two" (buffer-string))))))

;; The keys are the editor's: RET sends, C-c C-k aborts, C-c C-m picks the
;; model, C-c C-r takes the turn back.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (console (pearl-app-console (pearl-app-open root "keys")))))
  (:when (pearl-app-wire console))
  (:then (with-current-buffer console
           (let ((keys (mapcar (lambda (key)
                                 (lookup-key (current-local-map) (kbd key)))
                               '("RET" "C-c C-k" "C-c C-m" "C-c C-r"))))
             (and (seq-every-p #'commandp keys)
                  (equal 4 (length (delete-dups keys))))))))

;; M4: 600 deltas cost the main thread no more than 0.3 ms each, because
;; rendering is batched and fontified once, not once per token.  The number is
;; the acceptance; PEARL_PERF_SCALE only lets a loaded machine say it is
;; loaded.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "chunks")))
           (event (pearl-event-new 'message-update '(:message "m" :delta "x")))
           (elapsed nil)))
  (:when (progn (setq elapsed (car (benchmark-run 600 (pearl-console-insert-event console event)))) (pearl-console-redraw console)))
  (:then (<= (/ elapsed 600) (* 0.0003 (support-tests-perf-scale))))
  (:then (with-current-buffer console (equal 600 (count-matches "x" (point-min) (point-max))))))

;; M4: paging back through a 100k-line transcript stays under 50 ms per
;; screenful, because the buffer is not re-fontified on every scroll.  Same
;; rule as above: the millisecond is the acceptance, the scale is the
;; machine's confession.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "paging")))
           (event (pearl-event-new 'message-update (list :message "m" :delta (apply #'concat (make-list 100000 "line\n")))))
           (elapsed nil) (start nil)))
  (:when (progn (pearl-console-insert-event console event) (pearl-console-redraw console) (save-window-excursion (set-window-buffer nil console) (with-current-buffer console (goto-char (point-min))) (setq elapsed (car (benchmark-run 1 (scroll-up-command)))) (setq start (window-start)))))
  (:then (<= elapsed (* 0.05 (support-tests-perf-scale))))
  (:then (> start 1)))

;; A user who scrolled up stays up; a user at the end follows the output.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "scroll")))
           (event (pearl-event-new 'message-update '(:message "m" :delta "more\n")))))
  (:when (progn (pearl-console-insert-event console (pearl-event-new 'message-update '(:message "m" :delta "first\n"))) (pearl-console-redraw console) (with-current-buffer console (goto-char (point-min))) (pearl-console-insert-event console event) (pearl-console-redraw console)))
  (:then (with-current-buffer console (equal (point-min) (point))))
  (:then (progn (with-current-buffer console (goto-char (point-max))) (pearl-console-insert-event console event) (pearl-console-redraw console) (with-current-buffer console (equal (point-max) (point))))))

;; M6: kill Emacs, reopen, and the history is there, in the order it
;; happened — a second console on the same root reads the record back.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (first (pearl-app-console (pearl-app-open root "restart")))
           (second nil)))
  (:when (progn (pearl-app-wire first) (pearl-console-send first "hello") (console-tests--wait-for first "one") (pearl-console-close first) (setq second (pearl-app-console (pearl-app-open root "restart"))) (pearl-app-wire second)))
  (:then (with-current-buffer second (string-match-p "hello" (buffer-string))))
  (:then (with-current-buffer second (string-match-p "one" (buffer-string))))
  (:then (with-current-buffer second (< (string-match "hello" (buffer-string)) (string-match "one" (buffer-string))))))

;; The promise, end to end: the agent edited a file through a buffer, and
;; one C-/ in that buffer takes the whole edit back (M0.5).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "promised.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "promise"))))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10)))
  (:then (equal "new\n" (console-tests--file-text file)))
  (:then (with-current-buffer (find-file-noselect file) (undo 1)))
  (:then (equal "old\n" (with-current-buffer (find-file-noselect file) (buffer-string)))))

;; M9: a turn can be rolled back after a restart: the patch is on the
;; record, the record is on disk, and the restore reads it back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "durable.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (first (pearl-app-console (pearl-app-open root "durable")))
           (second nil) (restored nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-app-wire first) (pearl-console-send first "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (pearl-console-close first) (setq second (pearl-app-console (pearl-app-open root "durable"))) (pearl-app-wire second) (setq restored (pearl-console-rollback second))))
  (:then (equal (list file) restored))
  (:then (equal "old\n" (console-tests--file-text file))))

;; M9: a file the user changed since the turn is refused by name instead
;; of overwritten: the status line says which file, and nothing is written.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "guarded.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (first (pearl-app-console (pearl-app-open root "guarded")))
           (second nil) (restored nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-app-wire first) (pearl-console-send first "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (pearl-console-close first) (with-temp-file file (insert "mine\n")) (setq second (pearl-app-console (pearl-app-open root "guarded"))) (pearl-app-wire second) (setq restored (pearl-console-rollback second))))
  (:then (null restored))
  (:then (string-match-p "guarded.el" (pearl-console-status second)))
  (:then (equal "mine\n" (console-tests--file-text file))))

;; A plan over more than one path is confirmed before it is applied, and a
;; refused confirmation writes nothing: both files stay as the turn left them.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (one (expand-file-name "one.el" root))
           (two (expand-file-name "two.el" root))
           (asked 0)
           ((symbol-function 'yes-or-no-p)
            (lambda (_question) (setq asked (1+ asked)) nil))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path one) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) (list 'tool-call "c2" "edit" (list (cons 'path two) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "confirm")))
           (restored nil))
          (with-temp-file one (insert "old\n"))
          (with-temp-file two (insert "old\n")))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "change both") (support-tests-wait (lambda () (and (equal "new\n" (console-tests--file-text one)) (equal "new\n" (console-tests--file-text two)))) 10) (setq restored (pearl-console-rollback console))))
  (:then (null restored))
  (:then (equal 1 asked))
  (:then (equal "new\n" (console-tests--file-text one)))
  (:then (equal "new\n" (console-tests--file-text two))))

;; A confirmation the user gives reverses the whole plan in one step: both
;; paths are named back and both files are what they were before the turn.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (one (expand-file-name "one.el" root))
           (two (expand-file-name "two.el" root))
           ((symbol-function 'yes-or-no-p) (lambda (_question) t))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path one) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) (list 'tool-call "c2" "edit" (list (cons 'path two) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "confirm-yes")))
           (restored nil))
          (with-temp-file one (insert "old\n"))
          (with-temp-file two (insert "old\n")))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "change both") (support-tests-wait (lambda () (and (equal "new\n" (console-tests--file-text one)) (equal "new\n" (console-tests--file-text two)))) 10) (setq restored (pearl-console-rollback console))))
  (:then (equal (list one two) (sort restored #'string<)))
  (:then (equal "old\n" (console-tests--file-text one)))
  (:then (equal "old\n" (console-tests--file-text two))))

;; M7: the status line says which regime is active, so nobody has to
;; guess whether a command was proved in-scope or refused.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (outside (ert-gwt--temp-file "pearl-outside"))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command (format "echo x > %s" outside)) (cons 'reason "write outside"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "regime")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "write it") (console-tests--wait-for console "done")))
  (:then (string-match-p "confirm" (pearl-console-status console)))
  (:then (equal "" (console-tests--file-text outside))))

;; Close is safe to run twice: shutdown runs it from the kill hook too.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "close")))))
  (:when (progn (pearl-app-wire console) (pearl-console-close console) (pearl-console-close console)))
  (:then (not (buffer-live-p console))))

;; A console whose tools are missing asks once, not on every wire: the
;; miss list is a question, not a nag.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-tool-registry nil)
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "missing")))
           (asked 0)
           ((symbol-function 'y-or-n-p) (lambda (_question) (setq asked (1+ asked)) t))))
  (:when (progn (pearl-app-wire console) (pearl-app-wire console)))
  (:then (equal 1 asked)))

;; ADR 10's third rule, the wiring half: a console killed mid-run without
;; its close hook reports the death instead of dropping the chain, and the
;; run asks for nothing more.  The hook is disabled so the guard backstop
;; is what fires, not the clean abort (ADR 15).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((delay 0.4) (text "late") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "dead")))
           (reports nil)
           ((symbol-function 'message) (lambda (format &rest args) (when format (push (apply #'format format args) reports)) nil))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "go") (console-tests--wait-for console "go") (let ((kill-buffer-hook nil)) (kill-buffer console)) (sleep-for 0.5)))
  (:then (seq-find (lambda (report) (string-match-p "killed" report)) reports))
  (:then (equal 1 (length (pearl-provider-fake-requests pearl-console-provider)))))

;; Output is fontified once, where it lands: the code a reply carries is
;; readable as code, not re-fontified on every redisplay of a long buffer.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "fontify")))
           (event (pearl-event-new 'message-update '(:message "m" :delta "(defvar pearl-x 1)\n")))))
  (:when (progn (pearl-console-insert-event console event) (pearl-console-redraw console)))
  (:then (with-current-buffer console
           (goto-char (point-min))
           (and (search-forward "(defvar" nil t)
                (eq (get-text-property (match-beginning 0) 'face)
                    (pearl-console-face 'assistant))))))

;; ADR 6's third seam: the console mode's hook runs when a console opens,
;; so an extension attaches the way Emacs extensions attach.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (hooked 0)
           (hook (lambda () (setq hooked (1+ hooked)))))
          (add-hook 'pearl-console-mode-hook hook))
  (:when (pearl-app-console (pearl-app-open root "hook")))
  (:then (equal 1 hooked))
  (:cleanup (remove-hook 'pearl-console-mode-hook hook)))

;; The status line is non-sticky: text typed after it inherits nothing,
;; while the line itself stays read-only.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "sticky"))) (pos nil)))
  (:when (progn (pearl-console-status console "working") (with-current-buffer console (goto-char (point-min)) (search-forward "working") (setq pos (match-beginning 0)) (insert "x"))))
  (:then (with-current-buffer console (get-text-property pos 'read-only)))
  (:then (with-current-buffer console (not (get-text-property (1- (point)) 'read-only)))))

;; Killing the console buffer runs the close hook: the run is aborted, the
;; store is closed, and nothing asks for another request.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((delay 0.4) (text "late") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "kill")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "go") (console-tests--wait-for console "go") (kill-buffer console) (sleep-for 0.3)))
  (:then (not (buffer-live-p console)))
  (:then (equal 1 (length (pearl-provider-fake-requests pearl-console-provider)))))

;; M7's other half: a provably in-root write leaves the status line naming
;; the project regime, so the user knows which one is active.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command "echo hi > out.txt") (cons 'reason "write inside"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "regime-project")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "write it") (console-tests--wait-for console "done")))
  (:then (string-match-p "project" (pearl-console-status console))))

;; M3: a message streams into the buffer as it arrives: the first delta is
;; visible before the second one exists, not rendered once at the end.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "first") (delay 0.4) (text "second") (stop "end"))))
           (console (pearl-app-console (pearl-app-open root "stream")))))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "go") (console-tests--wait-for console "first")))
  (:then (with-current-buffer console (string-match-p "first" (buffer-string))))
  (:then (with-current-buffer console (not (string-match-p "second" (buffer-string)))))
  (:then (console-tests--wait-for console "second")))

;; An undo is another entry, not a rewrite: a rollback appends an undo
;; entry to the record on disk, and everything before it stays as it was.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "recorded.el" root))
           (pearl-app-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-app-console (pearl-app-open root "recorded")))
           (path (pearl-session-jsonl-path-for root "recorded"))
           (before nil) (after nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-app-wire console) (pearl-console-send console "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (setq before (pearl-session-jsonl-read (pearl-session-jsonl-open path))) (pearl-console-rollback console) (setq after (pearl-session-jsonl-read (pearl-session-jsonl-open path)))))
  (:then (equal (1+ (length before)) (length after)))
  (:then (equal before (butlast after)))
  (:then (eq 'undo (plist-get (car (last after)) :kind))))

;; A console with no provider of its own reaches the endpoint the user
;; configured, with the key it resolved, and what comes back lands in the
;; buffer.  This is the only story where the console is wired to the real
;; adapter instead of a fake.
(ert-gwt-deftest
  (:given ((request nil)
           (server (support-tests-http-request-then
                    (lambda (client head)
                      (setq request head)
                      (process-send-string
                       client (concat (support-tests-sse-head)
                                      (support-tests-openai-chunk
                                       '(:content "hi there") "stop")
                                      (support-tests-openai-done))))))
           (port (process-contact server :service))
           (pearl-provider-sse-base-url (format "http://127.0.0.1:%d/v1" port))
           (pearl-console-model-default "story-model")
           (console (pearl-app-console (pearl-app-open (support-tests-temp-root) "endpoint"))))
          (setenv "PEARL_API_KEY" "sk-story"))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (console-tests--wait-for console "hi there")))
  (:then (string-match-p "POST /v1/chat/completions" request))
  (:then (string-match-p "Bearer sk-story" request))
  (:then (with-current-buffer console
           (string-match-p "hi there" (buffer-string))))
  (:cleanup (setenv "PEARL_API_KEY" nil) (support-tests-tcp-close server)))

;; Two consoles on one root are two sessions: the suffix names the record, so
;; a second console does not append to the first one's history.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-app-provider (pearl-provider-fake '((text "one") (stop "end")
                                                      (text "one") (stop "end"))))
           (left (pearl-app-console (pearl-app-open root "left")))
           (right (pearl-app-console (pearl-app-open root "right")))
           (left-record nil) (right-record nil)))
  (:when (progn (pearl-app-wire left) (pearl-app-wire right)
                (pearl-console-send left "to the left")
                (console-tests--wait-for left "one")
                (pearl-console-send right "to the right")
                (console-tests--wait-for right "one")
                (setq left-record
                      (console-tests--file-text
                       (pearl-session-jsonl-path-for root "left")))
                (setq right-record
                      (console-tests--file-text
                       (pearl-session-jsonl-path-for root "right")))))
  (:then (string-match-p "to the left" left-record))
  (:then (string-match-p "to the right" right-record))
  (:then (not (string-match-p "to the right" left-record)))
  (:then (not (string-match-p "to the left" right-record))))

;; The model is a setting a user changes mid-conversation: the choice lives in
;; the buffer, travels in the next request, and does not rewrite the transcript
;; above it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-model-default "from-the-custom")
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "model")))
           (requests nil)))
  (:when (progn (pearl-app-wire console)
                (pearl-console-set-model console "picked-in-the-buffer")
                (pearl-console-send console "hello")
                (support-tests-wait (lambda () (pearl-provider-fake-requests provider)) 10)
                (setq requests (pearl-provider-fake-requests provider))))
  (:then (equal "picked-in-the-buffer" (plist-get (car (last requests)) :model))))

;; A console with no model asks once and remembers the answer: the prompt is
;; the first screen of a session, not one per turn.
(ert-gwt-deftest
  (:given ((pearl-console-model-default nil)
           (pearl-console-models '("a-model" "another-model"))
           (asked 0)
           (first nil) (second nil)
           (console (pearl-app-console (pearl-app-open (support-tests-temp-root) "model-ask"))))
          (cl-letf (((symbol-function 'completing-read)
                     (lambda (&rest _) (setq asked (1+ asked)) "chosen")))))
  (:when (progn (setq first (pearl-console-model console))
                (setq second (pearl-console-model console))))
  (:then (equal "chosen" first))
  (:then (equal "chosen" second))
  (:then (= 1 asked)))

;; The key is resolved once per console and kept in the buffer: a second turn
;; does not ask again, and nothing about it is rendered (ADR 17).
(ert-gwt-deftest
  (:given ((resolved 0)
           (first nil) (second nil)
           (console (pearl-app-console (pearl-app-open (support-tests-temp-root) "key"))))
          (cl-letf (((symbol-function 'pearl-provider-sse-api-key)
                     (lambda (_host) (setq resolved (1+ resolved)) "sk-test")))))
  (:when (progn (setq first (pearl-console-api-key console))
                (setq second (pearl-console-api-key console))))
  (:then (equal "sk-test" first))
  (:then (equal "sk-test" second))
  (:then (= 1 resolved))
  (:then (with-current-buffer console (not (string-match-p "sk-test" (buffer-string))))))

;; The project's AGENTS.md is the project's half of the prompt: a console
;; wired on a root that has one sends its text ahead of the conversation.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "AGENTS.md" root))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "project")))
           (prompt nil))
          (with-temp-file file
            (insert "Run the tests with `make test' before you commit.\n")))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (support-tests-wait
                 (lambda () (pearl-provider-fake-requests provider)) 10)
                (setq prompt
                      (console-tests--prompt
                       (car (pearl-provider-fake-requests provider))))))
  (:then (string-match-p "make test" prompt)))

;; A root without one still gets a prompt: the shipped text, which names the
;; tools rather than the project's habits.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "no-project")))
           (prompt nil))
          (with-temp-file (expand-file-name "AGENTS.md" root)
            (insert "")))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (support-tests-wait
                 (lambda () (pearl-provider-fake-requests provider)) 10)
                (setq prompt
                      (console-tests--prompt
                       (car (pearl-provider-fake-requests provider))))))
  (:then (string-match-p "edit" prompt))
  (:then (string-match-p "undo" prompt)))

;; The user's file is not the project's: it is read first, and it is read even
;; when the repository has nothing to say.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (user-file (ert-gwt--temp-file "pearl-user-instructions.md"))
           (pearl-app-user-instructions-file user-file)
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "both-files")))
           (prompt nil))
          (with-temp-file user-file
            (insert "Answer in Chinese.  Never amend a commit.\n"))
          (with-temp-file (expand-file-name "AGENTS.md" root)
            (insert "Run the tests with `make test'.\n")))
  (:when (progn (pearl-app-wire console)
                (pearl-console-send console "hello")
                (support-tests-wait
                 (lambda () (pearl-provider-fake-requests provider)) 10)
                (setq prompt
                      (console-tests--prompt
                       (car (pearl-provider-fake-requests provider))))))
  (:then (string-match-p "Never amend a commit" prompt))
  (:then (< (string-match "Never amend a commit" prompt)
            (string-match "Run the tests" prompt))))

;; A file bigger than the limit is cut and says so; a short one is not, and
;; the marker is not something a user has to remember to read past.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (user-file (ert-gwt--temp-file "pearl-huge-instructions.md"))
           (pearl-app-user-instructions-file user-file)
           (sources nil))
          (with-temp-file user-file
            (insert (make-string 40000 ?x)))
          (with-temp-file (expand-file-name "AGENTS.md" root)
            (insert "short and useful\n")))
  (:when (setq sources (pearl-app-prompt-sources root)))
  (:then (< (length (plist-get (car sources) :text)) 40000))
  (:then (string-match-p "truncated" (plist-get (car sources) :text)))
  (:then (not (string-match-p "truncated"
                              (plist-get (cadr sources) :text)))))

;; Two consoles on one root with one suffix are one session, and the second
;; one is told so: appending into the first one's record would read as a
;; conversation answering itself (ADR 22).  The refusal does not disturb the
;; console that already holds it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (first (pearl-app-console (pearl-app-open root "same-suffix")))
           (second (pearl-app-console (pearl-app-open root "same-suffix")))
           (message nil)))
  (:when (progn
           (pearl-app-wire first)
           (setq message
                 (condition-case err
                     (progn (pearl-app-wire second) nil)
                   (user-error (error-message-string err))))))
  (:then (string-match-p "same-suffix" message))
  (:then (string-match-p "suffix" message))
  (:then (progn (pearl-console-send first "hello")
                (console-tests--wait-for first "one")))
  (:cleanup (pearl-console-close first) (pearl-console-close second)))

;; Two consoles on two roots are two prompts.  The prompt is session data, not
;; a defvar the console writes, so wiring the second one cannot rewrite the
;; first one's conversation (ADR 20).
(ert-gwt-deftest
  (:given ((left-root (support-tests-temp-root))
           (right-root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (left (pearl-app-console (pearl-app-open left-root "left-prompt")))
           (right (pearl-app-console (pearl-app-open right-root "right-prompt")))
           (requests nil))
          (with-temp-file (expand-file-name "AGENTS.md" left-root)
            (insert "The left rule.\n"))
          (with-temp-file (expand-file-name "AGENTS.md" right-root)
            (insert "The right rule.\n")))
  (:when (progn (pearl-app-wire left)
                (pearl-app-wire right)
                (pearl-console-send left "to the left")
                (console-tests--wait-for left "one")
                (pearl-console-send right "to the right")
                (console-tests--wait-for right "one")
                (setq requests (pearl-provider-fake-requests provider))))
  (:then (equal 2 (length requests)))
  (:then (string-match-p "The left rule" (console-tests--prompt (car requests))))
  (:then (string-match-p "The right rule"
                         (console-tests--prompt (cadr requests))))
  (:then (not (string-match-p "The right rule"
                              (console-tests--prompt (car requests))))))

;; An event becomes sections, and the section says what the text is: a tool
;; result is not the assistant's words, and the face is looked up from the role
;; instead of being named at the formatter (ADR 26).
(ert-gwt-deftest
  (:given ((event (pearl-event-new 'tool-execution-start
                                  '(:run "r1" :id "c1" :name "read")))
           (sections nil)))
  (:when (setq sections (pearl-console-render event)))
  (:then (equal 'tool (plist-get (car sections) :role)))
  (:then (string-match-p "read" (plist-get (car sections) :text)))
  (:then (and (pearl-console-face 'tool)
              (not (eq (pearl-console-face 'tool)
                       (pearl-console-face 'assistant))))))

;; A fold is view state: folding hides text in this buffer and writes nothing to
;; the record, so a second reader sees the same conversation unfolded (ADR 26).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "fold")))
           (before nil) (after nil)))
  (:when (progn
           (pearl-app-wire console)
           (pearl-console-send console "hello")
           (console-tests--wait-for console "one")
           (setq before (length (pearl-session-jsonl-read
                                 (pearl-session-jsonl-open
                                  (pearl-session-jsonl-path-for root "fold")))))
           (pearl-console-fold-toggle console "fold-me")
           (setq after (length (pearl-session-jsonl-read
                                (pearl-session-jsonl-open
                                 (pearl-session-jsonl-path-for root "fold")))))))
  (:then (> before 0))
  (:then (equal before after)))

;; The state is derived from the events: a tool starting is its own state, not
;; the same "busy" as a stream that has not begun.
(ert-gwt-deftest
  (:given ((console (pearl-app-console (pearl-app-open (support-tests-temp-root) "state-tool")))))
  (:when (pearl-console-insert-event
          console
          (pearl-event-new 'tool-execution-start
                           '(:run "r1" :id "c1" :name "read"))))
  (:then (eq 'tool (pearl-console-state console))))

;; Nothing yet is a state of its own, and idle is nil: a timer would call the
;; first one "busy" and never know when to stop saying it (ADR 26).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (provider (pearl-provider-fake '((delay 0.4) (text "one") (stop "end"))))
           (pearl-app-provider provider)
           (console (pearl-app-console (pearl-app-open root "state-waiting")))
           (at-submit nil)))
  (:when (progn
           (pearl-app-wire console)
           (pearl-console-send console "hello")
           (setq at-submit (pearl-console-state console))
           (console-tests--wait-for console "one")
           (support-tests-wait
            (lambda () (null (pearl-console-state console))))))
  (:then (eq 'waiting at-submit))
  (:then (null (pearl-console-state console))))

;; Wiring a second console onto the same session file is refused before it
;; starts talking: two writers on one conversation is a conversation that
;; answers itself (ADR 22).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (first (pearl-app-wire (pearl-app-open root)))
           (err nil)))
  (:when (condition-case e (pearl-app-wire (pearl-app-open root))
           (user-error (setq err e))))
  (:then (eq 'user-error (car err)))
  (:then (string-match-p "session" (error-message-string err))))

;; The undo page is the root's, so the console has to say whose node a row is:
;; the same renderer, two roles, one of which means "not this conversation".
(ert-gwt-deftest
  (:given ((a (pearl-session-new)) (b (pearl-session-new))
           (sections nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'message '(:content "mine")))
           (pearl-session-append b (pearl-contract-entry 'message '(:content "theirs")))
           (setq sections (pearl-console-render-timeline
                           (pearl-undo-timeline (list a b)) a))))
  (:then (equal '(timeline timeline-other)
                (mapcar (lambda (section) (plist-get section :role)) sections)))
  (:then (string-match-p "mine" (plist-get (car sections) :text)))
  (:then (string-match-p "theirs" (plist-get (cadr sections) :text)))
  (:then (facep (pearl-console-face 'timeline)))
  (:then (facep (pearl-console-face 'timeline-other))))

;; A page that is not following counts instead of moving, and refresh is the
;; gesture that both inserts what it counted and turns following back on.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (console (pearl-app-console (pearl-app-open root "timeline")))
           (a (pearl-session-new))
           (nodes nil) (inserted nil) (pending nil) (hidden nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'message '(:content "first")))
           (setq nodes (pearl-undo-timeline (list a)))
           (pearl-console-refresh console nodes)
           (pearl-console-follow console nil)
           (pearl-session-append a (pearl-contract-entry 'message '(:content "second")))
           (setq nodes (pearl-undo-timeline (list a)))
           (pearl-console-note console nodes)
           (setq pending (pearl-console-pending console))
           (pearl-console-redraw console)
           (setq hidden (with-current-buffer console
                          (not (string-match-p "second" (buffer-string)))))
           (setq inserted (pearl-console-refresh console nodes))
           (pearl-console-redraw console)))
  (:then (equal 1 pending))
  (:then hidden)
  (:then (equal 1 inserted))
  (:then (zerop (pearl-console-pending console)))
  (:then (pearl-console-follow console))
  (:then (with-current-buffer console (string-match-p "second" (buffer-string)))))

;; A reader is never moved by a refresh: the row it is on stays the row it is
;; on, because what went in went in above the insertion point or below it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (console (pearl-app-console (pearl-app-open root "follow")))
           (a (pearl-session-new))
           (line nil) (still nil) (following nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'message '(:content "first")))
           (pearl-console-refresh console (pearl-undo-timeline (list a)))
           (pearl-console-redraw console)
           (with-current-buffer console
             (goto-char (point-min))
             (setq line (buffer-substring (line-beginning-position)
                                          (line-end-position))))
           (pearl-session-append a (pearl-contract-entry 'message '(:content "second")))
           (pearl-console-refresh console (pearl-undo-timeline (list a)))
           (pearl-console-redraw console)
           (with-current-buffer console
             (setq still (equal line (buffer-substring (line-beginning-position)
                                                       (line-end-position))))
             (setq following (pearl-console-follow console)))))
  (:then still)
  (:then following)
  (:then (with-current-buffer console (string-match-p "second" (buffer-string)))))
(provide 'console-tests)

;;; console-tests.el ends here
