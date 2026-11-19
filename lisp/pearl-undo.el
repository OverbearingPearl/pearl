;;; pearl-undo.el --- change groups and the turn snapshot  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The two layers under the promise.  The first is a change group per tool
;; call, amalgamated into a single undo step, which is the everyday path and
;; the reason there is no undo command to learn.  The second is a snapshot of
;; the working tree taken before a turn, which is what makes a change
;; revertible after the buffer is gone or Emacs has restarted.
;;
;; The snapshot is a dangling commit, written through a temporary index so the
;; real one is untouched: it covers edits made outside buffers, respects
;; `.gitignore', and needs no commit message.  A file that no longer matches
;; its baseline is a conflict, and a restore refuses by name rather than
;; guessing, because a wrong restore is worse than a refused one.  A binary
;; change carries no text patch: its durable restore is the snapshot's blob,
;; so once git has collected the dangling commit, a rollback can detect the
;; conflict but not restore the content.
;;
;; This module is the least replicable in the package: every other organ has an
;; upstream shape to copy, and this one is the promise itself.  M0.5 in the
;; roadmap exists to prove it before anything is built on it.
;;
;; Planned API
;;
;; `pearl-undo-group-prepare' (BUFFER) -> handle
;;      Wrap `prepare-change-group' and remember the buffer, so the two ways a
;;      group can end cannot be confused with each other.
;;
;; `pearl-undo-group-accept' (HANDLE) -> nil
;;      Accept the group and amalgamate it into one undo step.  After this,
;;      exactly one `C-/` reverts everything the tool call did.
;;
;; `pearl-undo-group-cancel' (HANDLE) -> nil
;;      Cancel the group, leaving the buffer text and its undo list as they
;;      were before the call.  A failed tool is not an undo step.
;;
;; `pearl-undo-snapshot' (ROOT) -> snapshot or nil
;;      A tree snapshot of ROOT taken before a turn, as a dangling commit
;;      written through a temporary index, or nil outside a git repository.
;;      Nil is a documented state, not a failure: the console says so.
;;
;; `pearl-undo-restore' (SNAPSHOT PATHS) -> list of restored paths
;;      Restore each path from the snapshot after checking it against its
;;      baseline.  A path the snapshot does not contain was created after it
;;      and is deleted; a path the snapshot contains and the worktree does not
;;      is written back from it.  The conflict check covers both directions: a
;;      created file this turn did not write, or a deleted one it did not
;;      delete, is a conflict like any other.  Signals `pearl-undo-conflict'
;;      naming every conflict, and restores nothing when any path conflicts.
;;
;; `pearl-undo-conflict-p' (SNAPSHOT PATH) -> boolean
;;      Non-nil when PATH changed since the snapshot in a way the patch on the
;;      entry cannot explain.
;;
;; `pearl-undo-plan' (SESSION &optional ENTRY) -> plan
;;      What takes the session back to ENTRY, in reverse order, one path per
;;      step: the patch itself where a patch can reverse the change,
;;      `(:restore PATH)' where only the turn's snapshot can, and
;;      `(:preimage PATH)' where the snapshot never covered the path and the
;;      tool call's own pre-image is what takes it back (ADR 23, ADR 25).
;;      Never a single entry out of the middle: a later patch on the same path
;;      was written against this one.  A turn with nothing attributable
;;      contributes no step, because a step that cannot name what it reverted
;;      is a lie the console repeats.
;;
;;      The plan is here and not in the session.  Building it means choosing
;;      between three mechanisms, and a record that chooses is a record that
;;      interprets; the core keeps the patches and this module reads them
;;      (ADR 3, ADR 23).
;;
;; `pearl-undo-timeline' (SESSIONS) -> list of nodes
;;      Every session of a root straightened into one list, oldest first, each
;;      node naming the entry it is and the session it belongs to:
;;      `(:entry ENTRY :session SESSION)'.
;;
;;      This is a projection over the files that already exist and not a
;;      record of its own, so nothing has to be kept in sync with it and a
;;      writer that is still appending contributes a valid prefix ("live"
;;      means "as of the last complete entry").  The order is an interleaving
;;      and not a causal order: entries are sorted by their own timestamp and
;;      ties are broken by session id, because two clocks disagree and a
;;      child's turn can end after a later one began (ADR 30).  A reader that
;;      keeps nothing but the offset it read to takes the bytes after it, which
;;      only works because a session file grows rather than being rewritten.
;;
;; `pearl-undo-plan-at' (SESSIONS NODE) -> plan
;;      What takes the session NODE belongs to back to NODE, with the plan
;;      `pearl-undo-plan' builds for the branch-local case.  Beyond it: a step
;;      whose path a later entry on another branch wrote is refused, because
;;      reverting another session's work is allowed and requires naming it
;;      (ADR 30).  The refusal is a signal of `pearl-undo-conflict' whose data
;;      is `(:path PATH :session SESSION :entry ENTRY)' - the session that
;;      wrote the later change and the entry that carries it, so the console
;;      can say whose change it was and what else that session did after it.
;;
;;      A previewed plan is advisory.  It is built again when it is applied,
;;      and if a background write changed what it would touch the plan is
;;      refused with the sentence that says what changed.  Nothing is applied
;;      quietly and nothing is refused silently.
;;
;;      The undo of another branch's change is appended to that branch, with
;;      `:by' naming the session that asked: the patch belongs to the session
;;      that wrote it, and who asked is metadata rather than ownership.
;;
;; `pearl-undo-patch-conflict-p' (PATCH CONTENT) -> boolean
;;      Non-nil when PATCH cannot be reversed against CONTENT, which is what
;;      makes a refused restore possible.  A patch whose written text is empty
;;      is always a conflict: an empty segment cannot be found uniquely.  This
;;      is the patch's question; `pearl-undo-conflict-p' is the snapshot's.
;;
;; `pearl-undo-preimage-limit' is a defvar, in bytes
;;      How large a single path may be before PEARL stops copying it as a
;;      pre-image: 64 MiB.  A pre-image is paid for on every command that
;;      writes outside the turn's snapshot, and a copy that costs more than the
;;      write is a tax on the common case to cover the rare one (ADR 25).
;;
;; `pearl-undo-preimage' (ROOT PATHS) -> preimage
;;      Clone PATHS into the project's pre-image area before a command writes
;;      them, so a write the turn's snapshot never covered can still be taken
;;      back.  Each path is recorded as copied, absent (nothing there to
;;      restore, so taking it back deletes whatever appeared), over the limit,
;;      or failed - and only the first two can be restored.  Never signals for
;;      a path it could not copy: a confirmed command still runs, and the
;;      result says what the undo can reach instead of aborting the action the
;;      user just approved.
;;
;; `pearl-undo-preimage-restore' (PREIMAGE) -> list of restored paths
;;      Put back what it captured: a copied path from its clone, an absent one
;;      by deleting what is now there.  The paths it could not capture are left
;;      alone and are not in the return value, because a restore that names a
;;      path it did not restore is how a user learns not to trust the report.
;;
;; `pearl-undo-preimage-revertible-p' (PREIMAGE) -> boolean
;;      Non-nil when every path was captured, so the command can be taken back
;;      whole.  One over-limit path makes it nil: half a rollback reported as a
;;      rollback is worse than a write that is honest about being permanent.
;;
;; `pearl-undo-attach' (LOOP ROOT SESSION-ID) -> unsubscribe function
;;      Subscribe to the loop's `turn-start' and `turn-end' events and keep
;;      one snapshot per turn, dropping it when the turn changed nothing.  The
;;      last one is kept under (ROOT SESSION-ID) and not under ROOT, because
;;      two consoles on one root must not offer each other's turns (ADR 23).
;;      The loop may not require this module (ADR 12: product above core), so
;;      the console wires it in when it builds the loop, and the event stream
;;      is the only channel it needs.
;;
;; `pearl-undo-turn-begin' (ROOT) -> turn
;; `pearl-undo-turn-end' (TURN) -> list of paths changed
;;      The pair the event handler calls.  `turn-begin' takes the snapshot;
;;      `turn-end' returns the paths this turn wrote that actually moved, and
;;      records their post-turn state as the baseline a restore verifies
;;      against.  "Wrote" is attribution and not a diff: the paths are the
;;      patches the tools returned plus the `:scope' a shell call proved, and
;;      a tree diff only decides which of those moved.  The two questions
;;      differ the moment a second session writes in the same root, and the
;;      one this pair answers is "which of these did I change" (ADR 23).
;;
;; `pearl-undo-turn-snapshot' (TURN) -> snapshot
;;      The snapshot a turn began with, for `pearl-undo-restore' and
;;      `pearl-undo-conflict-p' to read.
;;
;; `pearl-undo-latest' (ROOT SESSION-ID) -> snapshot or nil
;;      The snapshot of the last turn of that session that changed something,
;;      or nil when its last turn changed nothing.  This is what the console's
;;      rollback offers when the user says "undo that turn": the turn they are
;;      looking at, not the last turn anybody ran under this root (ADR 23).

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'pearl-contract)
(require 'pearl-event)

(define-error 'pearl-undo-conflict "A path changed since the turn that wrote it")
(define-error 'pearl-undo-no-repository "This root is not a git working tree")

;;; Change groups

(cl-defstruct (pearl-undo-group (:constructor pearl-undo--group-create))
  buffer
  handle)

(defun pearl-undo-group-prepare (buffer)
  "Open a change group in BUFFER and return a handle for it."
  (pearl-undo--group-create
   :buffer buffer
   :handle (with-current-buffer buffer (prepare-change-group))))

(defun pearl-undo-group-accept (handle)
  "Accept HANDLE's group as one undo step."
  (let ((buffer (pearl-undo-group-buffer handle)))
    (when (buffer-live-p buffer)
      (with-current-buffer buffer
        (accept-change-group (pearl-undo-group-handle handle))
        (undo-boundary)))))

(defun pearl-undo-group-cancel (handle)
  "Cancel HANDLE's group, putting the buffer back as it was."
  (let ((buffer (pearl-undo-group-buffer handle)))
    (when (buffer-live-p buffer)
      (with-current-buffer buffer
        (let ((inhibit-read-only t))
          (cancel-change-group (pearl-undo-group-handle handle)))))))

;;; Asking git things

(defvar pearl-undo--identity
  '(("GIT_AUTHOR_NAME" . "PEARL")
    ("GIT_AUTHOR_EMAIL" . "pearl@localhost")
    ("GIT_COMMITTER_NAME" . "PEARL")
    ("GIT_COMMITTER_EMAIL" . "pearl@localhost"))
  "How a snapshot commit is signed, for a machine with no git identity.")

(defconst pearl-undo--ambient
  '("GIT_DIR" "GIT_WORK_TREE" "GIT_COMMON_DIR" "GIT_INDEX_FILE"
    "GIT_OBJECT_DIRECTORY" "GIT_ALTERNATE_OBJECT_DIRECTORIES"
    "GIT_NAMESPACE" "GIT_PREFIX" "GIT_CEILING_DIRECTORIES"
    "GIT_DISCOVERY_ACROSS_FILESYSTEM")
  "Variables that point git at a repository other than the one it is handed.

Emacs inherits whatever started it, and a git hook exports `GIT_DIR': a
snapshot taken from such an Emacs would record another worktree's files and
prove another worktree's writes.  A snapshot of ROOT has to be a snapshot of
ROOT, so these are dropped rather than passed on.")

