;;; console-tests.el --- stories about the surface  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console is the only place the user sees a run, so it has to be
;; cheap per delta and honest about what it shows.

;;; Code:

(require 'support-tests)
(require 'pearl-console)
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

(defun console-tests--file-text (file)
  "Return FILE's contents as a string."
  (with-temp-buffer
    (insert-file-contents file)
    (buffer-string)))

;; A conversation happens in one buffer: the user's message and the reply
;; are both there, in the order they happened, in the console's own mode.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-console-open root "story"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "hello") (console-tests--wait-for console "one")))
  (:then (with-current-buffer console (derived-mode-p 'pearl-console-mode)))
  (:then (with-current-buffer console (string-match-p "hello" (buffer-string))))
  (:then (with-current-buffer console (string-match-p "one" (buffer-string))))
  (:then (with-current-buffer console (< (string-match "hello" (buffer-string)) (string-match "one" (buffer-string))))))

;; Abort puts the queue back in the editor, in the order the loop handed
;; it back, and nothing arrives after it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((delay 0.5) (text "late") (stop "end"))))
           (console (pearl-console-open root "abort"))
           (queued nil)))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "first") (console-tests--wait-for console "first") (pearl-console-send console "second") (setq queued (pearl-console-abort console)) (sleep-for 0.3)))
  (:then (member "second" (mapcar (lambda (message) (plist-get message :content)) queued)))
  (:then (with-current-buffer console (string-match-p "second" (buffer-string))))
  (:then (with-current-buffer console (not (string-match-p "late" (buffer-string))))))

(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "status"))))
  (:when (pearl-console-status console "reading a.el"))
  (:then (equal "reading a.el" (pearl-console-status console))))

;; The status line is output, not a cage: it is read-only where it sits,
;; and the user can still send after it was written.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-console-open root "readonly"))))
  (:when (progn (pearl-console-wire console) (pearl-console-status console "working") (pearl-console-send console "hello") (console-tests--wait-for console "one")))
  (:then (with-current-buffer console (text-property-any (point-min) (point-max) 'read-only t)))
  (:then (with-current-buffer console (string-match-p "one" (buffer-string)))))

;; A message sent while a run is in flight steers instead of interrupting:
;; the console decides once, and the reply order shows the decision.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((delay 0.2) (text "one") (stop "end") (text "two") (stop "end"))))
           (console (pearl-console-open root "steer"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "first") (pearl-console-send console "second") (console-tests--wait-for console "two")))
  (:then (with-current-buffer console (< (string-match "one" (buffer-string)) (string-match "second" (buffer-string)))))
  (:then (with-current-buffer console (< (string-match "second" (buffer-string)) (string-match "two" (buffer-string))))))

;; The keys are the editor's: RET sends, C-c C-k aborts, C-c C-m picks the
;; model, C-c C-r takes the turn back.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (console (pearl-console-open root "keys"))))
  (:when (pearl-console-wire console))
  (:then (with-current-buffer console (lookup-key (current-local-map) (kbd "RET"))))
  (:then (with-current-buffer console (lookup-key (current-local-map) (kbd "C-c C-k"))))
  (:then (with-current-buffer console (lookup-key (current-local-map) (kbd "C-c C-m"))))
  (:then (with-current-buffer console (lookup-key (current-local-map) (kbd "C-c C-r")))))

;; M4: 600 deltas cost the main thread no more than 0.3 ms each, because
;; rendering is batched and fontified once, not once per token.
(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "chunks"))
           (event (pearl-event-new 'message-update '(:message "m" :delta "x")))
           (elapsed nil)))
  (:when (progn (setq elapsed (car (benchmark-run 600 (pearl-console-insert-event console event)))) (pearl-console-redraw console)))
  (:then (<= (/ elapsed 600) 0.0003))
  (:then (with-current-buffer console (equal 600 (count-matches "x" (point-min) (point-max))))))

