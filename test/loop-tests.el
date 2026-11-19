;;; loop-tests.el --- stories about the double loop  -*- lexical-binding: t; -*-

;;; Commentary:

;; The two channels are the whole control flow: steering arrives mid-run
;; and never ends it, follow-up arrives only when the loop would stop.

;;; Code:

(require 'support-tests)
(require 'pearl-loop)
(require 'pearl-contract)
(require 'pearl-session)
(require 'pearl-tool)
(require 'pearl-provider)
(require 'pearl-prompt)
(require 'pearl-tools-shell)
(require 'cl-lib)
(require 'ert-gwt)

(defun loop-tests--branch-texts (session)
  "Return the content of each message entry on SESSION's active branch."
  (mapcar (lambda (entry)
            (plist-get (plist-get entry :data) :content))
          (seq-filter (lambda (entry)
                        (eq 'message (plist-get entry :kind)))
                      (pearl-session-branch session))))

(defun loop-tests--tool-entries (session)
  "Return the tool entries on SESSION's active branch, in branch order."
  (seq-filter (lambda (entry) (eq 'tool (plist-get entry :kind)))
              (pearl-session-branch session)))

(defun loop-tests--request-messages (request)
  "Return REQUEST's messages, in the order the model read them."
  (plist-get request :messages))

(defun loop-tests--conversation (request)
  "Return REQUEST's non-system messages, in wire order."
  (seq-remove (lambda (message) (eq 'system (plist-get message :role)))
              (loop-tests--request-messages request)))

(defun loop-tests--idle (loop)
  "Wait until LOOP's run finishes; return non-nil when it did."
  (support-tests-wait (lambda () (not (pearl-loop-busy-p loop)))))

;; Steering is a queue, not an interrupt: a message handed to the channel
;; waits there until the loop picks it up.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.3) (text "one") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))))
  (:when (pearl-loop-steer loop (pearl-contract-message 'user "stop here")))
  (:then (equal '("stop here")
                (mapcar #'pearl-contract-text (pearl-loop-queued loop 'steering))))
  (:then (null (pearl-loop-busy-p loop)))
  (:cleanup (pearl-loop-abort loop)))

;; A plain turn runs to completion: the user message and the reply are on
;; the record, the queues are empty, and the loop is idle again.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end"))))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools nil :session session :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal '("go" "one") (loop-tests--branch-texts session)))
  (:then (null (pearl-loop-queued loop)))
  (:then (null (pearl-loop-busy-p loop))))

;; Steering joins the inner loop: it lands after the turn's tool results
;; and before the next request, never in the request that is already flying.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "echo" nil) (stop "end") (text "two") (stop "end"))))
           (tool (pearl-tool-define "echo" '(:description "echo") (lambda (_arguments _context) "ok")))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session session :emit #'ignore))))
  (:when (progn (pearl-loop-steer loop (pearl-contract-message 'user "stop here")) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal '("go") (mapcar #'pearl-contract-text (loop-tests--conversation (car (pearl-provider-fake-requests provider))))))
  ;; The empty text is the assistant message that asked for the tool, which
  ;; the next request has to carry before the answer.
  (:then (equal '("go" "" "ok" "stop here") (mapcar #'pearl-contract-text (loop-tests--conversation (cadr (pearl-provider-fake-requests provider)))))))

;; Follow-up re-enters the outer loop: it becomes a new turn after the
;; first turn ended, never a message inside the turn that was running.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end") (text "two") (stop "end"))))
           (session (pearl-session-new))
           (events nil)
           (loop (pearl-loop-new :provider provider :tools nil :session session :emit (lambda (event) (push event events))))))
  (:when (progn (pearl-loop-follow-up loop (pearl-contract-message 'user "next")) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal '("go" "one" "next" "two") (loop-tests--branch-texts session)))
  (:then (equal 2 (length (seq-filter (lambda (event) (eq 'turn-start (plist-get event :kind))) events))))
  (:then (let* ((ordered (nreverse events))
                (kinds (mapcar (lambda (event) (plist-get event :kind)) ordered))
                (turn-end (cl-position 'turn-end kinds))
                (follow-up (cl-position-if
                            (lambda (event)
                              (and (eq 'message-start (plist-get event :kind))
                                   (equal "next" (plist-get (plist-get (plist-get event :data) :message) :content))))
                            ordered)))
           (and turn-end follow-up (< turn-end follow-up)))))

;; Abort stops the run, keeps the tool results already produced, hands the
;; queues back in delivery order, and nothing arrives after it.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "echo" nil) (stop "end") (delay 0.5) (text "late") (stop "end"))))
           (tool (pearl-tool-define "echo" '(:description "echo") (lambda (_arguments _context) "ok")))
           (session (pearl-session-new))
           (events nil)
           (loop (pearl-loop-new :provider provider :tools (list tool) :session session :emit (lambda (event) (push event events))))
           (handed nil) (count-at-abort nil)))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (support-tests-wait (lambda () (loop-tests--tool-entries session)))
                (pearl-loop-steer loop (pearl-contract-message 'user "s"))
                (pearl-loop-follow-up loop (pearl-contract-message 'user "f"))
                (setq handed (pearl-loop-abort loop))
                (setq count-at-abort (length events))
                (sleep-for 0.3)))
  (:then (equal '("s" "f") (mapcar #'pearl-contract-text handed)))
  (:then (null (pearl-loop-busy-p loop)))
  (:then (equal 1 (length (loop-tests--tool-entries session))))
  (:then (not (member "late" (loop-tests--branch-texts session))))
  (:then (and (> count-at-abort 0)
              (equal count-at-abort (length events)))))

;; A length-truncated reply fails every tool call of that turn instead of
;; running them, and the turn ends without another request (ADR 8).
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "count" nil) (tool-call "c2" "count" nil) (stop "length"))))
           (runs 0)
           (tool (pearl-tool-define "count" '(:description "count") (lambda (_arguments _context) (setq runs (1+ runs)) "ran")))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session session :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal 0 runs))
  (:then (equal 2 (length (loop-tests--tool-entries session))))
  (:then (seq-every-p (lambda (entry) (plist-get (plist-get entry :data) :error)) (loop-tests--tool-entries session)))
  (:then (equal 1 (length (pearl-provider-fake-requests provider)))))

;; Calls within one turn run in order and their results keep that order,
;; because a change group and a confirmation may sit between two calls.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "tag" ((tag . "first"))) (tool-call "c2" "tag" ((tag . "second"))) (stop "end") (text "done") (stop "end"))))
           (seen nil)
           (tool (pearl-tool-define "tag" '(:description "tag") (lambda (arguments _context) (push (plist-get arguments :tag) seen) "ok")))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session session :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal '("first" "second") (nreverse seen)))
  (:then (equal '("c1" "c2") (mapcar (lambda (entry) (plist-get (plist-get entry :data) :id)) (loop-tests--tool-entries session)))))

;; A turn that asked for a tool is sent back as the assistant message that
;; asked, followed by the answer carrying the id it answers: an endpoint
;; refuses a `tool' message whose call it never saw, so the call has to
;; survive the record and the projection.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "echo" nil)
                                            (stop "end")
                                            (text "done")
                                            (stop "end"))))
           (tool (pearl-tool-define "echo" '(:description "echo")
                                    (lambda (_arguments _context) "ok")))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools (list tool)
                                 :session session :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (loop-tests--idle loop)))
  (:then (let* ((messages (loop-tests--conversation
                           (cadr (pearl-provider-fake-requests provider))))
                (asked (nth 1 messages))
                (answer (nth 2 messages)))
           (equal "c1" (plist-get (car (plist-get asked :calls)) :id))))
  (:then (let* ((messages (loop-tests--conversation
                           (cadr (pearl-provider-fake-requests provider))))
                (asked (nth 1 messages)))
           (equal "echo" (plist-get (car (plist-get asked :calls)) :name))))
  (:then (let* ((messages (loop-tests--conversation
                           (cadr (pearl-provider-fake-requests provider))))
                (answer (nth 2 messages)))
           (and (eq 'tool (plist-get answer :role))
                (equal "c1" (plist-get answer :id))))))

;; The event order of a text turn is the conversation, pinned to Pi's
;; taxonomy by ADR 8.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end"))))
           (events nil)
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit (lambda (event) (push (plist-get event :kind) events))))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (equal '(agent-start turn-start message-start message-update message-end turn-end agent-end) (nreverse events))))

;; A second submit while a run is in flight is refused by name; the caller
;; queues instead of interrupting.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.5) (text "one") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))
           (refused nil)))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (setq refused (condition-case err (pearl-loop-submit loop (pearl-contract-message 'user "again")) (pearl-loop-busy err)))))
  (:then (eq 'pearl-loop-busy (car refused)))
  (:cleanup (pearl-loop-abort loop)))

;; Submit returns immediately: the run happens on the event loop, not
;; inside the call, so the editor never waits for a model (ADR 10).
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.3) (text "one") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))))
  (:when (pearl-loop-submit loop (pearl-contract-message 'user "go")))
  (:then (pearl-loop-busy-p loop))
  (:cleanup (pearl-loop-abort loop)))

;; The next request carries what the session recorded: the model reads the
;; tool result it has not seen yet, through the projection.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "echo" nil) (stop "end") (text "two") (stop "end"))))
           (tool (pearl-tool-define "echo" '(:description "echo") (lambda (_arguments _context) "ok")))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session (pearl-session-new) :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (member "ok" (mapcar #'pearl-contract-text (loop-tests--request-messages (cadr (pearl-provider-fake-requests provider))))))
  (:then (not (member "ok" (mapcar #'pearl-contract-text (loop-tests--request-messages (car (pearl-provider-fake-requests provider))))))))

;; A change to the tool loadout is declared to the model as a difference on
;; a system message, so replaying the transcript yields the same tool set.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end") (text "two") (stop "end"))))
           (alpha (pearl-tool-define "alpha" '(:description "a") #'ignore))
           (beta (pearl-tool-define "beta" '(:description "b") #'ignore))
           (loop (pearl-loop-new :provider provider :tools (list alpha) :session (pearl-session-new) :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (loop-tests--idle loop)
                (pearl-loop-set-tools loop (list alpha beta))
                (pearl-loop-submit loop (pearl-contract-message 'user "again"))
                (loop-tests--idle loop)))
  (:then (seq-find (lambda (message)
                     (and (eq 'system (plist-get message :role))
                          (string-match-p "beta" (pearl-contract-text message))))
                   (loop-tests--request-messages (cadr (pearl-provider-fake-requests provider)))))
  (:then (not (seq-find (lambda (message)
                          (and (eq 'system (plist-get message :role))
                               (string-match-p "beta" (pearl-contract-text message))))
                        (loop-tests--request-messages (car (pearl-provider-fake-requests provider)))))))

;; A loop hands out the emitter it speaks through, so watching a run is one
;; subscription API and not a second one beside it.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end") (text "two") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))
           (seen nil) (unsubscribe nil)))
  (:when (progn (setq unsubscribe (pearl-event-subscribe (pearl-loop-emitter loop)
                                                      (lambda (event)
                                                        (push (plist-get event :kind) seen))))
                (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (loop-tests--idle loop)
                (funcall unsubscribe)
                (pearl-loop-submit loop (pearl-contract-message 'user "again"))
                (loop-tests--idle loop)))
  (:then (memq 'agent-end seen))
  (:then (equal 1 (length (seq-filter (lambda (kind) (eq 'agent-end kind)) seen)))))

;; ADR 10's third rule, the core half: when the owner dies mid-run the
;; chain stops through the terminal callback, the busy flag is released,
;; and the death is reported by name instead of dropped (ADR 15).
(ert-gwt-deftest
  (:given ((alive t)
           (provider (pearl-provider-fake '((delay 0.3) (text "late") (stop "end"))))
           (session (pearl-session-new))
           (errors nil)
           (events nil)
           (loop (pearl-loop-new :provider provider :tools nil :session session :emit (lambda (event) (push event events)) :guard (lambda () alive) :on-error (lambda (err) (push err errors))))
           (count-at-death nil)))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (setq alive nil) (setq count-at-death (length events)) (support-tests-wait (lambda () (not (pearl-loop-busy-p loop)))) (sleep-for 0.3)))
  (:then (eq 'pearl-loop-dead-owner (car (car errors))))
  (:then (null (pearl-loop-busy-p loop)))
  (:then (equal count-at-death (length events)))
  (:then (not (member "late" (loop-tests--branch-texts session)))))

;; Every request carries a system prompt: the projection prepends it, so
;; the model is told what PEARL is before it is told what the user said.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go")) (loop-tests--idle loop)))
  (:then (eq 'system (plist-get (car (loop-tests--request-messages (car (pearl-provider-fake-requests provider)))) :role)))
  (:then (equal (pearl-prompt-text nil nil)
                (plist-get (car (loop-tests--request-messages
                                 (car (pearl-provider-fake-requests provider))))
                           :content))))

;; A source an extension sets is part of the prompt, and the projection
;; carries it as the system message ahead of the user's words: that is the
;; whole extension surface for prompt text now (ADR 21).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (provider (pearl-provider-fake '((text "one") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil
                                 :session session :emit #'ignore)))
          (pearl-session-set-prompt-sources
           session
           (list (list :name "notes" :path nil :text "an extension's note"))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (loop-tests--idle loop)))
  (:then (string-match-p
          "an extension's note"
          (pearl-contract-text
           (car (loop-tests--request-messages
                 (car (pearl-provider-fake-requests provider))))))))

;; Arguments that do not parse fail their call by name and the turn goes on:
;; a model that emitted half an object can be told so, and a turn that dies in
;; the editor's command loop cannot.  The empty text is the assistant message
;; that made the call, recorded because the next request has to show the
;; endpoint the message the answer belongs to.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((tool-call "c1" "read" "{\"path\":")
                                            (stop "end") (text "two") (stop "end"))))
           (tool (pearl-tool-define "read"
                                    '(:description "read" :arguments ((path :required t)))
                                    (lambda (_arguments _context) "read")))
           (session (pearl-session-new))
           (loop (pearl-loop-new :provider provider :tools (list tool)
                                 :session session :emit #'ignore))))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message 'user "go"))
                (loop-tests--idle loop)))
  (:then (plist-get (plist-get (car (loop-tests--tool-entries session)) :data)
                    :error))
  (:then (equal '("go" "" "two") (loop-tests--branch-texts session))))

;; A tool that waits does not stop the loop: the moment it starts, timers still
;; fire, and the abort command takes the command down instead of waiting for
;; it.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (events nil)
           (started nil)
           (stopped nil)
           (provider (pearl-provider-fake
                      '((tool-call "c1" "shell" ((command . "sleep 5")
                                                 (reason . "wait")))
                        (stop "end"))))
           (loop (pearl-loop-new :provider provider
                                 :tools (pearl-tools-shell-install nil)
                                 :session (pearl-session-new)
                                 :root root
                                 :emit (lambda (event) (push event events)))))
          nil)
  (:when (progn
           (setq started (float-time))
           (pearl-loop-submit loop (pearl-contract-message 'user "go"))
           (run-at-time 0.3 nil (lambda () (pearl-loop-abort loop)))
           (support-tests-wait (lambda () (not (pearl-loop-busy-p loop))) 2)
           (setq stopped (- (float-time) started))
           (support-tests-wait
            (lambda ()
              (seq-some (lambda (event)
                          (eq 'tool-execution-end (plist-get event :kind)))
                        events))
            2)))
  ;; The command sleeps for five seconds.  Stopping the run has to take the
  ;; command with it, or "stop" means "stop listening".
  (:then (< stopped 2))
  (:then (null (seq-filter (lambda (process)
                             (string-prefix-p "pearl-proc-"
                                              (or (process-name process) "")))
                           (process-list))))
  (:then (seq-some (lambda (event)
                     (eq 'tool-execution-end (plist-get event :kind)))
                   events))
  (:cleanup (delete-directory root t)))

(provide 'loop-tests)

;;; loop-tests.el ends here
