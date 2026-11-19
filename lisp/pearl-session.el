;;; pearl-session.el --- entries, branches and projection  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The record of what happened, and the only module that decides what the model
;; reads next.  Entries are immutable and only appended; an entry carries the
;; id of its parent, which makes the session a tree and not a log.  The path
;; from the root to the head is the active branch, and it is what the
;; projection turns into context.
;;
;; This module is in the core, next to the loop, because PEARL's promise is
;; that an applied change can be taken back, and a guarantee that depends on
;; the layer above remembering to record something is a convention rather than
;; a guarantee.  ADR 3 records the argument and its price.  Storing entries is
;; an adapter's job, and this module never learns a file name.
;;
;; Two things that used to live here live above it now, each for the reason it
;; could not be tested here: `pearl-prompt-text' renders text from sources
;; without knowing what a session is, and `pearl-compaction' writes the entry
;; that changes what the projection reads.  The projection itself stays,
;; because it is the one function that decides what the model sees next and the
;; one place the order of that is written down.
;;
;; Planned API
;;
;; `pearl-session-new' (&optional ROOT-ENTRY) -> session
;;      A session with no head yet, or one whose head is ROOT-ENTRY.
;;
;; `pearl-session-restore' (ENTRIES &optional HEAD) -> session
;;      The inverse of `pearl-session-jsonl-read': a session whose record is
;;      ENTRIES, in the order they were appended, and whose head is HEAD or,
;;      when that is nil, the last entry in ENTRIES.  This is what resuming is
;;      (ADR 29) - the store reads entries and this turns them back into the
;;      thing a console can continue, so the core still learns no file name.
;;      An entry whose parent is not in ENTRIES is not silently reattached: the
;;      record it came from is not the record it claims, and a session that
;;      guesses its own history cannot be replayed.  Entries are taken whole,
;;      so a restored session evaluates to the same projection as the one that
;;      wrote them.
;;
;; `pearl-session-append' (SESSION ENTRY) -> entry
;;      Set the entry's parent to the current head and make it the head.  The
;;      returned entry is the stored one, so a caller never holds a copy.
;;
;; `pearl-session-entries' (SESSION) -> list of entries in append order
;; `pearl-session-head' (SESSION) -> entry or nil
;; `pearl-session-parent' (SESSION ENTRY) -> entry or nil
;; `pearl-session-set-head' (SESSION ENTRY) -> nil
;;      Move to an existing entry.  Branches stay in the session and leave the
;;      active branch; nothing is deleted.  ENTRY may be nil, meaning before the
;;      root: re-editing the first message is a sibling of it and not a
;;      replacement for it, and the append that follows writes the second
;;      attempt while the first stays readable (ADR 29).
;;
;; `pearl-session-branch' (SESSION) -> list of entries, root first
;;      The path the head stands on, and the only thing the projection reads.
;;      The other branches stay in the session, unread until the head moves
;;      back onto one: rewinding is moving the head, and it is how a second
;;      attempt at the same point is written without erasing the first.
;;
;; `pearl-session-projection' (SESSION TOOLS) -> list of messages The
;;      pure function that owns what the model sees: walk the active branch,
;;      prepend the prompt over the declarations TOOLS carry, and fold tool
;;      output a later turn has already read: a tool entry with a message after
;;      it on the branch projects as a short marker, never the full text, while
;;      the newest tool output stays whole.  The entry itself keeps everything.
;;      It cannot add an entry, and compaction is a change to this function
;;      rather than to the tree: the entries a `compaction' entry covers do not
;;      appear, and its summary appears once (ADR 27).  A compaction whose own
;;      entry a later range covers is not rendered at all: the later summary
;;      stands for the work, one summary per request, and the older entry is
;;      still on the branch for whoever expands the range.
;;
;;      Where it appears is the earliest position the covered entries occupy,
;;      not the compaction entry's own, because the bytes a request is billed
;;      for are the ones before the fold point and the fold should move that
;;      point as far back as it honestly can.  Two places are refused: before
;;      the branch's first user entry, which providers reject when it goes
;;      missing, and between a tool call and the result that answers it, which
;;      no provider accepts on its own.  A rule about position is a rule about
;;      bytes, and it belongs in the function that emits them.
;;
;;      It takes TOOLS and not a request.  A request is what one provider is
;;      asked; the projection is what the branch says, and a function that read
;;      a request would make the record depend on the shape an adapter converts
;;      (ADR 7).  The order of the context is decided here and nowhere else:
;;      the prompt first, then the branch in append order, with the tool list
;;      as it stood at the entry that declared it.
;;
;;      The projection is latched in the sense the provider's cache needs: what
;;      it produced last time is kept and replayed, so a request that changes
;;      nothing above the fold point is billed as the same prefix.  The latch
;;      is refreshed by the events that must change it anyway - an append, a
;;      fold, a tool loadout that differs, a head that moved - and not by the
;;      passing of time or by a new estimate, which is the difference between a
;;      rule and a tuning knob.  Recomputing the head every turn would be
;;      correct and would cost full price on every request, which is why this
;;      is stated here rather than left to an implementer's instinct.
;;
;; `pearl-session-prompt-sources' (SESSION) -> list of sources
;;      What the product read and this session stores, as a list of
;;      `(:name :path :text)'.  The session holds them and never loads them,
;;      which is why a core story can test a prompt without a temporary
;;      directory, and why `pearl-prompt-text' takes the list rather than a
;;      session.
;;
;; `pearl-session-set-prompt-sources' (SESSION SOURCES) -> SESSION
;;      Replace the stored sources.  The product calls this when the prompt's
;;      inputs changed, before the next turn; there is no other way to add text
;;      to a prompt (ADR 21).
;;
;; `pearl-session-patch' (ENTRY) -> patch or nil
;;      Only an entry that changed something outside the session carries one.
;;      A patch names the tool, the file, the text replaced and the text
;;      written, plus a hash where one side has no text (create, delete,
;;      rename).  The core records patches and never interprets them.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'pearl-contract)
(require 'pearl-prompt)

(define-error 'pearl-session-invalid "A session record does not hold together")

(cl-defstruct (pearl-session--instance (:constructor pearl-session--create)
                                        (:conc-name pearl-session-)
                                        (:predicate pearl-session-p)
                                        (:copier copy-pearl-session))
  record head sources latch id)

(defun pearl-session-new (&optional root-entry id)
  "A session with no head yet, or one whose head is ROOT-ENTRY.
ID names the session in a record that holds one - a file name, or a
delegate a parent's entry points at."
  (pearl-session--create :record (if root-entry (list root-entry))
                         :head root-entry
                         :id id))

(defun pearl-session-restore (entries &optional head id)
  "A session whose record is ENTRIES, whose head is HEAD and whose id is ID."
  (let ((ids (mapcar (lambda (entry) (plist-get entry :id)) entries)))
    (dolist (entry entries)
      (let ((parent (plist-get entry :parent)))
        (unless (or (null parent) (member parent ids))
          (signal 'pearl-session-invalid
                  (list :parent parent :entry (plist-get entry :id))))))
    (let ((head (or head (car (last entries)))))
      (unless (or (null head) (member (plist-get head :id) ids))
        (signal 'pearl-session-invalid (list :head (plist-get head :id))))
      (pearl-session--create :record (reverse entries) :head head :id id))))

(defun pearl-session-append (session entry)
  "Set ENTRY's parent to SESSION's head and make it the head."
  (let ((head (pearl-session-head session)))
    (plist-put entry :parent (and head (plist-get head :id)))
    (push entry (pearl-session-record session))
    (setf (pearl-session-head session) entry)
    (setf (pearl-session-latch session) nil)
    entry))

(defun pearl-session-entries (session)
  "Return SESSION's entries, in the order they were appended."
  (reverse (pearl-session-record session)))

(defun pearl-session-parent (session entry)
  "Return the entry ENTRY continues in SESSION, or nil at the root."
  (let ((parent (plist-get entry :parent)))
    (seq-find (lambda (candidate)
                (and parent (equal parent (plist-get candidate :id))))
              (pearl-session-entries session))))

(defun pearl-session-set-head (session entry)
  "Move SESSION's head to ENTRY, or to nothing when ENTRY is nil."
  (setf (pearl-session-head session) entry)
  (setf (pearl-session-latch session) nil)
  nil)

(defun pearl-session-branch (session)
  "Return the path SESSION's head stands on, root first."
  (let ((branch nil)
        (entry (pearl-session-head session)))
    (while entry
      (push entry branch)
      (setq entry (pearl-session-parent session entry)))
    branch))

(defun pearl-session-patch (entry)
  "Return the patch ENTRY carries, or nil."
  (plist-get entry :patch))

(defun pearl-session-prompt-sources (session)
  "Return the prompt sources stored in SESSION."
  (pearl-session-sources session))

(defun pearl-session-set-prompt-sources (session sources)
  "Replace SESSION's stored SOURCES."
  (setf (pearl-session-sources session) sources)
  (setf (pearl-session-latch session) nil)
  session)

(defun pearl-session--message (entry)
  "The message ENTRY projects as, or nil when it projects as none."
  (let ((data (plist-get entry :data)))
    (pcase (plist-get entry :kind)
      ('message
       (let ((message (pearl-contract-message
                       (or (plist-get data :role) 'user)
                       (plist-get data :content)
                       (or (plist-get data :timestamp)
                           (plist-get entry :timestamp)))))
         (if-let* ((calls (plist-get data :calls)))
             (append message (list :calls calls))
           message)))
      ('tool
       (list :role 'tool
             :id (plist-get data :id)
             :content (pearl-session--tool-content entry)
             :timestamp (plist-get entry :timestamp))))))

(defun pearl-session--tool-output (entry)
  "Return the whole text a tool ENTRY produced."
  (let ((content (plist-get (plist-get entry :data) :content)))
    (cond ((stringp content) content)
          ((consp content) (pearl-contract-text (list :content content)))
          (t (format "%s" content)))))

(defconst pearl-session--fold-limit 200
  "How much tool output a projection keeps before it folds the rest away.

A short result is cheaper to carry than a marker that names it, and a
model that has to ask for its own last answer wastes a turn; a long one
is what the fold is for (ADR 27).")

(defun pearl-session--folded (entry read-later)
  "Return the marker that stands for a tool ENTRY when READ-LATER."
  (let* ((data (plist-get entry :data))
         (text (pearl-session--tool-output entry)))
    (if (or (not read-later)
            (<= (length text) pearl-session--fold-limit))
        text
      (format "[%s %s folded: %d characters]"
              (or (plist-get data :name) "tool")
              (or (plist-get data :id) "?")
              (length text)))))

(defun pearl-session--tool-content (entry)
  "Return the text a tool ENTRY projects as."
  (pearl-session--folded entry nil))

(defun pearl-session--project (session tools)
  "Return the messages for SESSION, given TOOLS."
  (let* ((branch (pearl-session-branch session))
         (places (make-hash-table :test #'equal))
         (covered (make-hash-table :test #'equal))
         (summaries (make-hash-table :test #'equal))
         (index 0)
         (messages nil))
    (dolist (entry branch)
      (puthash (plist-get entry :id) index places)
      (setq index (1+ index)))
    (dolist (entry branch)
      (when (eq 'compaction (plist-get entry :kind))
        (dolist (id (plist-get (plist-get entry :data) :covers))
          (puthash id t covered))))
    ;; A compaction an earlier one's range reaches is one summary too many:
    ;; its entry is covered, so whatever it said has already been said again
    ;; by the newer summary that folded it.  The newer entry stands where the
    ;; range it covers used to, and the older one stays on the branch for a
    ;; reader to expand, which is what makes compacting twice cost one model
    ;; answer per compaction and still one summary in the request.
    (dolist (entry branch)
      (when (and (eq 'compaction (plist-get entry :kind))
                 (not (gethash (plist-get entry :id) covered)))
        (let* ((data (plist-get entry :data))
               (covers (plist-get data :covers))
               (found (delq nil (mapcar (lambda (id) (gethash id places)) covers)))
               (place (and found (pearl-session--summary-place branch
                                                               (apply #'min found)))))
          (when place
            (puthash place (or (gethash place summaries)
                               (plist-get data :summary))
                     summaries)))))
    ;; The prompt and a compaction's summary carry no clock: they are text a
    ;; projection puts in front of the record, not things that happened, and a
    ;; timestamp here would make two projections of one branch differ.
    (push (list :role 'system
                :content (pearl-prompt-text
                          (pearl-session-prompt-sources session) tools))
          messages)
    (let ((position 0))
      (dolist (entry branch)
        (let ((summary (gethash position summaries)))
          (when summary
            (setq messages (cons (list :role 'system :content summary)
                                 messages))))
        (unless (gethash (plist-get entry :id) covered)
          (let ((message (pearl-session--message entry)))
            (when message
              (when (eq 'tool (plist-get entry :kind))
                (setf (plist-get message :content)
                      (pearl-session--folded
                       entry
                       (seq-some (lambda (later)
                                   (eq 'message (plist-get later :kind)))
                                 (nthcdr (1+ position) branch)))))
              (setq messages (cons message messages)))))
        (setq position (1+ position))))
    (nreverse messages)))

(defun pearl-session--summary-place (branch place)
  "Return where a summary may stand in BRANCH, given the PLACE it was cut at."
  (let ((first-user (seq-position branch t
                                  (lambda (entry _)
                                    (and (eq 'message (plist-get entry :kind))
                                         (not (eq 'assistant
                                                  (plist-get (plist-get entry :data)
                                                             :role))))))))
    (max place (or first-user 0))))

(defun pearl-session-projection (session tools)
  "Return the projection of SESSION, given TOOLS."
  (let ((key (list (plist-get (pearl-session-head session) :id)
                   tools
                   (length (pearl-session-record session))
                   (pearl-session-prompt-sources session)))
        (latch (pearl-session-latch session)))
    (if (equal key (plist-get latch :key))
        (plist-get latch :value)
      (let ((value (pearl-session--project session tools)))
        (setf (pearl-session-latch session) (list :key key :value value))
        value))))

(provide 'pearl-session)

;;; pearl-session.el ends here