;; M4: paging back through a 100k-line transcript stays under 50 ms per
;; screenful, because the buffer is not re-fontified on every scroll.
(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "paging"))
           (event (pearl-event-new 'message-update (list :message "m" :delta (apply #'concat (make-list 100000 "line\n")))))
           (elapsed nil)))
  (:when (progn (pearl-console-insert-event console event) (pearl-console-redraw console) (save-window-excursion (set-window-buffer nil console) (with-current-buffer console (goto-char (point-min))) (setq elapsed (car (benchmark-run 1 (scroll-up-command)))))))
  (:then (<= elapsed 0.05)))

;; A user who scrolled up stays up; a user at the end follows the output.
(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "scroll"))
           (event (pearl-event-new 'message-update '(:message "m" :delta "more\n")))))
  (:when (progn (pearl-console-insert-event console (pearl-event-new 'message-update '(:message "m" :delta "first\n"))) (pearl-console-redraw console) (with-current-buffer console (goto-char (point-min))) (pearl-console-insert-event console event) (pearl-console-redraw console)))
  (:then (with-current-buffer console (equal (point-min) (point))))
  (:then (progn (with-current-buffer console (goto-char (point-max))) (pearl-console-insert-event console event) (pearl-console-redraw console) (with-current-buffer console (equal (point-max) (point))))))

;; M6: kill Emacs, reopen, and the history is there, in the order it
;; happened — a second console on the same root reads the record back.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (first (pearl-console-open root "restart"))
           (second nil)))
  (:when (progn (pearl-console-wire first) (pearl-console-send first "hello") (console-tests--wait-for first "one") (pearl-console-close first) (setq second (pearl-console-open root "restart")) (pearl-console-wire second)))
  (:then (with-current-buffer second (string-match-p "hello" (buffer-string))))
  (:then (with-current-buffer second (string-match-p "one" (buffer-string))))
  (:then (with-current-buffer second (< (string-match "hello" (buffer-string)) (string-match "one" (buffer-string))))))

;; The promise, end to end: the agent edited a file through a buffer, and
;; one C-/ in that buffer takes the whole edit back (M0.5).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "promised.el" root))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-console-open root "promise")))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10)))
  (:then (equal "new\n" (console-tests--file-text file)))
  (:then (with-current-buffer (find-file-noselect file) (undo 1)))
  (:then (equal "old\n" (with-current-buffer (find-file-noselect file) (buffer-string)))))

;; M9: a turn can be rolled back after a restart: the patch is on the
;; record, the record is on disk, and the restore reads it back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "durable.el" root))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (first (pearl-console-open root "durable"))
           (second nil) (restored nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-console-wire first) (pearl-console-send first "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (pearl-console-close first) (setq second (pearl-console-open root "durable")) (pearl-console-wire second) (setq restored (pearl-console-rollback second))))
  (:then (equal (list file) restored))
  (:then (equal "old\n" (console-tests--file-text file))))

;; M9: a file the user changed since the turn is refused by name instead
;; of overwritten: the status line says which file, and nothing is written.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "guarded.el" root))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (first (pearl-console-open root "guarded"))
           (second nil) (restored nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-console-wire first) (pearl-console-send first "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (pearl-console-close first) (with-temp-file file (insert "mine\n")) (setq second (pearl-console-open root "guarded")) (pearl-console-wire second) (setq restored (pearl-console-rollback second))))
  (:then (null restored))
  (:then (string-match-p "guarded.el" (pearl-console-status second)))
  (:then (equal "mine\n" (console-tests--file-text file))))

