;;; pearl-app.el --- the one place that knows every piece  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A console is a buffer.  It draws, it takes text, and it must not know how
;; a session store is opened, which provider is in use or where the user's
;; instructions file lives.  This module knows all of that, once, so the
;; console can be replaced without touching any of it and every piece can be
;; handed to it already built.
;;
;; That is also why the wiring is here rather than in the console: product may
;; require core and adapters and the reverse is what the layer rule forbids, so
;; the module that assembles pieces from all three can only live up here.  A
;; console that assembled them would be the console depending on everything,
;; which is what `pearl-console-close' being unable to name a store already
;; showed.
;;
;; It reads files and it starts nothing.  The prompt's two files are read once,
;; here, and handed to the session as sources; the session composes the prompt
;; from them, so two consoles cannot rewrite each other's (ADR 20).  No
;; request is made from this module: a model call belongs to the loop.
;;
;; Planned API
;;
;; `pearl-app-open' (&optional ROOT SUFFIX) -> app
;;      Create the console, the session and the loop for ROOT, and hand back
;;      the object that holds them.  SUFFIX names the session so two consoles
;;      on one root do not collide.  Nothing is opened yet: `pearl-app-wire'
;;      is where a file is touched, so a story can build an app and never
;;      write.
;;
;; `pearl-app-console' (APP) -> console
;;      The buffer.  Everything the user does goes through it, so the app is
;;      reachable from a test without being a second thing the user types at.
;;
;; `pearl-app-wire' (APP) -> APP
;;      Assemble the pieces the app is the only place able to assemble: open
;;      the session store with `:lock t', because a console is a writer and a
;;      second one on the same suffix is a conversation that answers itself
;;      (ADR 22); a store that is already locked becomes a `user-error' naming
;;      the session, saying whether the holder is this Emacs, and suggesting
;;      another suffix.  Then restore the session that reads it back, build the
;;      loop over `pearl-app-provider' and the tool list, install the default
;;      tools and the review wiring, pass the loop a guard closure over
;;      `buffer-live-p' (ADR 15), and attach undo to the turn events.  Every
;;      request the loop makes carries `(pearl-console-model console)', so the
;;      model is a property of the conversation and not of the provider.
;;
;;      It reads the prompt's files once with `pearl-app-prompt-sources' and
;;      hands them to the session through `pearl-session-set-prompt-sources'.
;;      It does not compose the prompt itself: the session does, from the
;;      sources it was given (ADR 20).
;;
;; `pearl-app-provider' -> defcustom, a provider or nil
;;      The provider an app wires its loop over.  Nil builds the SSE provider
;;      from `pearl-provider-sse-base-url', the console's model and the key it
;;      resolved; a test binds a fake here, which is the second use of the
;;      interface ADR 7 asks for.
;;
;; `pearl-app-user-instructions-file' -> defcustom, a file or nil
;;      The user's own rules, read for every console whatever the project is:
;;      the reply language, what to do before a commit, what never to touch.
;;      Defaults to `(locate-user-emacs-file "pearl/AGENTS.md")', and nil
;;      disables the layer, so someone whose rules are managed centrally can
;;      drop it without losing the project's (ADR 20).
;;
;; `pearl-app-prompt-file-limit' -> defconst, in bytes
;;      How much of one instructions file reaches the prompt: 32 KiB, which is
;;      roughly what a long one costs in tokens.  A constant rather than a
;;      defcustom, because raising it is a different decision and not a
;;      setting: the prompt is paid for on every request, and the summary a
;;      compaction records is what pays it down.
;;
;; `pearl-app-prompt-sources' (ROOT) -> list of sections
;;      Read the two files the app owns, in the order they belong in the
;;      prompt: `pearl-app-user-instructions-file' first, as `user', then
;;      `AGENTS.md' at ROOT as `project'.  A missing, empty or unreadable file
;;      is skipped rather than injected blank, and nothing above ROOT is read,
;;      because a parent's instructions are not this project's.  A file over
;;      `pearl-app-prompt-file-limit' is cut there and its text ends with a
;;      one-line marker saying it was cut, because a prompt that silently grew
;;      past the window fails later as a request error with no explanation.
;;      Called once when the app is wired, so one console has one prompt
;;      (ADR 20).

;;; Code:

;;; Code:

(require 'cl-lib)

(require 'pearl-compaction)
(require 'pearl-console)
(require 'pearl-delegate)
(require 'pearl-contract)
(require 'pearl-loop)
(require 'pearl-prompt)
(require 'pearl-review)
(require 'pearl-provider)
(require 'pearl-provider-sse)
(require 'pearl-session)
(require 'pearl-session-jsonl)
(require 'pearl-tool)
(require 'pearl-tools-edit)
(require 'pearl-tools-read)
(require 'pearl-tools-shell)
(require 'pearl-undo)

(defgroup pearl nil
  "An agent loop that lives in Emacs."
  :group 'tools
  :prefix "pearl-")

(defcustom pearl-app-provider nil
  "The provider a console sends through, or nil to build the SSE one.
A story binds a fake here; a user sets `pearl-provider-sse-base-url',
sets `PEARL_API_KEY' and leaves this alone."
  :type '(choice (const :tag "build the SSE provider" nil) sexp)
  :group 'pearl)

(defcustom pearl-app-user-instructions-file
  (locate-user-emacs-file "pearl/AGENTS.md")
  "The file whose text reaches every prompt as the user section.
Nil disables the section, which is how a user says the project file is
the only instructions."
  :type '(choice (const :tag "none" nil) file)
  :group 'pearl)

(defcustom pearl-app-context-window nil
  "The window the projection is kept inside, or nil to never compact by size.

A window is a number of tokens the provider will take, so the threshold is
that number minus the room a reply needs (ADR 27).  Nil means the console
compacts when the user asks and not before: a model call nobody asked for
is a model call nobody paid for."
  :type '(choice (const :tag "Never by size" nil) integer)
  :group 'pearl)

(defcustom pearl-app-keep-tokens 20000
  "How much of the tail a compaction leaves verbatim.

The boundary is a token count rather than a number of turns because one
tool result can outgrow a hundred messages (ADR 27)."
  :type 'integer
  :group 'pearl)

(defconst pearl-app-prompt-file-limit 32768
  "How much of one instruction file reaches a prompt, in bytes.")

(cl-defstruct (pearl-app--instance (:constructor pearl-app--create)
                                    (:conc-name pearl-app-)
                                    (:predicate pearl-app-p)
                                    (:copier copy-pearl-app))
  "One console, its record, its session and its loop."
  root suffix console store session loop wired asked (written 0) compacted
  retried)

(defun pearl-app-open (&optional root suffix)
  "Open a console under ROOT with SUFFIX, and read nothing yet.
The console exists before the record does, because a user can look at a
console that failed to wire and the buffer is the only place to say so."
  (let* ((root (file-name-as-directory (expand-file-name (or root default-directory))))
         (suffix (or suffix (format-time-string "%Y%m%d-%H%M%S")))
         (app (pearl-app--create :root root :suffix suffix)))
    (setf (pearl-app-console app) (pearl-console--open app root suffix))
    app))

(defun pearl-app--default-tools ()
  "Return the tools this package ships, installed into an empty list."
  (let ((tools nil))
    (setq tools (pearl-tools-read-install tools))
    (setq tools (pearl-tools-edit-install tools))
    (setq tools (pearl-tools-shell-install tools))
    (setq tools (pearl-delegate-install tools))
    tools))

(defun pearl-app--tools (app)
  "Return the tool list APP wires, or nil when the user took them away."
  (let ((registry pearl-tool-registry))
    (cond
     ((eq registry 'default) (pearl-app--default-tools))
     ((null registry)
      (unless (pearl-app-asked app)
        (setf (pearl-app-asked app) t)
        (y-or-n-p "No tools are registered.  Continue without them? "))
      nil)
     (t registry))))

(defun pearl-app--provider (console)
  "Return the provider CONSOLE sends through.
A configured `pearl-app-provider' is used as it stands; otherwise an
SSE client is built the first time something is asked of it, because a
key is resolved at use (ADR 17) and wiring a console must not ask for
one.  Each request carries the model the buffer currently shows."
  (if pearl-app-provider
      pearl-app-provider
    (let (client)
      (list :pearl-provider t
            :kind 'sse
            :request
            (lambda (_provider request emit keys)
              (setq client (or client
                               (pearl-provider-sse-client
                                :api-key (pearl-console-api-key console))))
              (funcall (plist-get client :request)
                       client
                       (plist-put (copy-sequence request) :model
                                  (pearl-console-model console t))
                       emit keys))
            :cancel
            (lambda (_provider handle)
              (when client
                (funcall (plist-get client :cancel) handle)))))))