(defun pearl-undo--ambient-p (name)
  "Non-nil when NAME would point git away from the directory it is handed.

`GIT_CONFIG_PARAMETERS' and the `GIT_CONFIG_COUNT' family are git's channels
for command-line `-c' settings, and `-c core.worktree=DIR' redirects a
repository exactly as `GIT_DIR' does.  A wrapper that pushes with such a
setting exports it to everything it starts."
  (or (member name pearl-undo--ambient)
      (string-equal name "GIT_CONFIG_PARAMETERS")
      (string-equal name "GIT_CONFIG_COUNT")
      (string-match-p "\\`GIT_CONFIG_\\(?:KEY\\|VALUE\\)_[0-9]+\\'" name)))

(defvar pearl-undo--index nil
  "The temporary index the current snapshot writes through, or nil.
Bound by `pearl-undo-snapshot' around its own git calls, so the real index is
never touched.")

(defun pearl-undo--environment ()
  "Return the inherited environment with git's redirections removed.
The snapshot's index and pearl's signing identity are added on top."
  (let ((inherited
         (seq-remove (lambda (entry)
                       (pearl-undo--ambient-p (car (split-string entry "="))))
                     process-environment)))
    (append (mapcar (lambda (pair) (concat (car pair) "=" (cdr pair)))
                    pearl-undo--identity)
            (when pearl-undo--index
              (list (concat "GIT_INDEX_FILE=" pearl-undo--index)))
            inherited)))

(defun pearl-undo--git (root &rest arguments)
  "Run git in ROOT with ARGUMENTS, returning `(:status :stdout)'."
  (let ((default-directory (file-name-as-directory root))
        (process-environment (pearl-undo--environment)))
    (with-temp-buffer
      (set-buffer-multibyte nil)
      (let ((status (apply #'call-process "git" nil (current-buffer) nil arguments)))
        (list :status status :stdout (buffer-string))))))

(defun pearl-undo--toplevel (root)
  "Return the git working tree ROOT is inside, or nil."
  (when (and root (file-directory-p root))
    (let ((result (pearl-undo--git root "rev-parse" "--show-toplevel")))
      (when (eq 0 (plist-get result :status))
        (string-trim (plist-get result :stdout))))))

(defun pearl-undo-ignored-p (path root)
  "Non-nil when git treats PATH under ROOT as ignored.
An ignored path is invisible to the snapshot, so writing it is exactly as
unrevertible as writing outside the project.  The index is not consulted:
the question is about what the snapshot would cover, not about what is
staged now.

ROOT is named as the working tree, because a repository can carry a
core.worktree that points somewhere else -- the residue a redirected
`git init' leaves -- and git then answers about that tree and calls every
path here outside it.  The name handed over is relative for the same
reason: an absolute one has to be matched against a working tree git
spells without the symlink the caller went through."
  (let* ((tree (pearl-undo--canonical root))
         (target (file-relative-name (pearl-undo--canonical
                                      (expand-file-name path root))
                                     tree)))
    (eq 0 (plist-get (pearl-undo--git root
                                      (concat "--work-tree=" tree)
                                      "check-ignore" "-q" "--no-index" target)
                     :status))))

(defun pearl-undo--canonical (path)
  "Return PATH with every symlink on it resolved, so two names match.

A temporary directory is named through a symlink on macOS, so the
working tree git reports and the path a story passes are two spellings
of one file."
  (file-truename (expand-file-name path)))

(defun pearl-undo--relpath (snapshot path)
  "Return PATH relative to SNAPSHOT's working tree."
  (file-relative-name (pearl-undo--canonical path) (plist-get snapshot :root)))

(defun pearl-undo--contents-at (snapshot path)
  "Return the content SNAPSHOT kept for PATH, or nil for none."
  (let ((result (pearl-undo--git (plist-get snapshot :root)
                                 "cat-file" "blob"
                                 (format "%s:%s" (plist-get snapshot :commit)
                                         (pearl-undo--relpath snapshot path)))))
    (when (eq 0 (plist-get result :status))
      (plist-get result :stdout))))

(defun pearl-undo--hash (path)
  "Return a digest of PATH's bytes, or nil when PATH is not there."
  (when (file-exists-p path)
    (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert-file-contents-literally path)
      (secure-hash 'sha1 (current-buffer)))))

(defun pearl-undo--hash-string (string)
  "Return a digest of STRING's bytes."
  (with-temp-buffer
    (set-buffer-multibyte nil)
    (insert string)
    (secure-hash 'sha1 (current-buffer))))

(defun pearl-undo--moved-p (snapshot path)
  "Non-nil when PATH no longer matches what SNAPSHOT recorded."
  (let ((now (pearl-undo--hash path))
        (then (pearl-undo--contents-at snapshot path)))
    (not (equal now (and then (pearl-undo--hash-string then))))))

;;; The snapshot

(defvar pearl-undo--snapshots 0
  "How many snapshots this Emacs has taken; it names each commit.")

(defun pearl-undo-snapshot (root)
  "Return a snapshot of ROOT taken now, or nil outside a git working tree."
  (let ((toplevel (pearl-undo--toplevel root)))
    (when toplevel
      (let ((index (make-temp-name
                    (expand-file-name "pearl-index-" temporary-file-directory))))
        (unwind-protect
            (let ((pearl-undo--index index))
              (let ((added (pearl-undo--git toplevel "add" "-A" ".")))
                (when (eq 0 (plist-get added :status))
                  (let ((tree (pearl-undo--git toplevel "write-tree")))
                    (when (eq 0 (plist-get tree :status))
                      (let* ((tree-id (string-trim (plist-get tree :stdout)))
                             (commit (pearl-undo--git
                                      toplevel "commit-tree" tree-id
                                      "-m" (format "pearl snapshot %s-%s"
                                                   (float-time)
                                                   (cl-incf pearl-undo--snapshots)))))
                        (when (eq 0 (plist-get commit :status))
                          (list :root (pearl-undo--canonical toplevel)
                                :commit (string-trim (plist-get commit :stdout))
                                :after nil))))))))
          (condition-case nil (delete-file index) (error nil)))))))

(defun pearl-undo-conflict-p (snapshot path)
  "Non-nil when PATH changed since SNAPSHOT in a way nobody explained."
  (let* ((full (expand-file-name path))
         (canonical (pearl-undo--canonical full))
         (expected (assoc canonical (plist-get snapshot :after)))
         (now (pearl-undo--hash full)))
    (if expected
        (not (equal now (cdr expected)))
      (let ((then (pearl-undo--contents-at snapshot full)))
        (not (equal now (and then (pearl-undo--hash-string then))))))))

(defun pearl-undo-restore (snapshot paths)
  "Put PATHS back as SNAPSHOT has them, returning the paths restored.

Signals `pearl-undo-conflict' naming every conflict, and restores
nothing when any of them conflicts."
  (let* ((full (mapcar #'expand-file-name paths))
         (conflicts (seq-filter (lambda (path) (pearl-undo-conflict-p snapshot path)) full)))
    (when conflicts
      (signal 'pearl-undo-conflict (list (list :paths conflicts))))
    (dolist (path full)
      (let ((content (pearl-undo--contents-at snapshot path)))
        (cond
         (content
          (make-directory (file-name-directory path) t)
          (with-temp-buffer
            (set-buffer-multibyte nil)
            (insert content)
            (write-region (point-min) (point-max) path nil 'silent)))
         ((file-exists-p path)
          (delete-file path)))))
    full))

;;; Pre-images

(defvar pearl-undo-preimage-limit (* 64 1024 1024)
  "How large a single path may be before it is not copied as a pre-image.")

(defun pearl-undo--preimage-dir (root)
  "Return the place ROOT puts pre-images in."
  (let ((toplevel (or (pearl-undo--toplevel root) (expand-file-name root))))
    (expand-file-name (format ".git/pearl-preimages/%s/" (float-time)) toplevel)))

(defun pearl-undo-preimage (root paths)
  "Copy PATHS of ROOT aside before a command writes them."
  (let ((directory (pearl-undo--preimage-dir root))
        (entries nil)
        (number 0))
    (dolist (path paths)
      (setq number (1+ number))
      (let ((full (expand-file-name path)))
        (push
         (cond
          ((not (file-exists-p full))
           (list full 'absent nil))
          ((> (or (file-attribute-size (file-attributes full)) 0)
              pearl-undo-preimage-limit)
           (list full 'over-limit nil))
          (t
           (let ((copy (expand-file-name (format "%d" number) directory)))
             (condition-case nil
                 (progn
                   (make-directory directory t)
                   (copy-file full copy t t)
                   (list full 'copied copy))
               (error (list full 'failed nil))))))
         entries)))
    (list :root (expand-file-name root) :directory directory :entries (nreverse entries))))

(defun pearl-undo-preimage-restore (preimage)
  "Put back what PREIMAGE captured, returning the paths it restored."
  (let ((restored nil))
    (dolist (entry (plist-get preimage :entries))
      (pcase (nth 1 entry)
        ('copied
         (make-directory (file-name-directory (nth 0 entry)) t)
         (copy-file (nth 2 entry) (nth 0 entry) t)
         (push (nth 0 entry) restored))
        ('absent
         (when (file-exists-p (nth 0 entry))
           (delete-file (nth 0 entry)))
         (push (nth 0 entry) restored))))
    (nreverse restored)))

(defun pearl-undo-preimage-revertible-p (preimage)
  "Non-nil when PREIMAGE captured every path it was asked for."
  (seq-every-p (lambda (entry) (memq (nth 1 entry) '(copied absent)))
               (plist-get preimage :entries)))

;;; Patches and plans

(defun pearl-undo-patch-conflict-p (patch content)
  "Non-nil when PATCH cannot be reversed against CONTENT."
  (let ((written (plist-get patch :new-text)))
    (or (not (stringp written))
        (= 0 (length written))
        (not (string-match-p (regexp-quote written) (or content ""))))))

(defun pearl-undo--steps (entry)
  "Return what ENTRY contributes to a plan."
  (let* ((patch (plist-get entry :patch))
         (data (plist-get entry :data))
         (scope (plist-get data :scope))
         (preimage (plist-get data :preimage)))
    (cond
     (patch (list patch))
     (preimage (mapcar (lambda (path) (list :preimage path)) scope))
     (scope (mapcar (lambda (path) (list :restore path)) scope)))))

(defun pearl-undo--delegate-session (entry sessions)
  "The child ENTRY names, among SESSIONS, or nil."
  (let ((id (plist-get (plist-get entry :data) :delegate)))
    (and id (seq-find (lambda (candidate)
                        (equal id (pearl-session-id candidate)))
                      sessions))))

(defun pearl-undo--delegate-plan (entry sessions)
  "The child's plan when ENTRY names a delegate among SESSIONS."
  (let ((child (pearl-undo--delegate-session entry sessions)))
    (and child (pearl-undo-plan child))))

(defun pearl-undo-plan (session-or-sessions &optional entry)
  "Return the plan that takes SESSION-OR-SESSIONS back to ENTRY.

SESSION-OR-SESSIONS is a session, or a list of them when the branch
names a delegate.  A child writes to its own record, so a plan that
stopped at the parent would restore a tree the child had changed and
leave the change attributed to nobody."
  (let* ((sessions (if (pearl-session-p session-or-sessions)
                       (list session-or-sessions)
                     session-or-sessions))
         (branch (pearl-session-branch (car sessions)))
         (steps nil)
         (reached (null entry)))
    (dolist (each branch)
      (if (eq each entry)
          (setq reached t)
        (when reached
          (let ((child (pearl-undo--delegate-plan each sessions)))
            (if child
                (dolist (step child) (push step steps))
              (dolist (step (pearl-undo--steps each))
                (push step steps)))))))
    steps))

(defun pearl-undo-timeline (sessions)
  "Return all entries of SESSIONS in the order they happened."
  (let ((nodes nil)
        (index 0))
    (dolist (session sessions)
      (setq index (1+ index))
      (dolist (entry (pearl-session-entries session))
        (push (list :entry entry :session session :order index) nodes)))
    (mapcar (lambda (node) (list :entry (plist-get node :entry)
                                 :session (plist-get node :session)))
            (sort (nreverse nodes)
                  (lambda (a b)
                    (let ((ta (or (plist-get (plist-get a :entry) :timestamp) 0))
                          (tb (or (plist-get (plist-get b :entry) :timestamp) 0)))
                      (if (= ta tb)
                          (< (plist-get a :order) (plist-get b :order))
                        (< ta tb))))))))

(defun pearl-undo--paths-of (entry &optional sessions)
  "Return the paths ENTRY recorded writing.

A delegate writes to its own record, so with SESSIONS in hand the child's
entries are read too: a conflict that only looked at the parent would
call a tree unchanged after a child had changed it."
  (let* ((patch (plist-get entry :patch))
         (data (plist-get entry :data))
         (child (and sessions (pearl-undo--delegate-session entry sessions))))
    (append (or (and patch (list (plist-get patch :file)))
                (plist-get data :scope))
            (and child
                 (mapcan (lambda (each)
                           (pearl-undo--paths-of each (list child)))
                         (pearl-session-entries child))))))

(defun pearl-undo-plan-at (sessions node)
  "Return the plan that takes NODE of SESSIONS back, refusing crosswrites."
  (let* ((target (plist-get node :entry))
         (session (plist-get node :session))
         (plan (pearl-undo-plan
                (cons session (delq session (copy-sequence sessions)))
                target))
         (children (seq-keep (lambda (each)
                               (pearl-undo--delegate-session each sessions))
                             (pearl-session-branch session)))
         (timeline (pearl-undo-timeline sessions))
         (after (seq-drop-while (lambda (each) (not (eq (plist-get each :entry) target)))
                                timeline)))
    (dolist (step plan)
      (let ((path (or (plist-get step :file) (cadr step))))
        (dolist (each (cdr after))
          (unless (memq (plist-get each :session) (cons session children))
            (when (member path (pearl-undo--paths-of (plist-get each :entry)
                                                     sessions))
              (signal 'pearl-undo-conflict
                      (list :path path
                            :session (plist-get each :session)
                            :entry (plist-get each :entry))))))))
    plan))

;;; Turns

(defvar pearl-undo--latest (make-hash-table :test #'equal)
  "The snapshot of the last turn that changed something, per root and session.")

(defun pearl-undo-turn-begin (root)
  "Take a snapshot of ROOT and return the turn it begins."
  (list :root (or (pearl-undo--toplevel root) (expand-file-name root))
        :given (expand-file-name root)
        :snapshot (pearl-undo-snapshot root)
        :writes nil
        :session nil))

(defun pearl-undo--changed-paths (turn)
  "Return the paths of TURN's root that differ from its snapshot."
  (let ((snapshot (plist-get turn :snapshot)))
    (when snapshot
      (let ((listed (pearl-undo--git (plist-get snapshot :root)
                                     "ls-files" "-z" "--cached" "--others"
                                     "--exclude-standard")))
        (when (eq 0 (plist-get listed :status))
          (let ((candidates (split-string (plist-get listed :stdout) "\0" t))
                (kept (pearl-undo--git (plist-get snapshot :root)
                                       "ls-tree" "-r" "--name-only"
                                       (plist-get snapshot :commit))))
            ;; A file the snapshot has and the worktree no longer lists was
            ;; deleted, and a deletion is a change `ls-files' cannot see.
            (when (eq 0 (plist-get kept :status))
              (setq candidates (append candidates
                                       (split-string (plist-get kept :stdout)
                                                     "\n" t))))
            (seq-filter
             (lambda (path) (pearl-undo--moved-p snapshot path))
             (delete-dups
              (mapcar (lambda (rel) (expand-file-name rel (plist-get turn :given)))
                      candidates)))))))))

(defun pearl-undo-turn-end (turn)
  "Close TURN, returning the paths it changed."
  (let* ((snapshot (plist-get turn :snapshot))
         (writes (mapcar (lambda (path)
                           (if (file-name-absolute-p path)
                               path
                             (expand-file-name path (plist-get turn :given))))
                         (plist-get turn :writes)))
         (candidates (if writes writes (pearl-undo--changed-paths turn)))
         (moved (when snapshot
                  (seq-filter (lambda (path) (pearl-undo--moved-p snapshot path))
                              candidates))))
    (when snapshot
      (setq snapshot (plist-put snapshot :after
                                (mapcar (lambda (path)
                                          (cons (pearl-undo--canonical path)
                                                (pearl-undo--hash (expand-file-name path))))
                                        moved)))
      (plist-put turn :snapshot snapshot)
      (let ((key (list (plist-get turn :root) (plist-get turn :session))))
        (if moved
            (puthash key snapshot pearl-undo--latest)
          (remhash key pearl-undo--latest))
        (when (plist-get turn :session)
          (let ((default (list (plist-get turn :root) nil)))
            (if moved
                (puthash default snapshot pearl-undo--latest)
              (remhash default pearl-undo--latest))))))
    moved))

(defun pearl-undo-turn-snapshot (turn)
  "Return the snapshot TURN began with."
  (plist-get turn :snapshot))

(defun pearl-undo-latest (root session-id)
  "Return the last snapshot of SESSION-ID under ROOT, or nil."
  (let* ((toplevel (or (pearl-undo--toplevel root) (expand-file-name root))))
    (or (gethash (list toplevel session-id) pearl-undo--latest)
        (gethash (list toplevel nil) pearl-undo--latest))))

(defun pearl-undo-attach (loop root session-id)
  "Watch LOOP for ROOT and SESSION-ID, returning the unsubscribe."
  (let ((turn nil))
    (pearl-event-subscribe
     (pearl-loop-emitter loop)
     (lambda (event)
       (pcase (plist-get event :kind)
         ('turn-start
          (setq turn (pearl-undo-turn-begin root)))
         ('tool-execution-end
          (when turn
            (let* ((data (plist-get event :data))
                   (result (plist-get data :result))
                   (patch (plist-get result :patch))
                   (scope (or (plist-get result :scope)
                              (plist-get data :scope))))
              (when patch
                (plist-put turn :writes
                           (cons (plist-get patch :file) (plist-get turn :writes))))
              (dolist (path scope)
                (plist-put turn :writes
                           (cons path (plist-get turn :writes)))))))
         ('turn-end
          (when turn
            (plist-put turn :session session-id)
            (pearl-undo-turn-end turn)
            (setq turn nil))))))))

(provide 'pearl-undo)

;;; pearl-undo.el ends here