;; A plan over more than one path is confirmed before it is applied, and
;; a refused confirmation writes nothing.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (one (expand-file-name "one.el" root))
           (two (expand-file-name "two.el" root))
           (answer nil)
           ((symbol-function 'yes-or-no-p) (lambda (_question) answer))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path one) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) (list 'tool-call "c2" "edit" (list (cons 'path two) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-console-open root "confirm"))
           (restored nil))
          (with-temp-file one (insert "old\n"))
          (with-temp-file two (insert "old\n")))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "change both") (support-tests-wait (lambda () (and (equal "new\n" (console-tests--file-text one)) (equal "new\n" (console-tests--file-text two)))) 10) (setq restored (pearl-console-rollback console))))
  (:then (null restored))
  (:then (equal "new\n" (console-tests--file-text one)))
  (:then (setq answer t))
  (:then (equal (list one two) (sort (pearl-console-rollback console) #'string<)))
  (:then (equal "old\n" (console-tests--file-text one)))
  (:then (equal "old\n" (console-tests--file-text two))))

;; M7: the status line says which regime is active, so nobody has to
;; guess whether a command was proved in-scope or refused.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (outside (ert-gwt--temp-file "pearl-outside"))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command (format "echo x > %s" outside)) (cons 'reason "write outside"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-console-open root "regime"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "write it") (console-tests--wait-for console "done")))
  (:then (string-match-p "confirm" (pearl-console-status console)))
  (:then (equal "" (console-tests--file-text outside))))

;; Close is safe to run twice: shutdown runs it from the kill hook too.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-console-open root "close"))))
  (:when (progn (pearl-console-wire console) (pearl-console-close console) (pearl-console-close console)))
  (:then (not (buffer-live-p console))))

;; A console whose tools are missing asks once, not on every wire: the
;; miss list is a question, not a nag.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-tool-registry nil)
           (pearl-console-provider (pearl-provider-fake '((text "one") (stop "end"))))
           (console (pearl-console-open root "missing"))
           (asked 0)
           ((symbol-function 'y-or-n-p) (lambda (_question) (setq asked (1+ asked)) t))))
  (:when (progn (pearl-console-wire console) (pearl-console-wire console)))
  (:then (equal 1 asked)))

;; ADR 10's third rule, the wiring half: a console killed mid-run without
;; its close hook reports the death instead of dropping the chain, and the
;; run asks for nothing more.  The hook is disabled so the guard backstop
;; is what fires, not the clean abort (ADR 15).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((delay 0.4) (text "late") (stop "end"))))
           (console (pearl-console-open root "dead"))
           (reports nil)
           ((symbol-function 'message) (lambda (format &rest args) (when format (push (apply #'format format args) reports)) nil))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "go") (console-tests--wait-for console "go") (let ((kill-buffer-hook nil)) (kill-buffer console)) (sleep-for 0.5)))
  (:then (seq-find (lambda (report) (string-match-p "killed" report)) reports))
  (:then (equal 1 (length (pearl-provider-fake-requests pearl-console-provider)))))