(defun pearl-app--failure (failure)
  "Return FAILURE as the sentence a user is shown.

A provider reports a refused request as a plist whose `:reason' is
`http-error' with the status and the body it decoded; anywhere else
FAILURE is a condition whose car names what went wrong.  The plist is not
a condition, so `error-message-string' prints it as a peculiar error with
the number buried inside, and the number is what a wrong key has to say."
  (let ((kind (and (consp failure)
                   (or (plist-get failure :reason) (car failure)))))
    (cond
     ((eq kind 'pearl-loop-dead-owner)
      "the console was killed, so the run stopped")
     ((eq kind 'http-error)
      (let* ((body (plist-get failure :body))
             (message (and (consp body) (plist-get body :message))))
        (format "the endpoint answered %s%s"
                (or (plist-get failure :status) "an error")
                (if (and message (stringp message))
                    (format ": %s" message)
                  ""))))
     ((eq kind 'overflow)
      "the prompt was longer than the context window allows")
     ((eq kind 'idle-timeout)
      "no byte arrived before the endpoint's idle timeout")
     ((consp failure) (error-message-string failure))
     (t (format "%s" failure)))))

(defun pearl-app--asked-id (session)
  "Return the id of the last message the user wrote on SESSION, or nil."
  (let (found)
    (dolist (entry (pearl-session-branch session) found)
      (when (and (eq (plist-get entry :kind) 'message)
                 (eq (plist-get (plist-get entry :data) :role) 'user))
        (setq found (plist-get entry :id))))))

(defun pearl-app--on-error (console condition)
  "Say why the run stopped, for CONDITION, on CONSOLE's status line.

A failed run never emits `turn-end', so the console would otherwise go on
saying it is waiting for a token that is not coming.

An overflow is the one failure worth a model call: ADR 27 makes the
projection something PEARL can shrink, so the run summarises the record
and asks the same question again, once per prompt.  A second overflow is
reported, because a projection that is still too long after a summary is
not something another summary can fix."
  (let* ((reason (pearl-app--failure condition))
         (app (and (buffer-live-p console)
                   (buffer-local-value 'pearl-console--app console)))
         (asked (and app
                     (consp condition)
                     (eq (plist-get condition :reason) 'overflow)
                     (pearl-app--asked-id (pearl-app-session app)))))
    (cond
     ((not (buffer-live-p console))
      (message "Pearl: %s" reason))
     ((and asked (not (equal asked (pearl-app-retried app))))
      (setf (pearl-app-retried app) asked)
      (pearl-console-status console "the run overflowed; summarising the record")
      (condition-case error
          (pearl-app-compact
           app
           (lambda (entry)
             (if entry
                 (condition-case failure
                     (pearl-loop-retry (pearl-console-loop console))
                   (error (pearl-console-fail console
                                              (pearl-app--failure failure))))
               (pearl-console-fail console reason))))
        (error (pearl-console-fail console (pearl-app--failure error)))))
     (t (pearl-console-fail console reason)))))

(defun pearl-app-wire (console-or-app)
  "Open the record of CONSOLE-OR-APP, restore the session and build the loop.
A console is accepted as well as an app because the command a user runs
stands in front of a buffer, not in front of a struct."
  (let ((app (if (pearl-app-p console-or-app)
                 console-or-app
               (buffer-local-value 'pearl-console--app console-or-app))))
    (pearl-app--wire app)))

(defun pearl-app--wire (app)
  "Wire APP: open its record, restore its session and build its loop."
  (unless (pearl-app-wired app)
    (let* ((console (pearl-app-console app))
           (root (pearl-app-root app))
           (suffix (pearl-app-suffix app))
           (path (pearl-session-jsonl-path-for root suffix))
           (store (condition-case nil
                      (pearl-session-jsonl-open path :create t :lock t)
                    (pearl-session-locked
                     (user-error "Cannot wire %s: a session of the same-suffix %s is already open here"
                                 suffix suffix)))))
      (let* ((entries (pearl-session-jsonl-read store))
             (session (if entries
                          (pearl-session-restore entries)
                        (pearl-session-new)))
             (tools (pearl-app--tools app)))
        (pearl-session-set-prompt-sources session (pearl-app-prompt-sources root))
        (pearl-console-show-record console entries)
        (with-current-buffer console
          (setq pearl-console--store store
                pearl-console--session session
                pearl-console--loop
                (pearl-loop-new
                 :provider (pearl-app--provider console)
                 :tools tools
                 :session session
                 :emit (lambda (event) (pearl-console-insert-event console event))
                 ;; A configured provider answers with whatever model it
                 ;; was configured with; only the adapter this package
                 ;; builds needs the console to name one.
                 :root root
                 :model (lambda () (pearl-console-model console))
                 :guard (lambda () (buffer-live-p console))
                 :on-error (lambda (condition)
                             (pearl-app--on-error console condition)))))
        (when (pearl-console-loop console)
          (pearl-undo-attach (pearl-console-loop console) root suffix)
          ;; The record is written when a turn ends, not when an entry is
          ;; made: the file is the transcript a reader sees, and half a
          ;; turn in it would be a transcript of something that did not
          ;; happen (ADR 22).
          (pearl-event-subscribe
           (pearl-loop-emitter (pearl-console-loop console))
           (lambda (event)
             (when (memq (plist-get event :kind)
                         '(turn-end agent-end tool-execution-end))
               (pearl-app--record app))
             (when (eq (plist-get event :kind) 'turn-end)
               (pearl-app--maybe-compact app)))))
        (setf (pearl-app-written app) (length entries)
              (pearl-app-store app) store
              (pearl-app-session app) session
              (pearl-app-loop app) (pearl-console-loop console)
              (pearl-app-wired app) t)
        ;; The build a tool prints is walkable from the console: the
        ;; review module listens to the loop and owns the findings, so a
        ;; console wired here walks places without being asked.
        (pearl-review-install console)
        app))))

(defun pearl-app--transcript (session)
  "Return the recorded parts of SESSION, as text for a summariser.
The projection is not used: it carries the prompt and the folded tool
output, and a summary of a prompt is a summary of this package."
  (let ((lines nil))
    (dolist (entry (pearl-session-branch session))
      (let ((data (plist-get entry :data))
            (kind (plist-get entry :kind)))
        (cond
         ((and (eq kind 'message) (plist-get data :content))
          (push (format "%s: %s" (plist-get data :role) (plist-get data :content))
                lines))
         ((eq kind 'tool)
          (push (format "tool %s: %s" (plist-get data :name)
                        (pearl-tool-truncate
                         (or (plist-get data :content) "")
                         :limit 400))
                lines)))))
    (mapconcat #'identity (nreverse lines) "\n")))

(defun pearl-app-compact (app &optional then)
  "Summarise what APP has recorded, and append the summary as an entry.

The model call is made here, not in the core: a summary is somebody's
answer, and the core reaches no network (ADR 27).  What is appended is a
`compaction' entry naming the entries it covers, so nothing is deleted
and `pearl-session-expand' still reads the branch.

The call is asynchronous like every model call, so what the summary
produced goes to THEN - the entry, or nil when the call failed - instead
of being returned."
  (let* ((session (pearl-app-session app))
         (console (pearl-app-console app))
         (provider (pearl-app--provider console))
         (transcript (pearl-app--transcript session))
         (summary ""))
    (when (string-empty-p transcript)
      (user-error "Nothing has been said yet, so there is nothing to summarise"))
    (pearl-provider-request
     provider
     (pearl-contract-request
      (list (pearl-contract-message
             'user
             (concat "Summarise the work so far for a model that will continue it.\n"
                     "Say what was asked, what was done, what is unresolved, and what "
                     "the next step is. Write prose, not a list.\n\n"
                     transcript)))
      :model (pearl-console-model console))
     (lambda (event)
       (when (eq 'text (plist-get event :type))
         (setq summary (concat summary (or (plist-get event :text) "")))))
     :on-done (lambda ()
                (let ((entry (pearl-compaction-append session summary
                                                      :keep pearl-app-keep-tokens)))
                  (pearl-app--record app)
                  (pearl-console-status console
                                        (if entry "compacted" "nothing to compact"))
                  (when then (funcall then entry))))
     :on-error (lambda (condition)
                 (pearl-console-status
                  console
                  (format "the summary failed: %s"
                          (pearl-app--failure condition)))
                 (when then (funcall then nil))))
    nil))

(defun pearl-app--maybe-compact (app)
  "Compact APP when the window it was given is spent.

A threshold nobody set spends no model call, so `pearl-app-context-window'
is nil by default.  The last compaction's entry is remembered because the
projection it leaves is still over the window: without it every turn would
summarise the same work again."
  (let* ((session (pearl-app-session app))
         (entries (pearl-session-entries session))
         (last (car (last entries))))
    (when (and pearl-app-context-window
               entries
               (not (eq (plist-get last :kind) 'compaction))
               (not (equal (plist-get last :id) (pearl-app-compacted app)))
               (pearl-compaction-needed session pearl-app-context-window))
      (setf (pearl-app-compacted app) (plist-get last :id))
      (pearl-app-compact app))))

(defun pearl-app--record (app)
  "Append the entries APP has not written yet to its store."
  (let ((store (pearl-app-store app))
        (session (pearl-app-session app)))
    (when (and store session)
      (let ((entries (pearl-session-entries session)))
        (while (< (pearl-app-written app) (length entries))
          (pearl-session-jsonl-append store (nth (pearl-app-written app) entries))
          (cl-incf (pearl-app-written app)))))))

(defun pearl-app--read-file (path)
  "Return the text of PATH, or nil when the file is silent.
A file over `pearl-app-prompt-file-limit' is cut and marked as cut,
because a prompt that silently loses a paragraph is worse than one that
admits it."
  (when (and path (file-regular-p path) (file-readable-p path))
    (let ((text (with-temp-buffer
                  (insert-file-contents path nil 0 pearl-app-prompt-file-limit)
                  (buffer-string))))
      (cond
       ((string-empty-p (string-trim text)) nil)
       ((< (nth 7 (file-attributes path)) pearl-app-prompt-file-limit) text)
       (t (concat text (format "\n[cut at %d characters]\n"
                              pearl-app-prompt-file-limit)))))))

(defun pearl-app-prompt-sources (root)
  "Return the prompt sections ROOT implies.
The user file comes first, the project file second, and each is left out
when it says nothing (ADR 21)."
  (let ((sources nil))
    (when-let* ((user (pearl-app--read-file pearl-app-user-instructions-file)))
      (push (list :name "user" :path pearl-app-user-instructions-file :text user)
            sources))
    (let* ((project (expand-file-name "AGENTS.md" (file-name-as-directory root)))
           (text (pearl-app--read-file project)))
      (when text
        (push (list :name "project" :path project :text text) sources)))
    (nreverse sources)))

(provide 'pearl-app)

;;; pearl-app.el ends here