;; Output is fontified once, where it lands: the code a reply carries is
;; readable as code, not re-fontified on every redisplay of a long buffer.
(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "fontify"))
           (event (pearl-event-new 'message-update '(:message "m" :delta "(defvar pearl-x 1)\n")))))
  (:when (progn (pearl-console-insert-event console event) (pearl-console-redraw console)))
  (:then (with-current-buffer console (goto-char (point-min)) (and (search-forward "(defvar" nil t) (get-text-property (match-beginning 0) 'face)))))

;; ADR 6's third seam: the console mode's hook runs when a console opens,
;; so an extension attaches the way Emacs extensions attach.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (hooked 0)
           (hook (lambda () (setq hooked (1+ hooked)))))
          (add-hook 'pearl-console-mode-hook hook))
  (:when (pearl-console-open root "hook"))
  (:then (equal 1 hooked))
  (:cleanup (remove-hook 'pearl-console-mode-hook hook)))

;; The status line is non-sticky: text typed after it inherits nothing,
;; while the line itself stays read-only.
(ert-gwt-deftest
  (:given ((console (pearl-console-open (support-tests-temp-root) "sticky")) (pos nil)))
  (:when (progn (pearl-console-status console "working") (with-current-buffer console (goto-char (point-min)) (search-forward "working") (setq pos (match-beginning 0)) (insert "x"))))
  (:then (with-current-buffer console (get-text-property pos 'read-only)))
  (:then (with-current-buffer console (not (get-text-property (1- (point)) 'read-only)))))

;; Killing the console buffer runs the close hook: the run is aborted, the
;; store is closed, and nothing asks for another request.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((delay 0.4) (text "late") (stop "end"))))
           (console (pearl-console-open root "kill"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "go") (console-tests--wait-for console "go") (kill-buffer console) (sleep-for 0.3)))
  (:then (not (buffer-live-p console)))
  (:then (equal 1 (length (pearl-provider-fake-requests pearl-console-provider)))))

;; M7's other half: a provably in-root write leaves the status line naming
;; the project regime, so the user knows which one is active.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "shell" (list (cons 'command "echo hi > out.txt") (cons 'reason "write inside"))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-console-open root "regime-project"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "write it") (console-tests--wait-for console "done")))
  (:then (string-match-p "project" (pearl-console-status console))))

;; M3: a message streams into the buffer as it arrives: the first delta is
;; visible before the second one exists, not rendered once at the end.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-console-provider (pearl-provider-fake '((text "first") (delay 0.4) (text "second") (stop "end"))))
           (console (pearl-console-open root "stream"))))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "go") (console-tests--wait-for console "first")))
  (:then (with-current-buffer console (string-match-p "first" (buffer-string))))
  (:then (with-current-buffer console (not (string-match-p "second" (buffer-string)))))
  (:then (console-tests--wait-for console "second")))

;; An undo is another entry, not a rewrite: a rollback appends an undo
;; entry to the record on disk, and everything before it stays as it was.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "recorded.el" root))
           (pearl-console-provider (pearl-provider-fake (list (list 'tool-call "c1" "edit" (list (cons 'path file) (cons 'edits (list (list (cons 'oldText "old") (cons 'newText "new")))))) '(stop "end") '(text "done") '(stop "end"))))
           (console (pearl-console-open root "recorded"))
           (path (pearl-session-jsonl-path-for root "recorded"))
           (before nil) (after nil))
          (with-temp-file file (insert "old\n")))
  (:when (progn (pearl-console-wire console) (pearl-console-send console "change it") (support-tests-wait (lambda () (equal "new\n" (console-tests--file-text file))) 10) (setq before (pearl-session-jsonl-read (pearl-session-jsonl-open path))) (pearl-console-rollback console) (setq after (pearl-session-jsonl-read (pearl-session-jsonl-open path)))))
  (:then (equal (1+ (length before)) (length after)))
  (:then (equal before (butlast after)))
  (:then (eq 'undo (plist-get (car (last after)) :kind))))

;; The model is a setting a user changes mid-conversation: the choice lives in
;; the buffer, travels in the next request, and does not rewrite the transcript
;; above it.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (pearl-model "from-the-custom")
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (pearl-console-provider provider)
           (console (pearl-console-open root "model"))
           (requests nil)))
  (:when (progn (pearl-console-wire console)
                (pearl-console-set-model console "picked-in-the-buffer")
                (pearl-console-send console "hello")
                (support-tests-wait (lambda () (pearl-provider-fake-requests provider)) 10)
                (setq requests (pearl-provider-fake-requests provider))))
  (:then (equal "picked-in-the-buffer" (plist-get (car (last requests)) :model))))

;; A console with no model asks once and remembers the answer: the prompt is
;; the first screen of a session, not one per turn.
(ert-gwt-deftest
  (:given ((pearl-model nil)
           (pearl-models '("a-model" "another-model"))
           (asked 0)
           (first nil) (second nil)
           (console (pearl-console-open (support-tests-temp-root) "model-ask")))
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
           (console (pearl-console-open (support-tests-temp-root) "key")))
          (cl-letf (((symbol-function 'pearl-provider-sse-api-key)
                     (lambda (_host) (setq resolved (1+ resolved)) "sk-test")))))
  (:when (progn (setq first (pearl-console-api-key console))
                (setq second (pearl-console-api-key console))))
  (:then (equal "sk-test" first))
  (:then (equal "sk-test" second))
  (:then (= 1 resolved))
  (:then (with-current-buffer console (not (string-match-p "sk-test" (buffer-string))))))

(provide 'console-tests)

;;; console-tests.el ends here
