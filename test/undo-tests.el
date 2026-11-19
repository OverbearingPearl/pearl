;;; undo-tests.el --- stories about taking a change back  -*- lexical-binding: t; -*-

;;; Commentary:

;; Two layers: the change group the user reverts with C-/, and the
;; snapshot that survives the buffer and the editor.

;;; Code:

(require 'support-tests)
(require 'pearl-undo)
(require 'pearl-loop)
(require 'pearl-session)
(require 'pearl-contract)
(require 'pearl-tool)
(require 'pearl-provider)
(require 'ert-gwt)

;; A cancelled group leaves the text as it was: a failed tool call is
;; not a state the user can reach.
(ert-gwt-deftest
  (:given ((buffer (generate-new-buffer "pearl-undo")) (handle nil) (undo-before nil)))
  (:when (progn (with-current-buffer buffer (insert "before\n") (undo-boundary) (setq undo-before buffer-undo-list) (setq handle (pearl-undo-group-prepare (current-buffer))) (insert "after\n") (pearl-undo-group-cancel handle))))
  (:then (equal "before\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal undo-before (with-current-buffer buffer buffer-undo-list))))

;; An accepted group is one undo step: exactly one C-/ takes back
;; everything the tool call did.
(ert-gwt-deftest
  (:given ((buffer (generate-new-buffer "pearl-undo-accept")) (handle nil)))
  (:when (progn (with-current-buffer buffer (insert "before\n") (undo-boundary) (setq handle (pearl-undo-group-prepare (current-buffer))) (insert "after\n") (pearl-undo-group-accept handle))))
  (:then (equal "before\nafter\n" (with-current-buffer buffer (buffer-string))))
  (:then (with-current-buffer buffer (let ((inhibit-message t)) (undo 1))))
  (:then (equal "before\n" (with-current-buffer buffer (buffer-string)))))

;; The one step holds under a real major mode, with several changes in
;; the group: font-lock and friends do not split the step (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-mode.el")) (buffer nil) (handle nil)))
  (:when (progn (with-temp-file file (insert "(defvar a 1)\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (emacs-lisp-mode) (setq handle (pearl-undo-group-prepare (current-buffer))) (goto-char (point-max)) (insert "(defvar b 2)\n") (insert "(defvar c 3)\n") (pearl-undo-group-accept handle))))
  (:then (with-current-buffer buffer (let ((inhibit-message t)) (undo 1))))
  (:then (equal "(defvar a 1)\n" (with-current-buffer buffer (buffer-string)))))

;; A turn's changes are revertible from the snapshot: the turn ended, the
;; files moved, and the restore puts them back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "written.txt" root))
           (turn nil) (restored nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (pearl-undo-turn-end turn) (setq restored (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list file)))))
  (:then (equal (list file) restored))
  (:then (equal "before\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; turn-end reports what moved: the paths the turn changed, and nothing
;; else.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "moved.txt" root))
           (turn nil) (moved nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (setq moved (pearl-undo-turn-end turn))))
  (:then (equal (list file) moved)))

;; A turn that changed nothing has nothing to report and nothing to keep: the
;; snapshot the previous turn left is not offered as the last turn's.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "quiet.txt" root))
           (turn nil) (kept nil) (moved nil)))
  (:when (progn
           (with-temp-file file (insert "before\n"))
           (setq turn (pearl-undo-turn-begin root))
           (with-temp-file file (insert "after\n"))
           (pearl-undo-turn-end turn)
           (setq kept (pearl-undo-latest root "story"))
           (setq turn (pearl-undo-turn-begin root))
           (setq moved (pearl-undo-turn-end turn))))
  (:then (equal (list file) (pearl-undo-restore kept (list file))))
  (:then (null moved))
  (:then (null (pearl-undo-latest root "story"))))

;; A file the user changed after the turn is a conflict, refused by name:
;; a wrong restore is worse than a refused one.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "conflict.txt" root))
           (turn nil) (err nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (pearl-undo-turn-end turn) (with-temp-file file (insert "the user was here\n")) (setq err (condition-case e (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list file)) (pearl-undo-conflict e)))))
  (:then (eq 'pearl-undo-conflict (car err)))
  (:then (string-match-p "conflict.txt" (format "%S" (cdr err))))
  (:then (pearl-undo-conflict-p (pearl-undo-turn-snapshot turn) file))
  (:then (equal "the user was here\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; One conflict refuses the whole restore: no path is written when any
;; path conflicts, so a refusal never leaves a half-restored tree.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (clean (expand-file-name "clean.txt" root))
           (dirty (expand-file-name "dirty.txt" root))
           (turn nil)))
  (:when (progn (with-temp-file clean (insert "before\n")) (with-temp-file dirty (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file clean (insert "after\n")) (with-temp-file dirty (insert "after\n")) (pearl-undo-turn-end turn) (with-temp-file dirty (insert "the user was here\n"))))
  (:then (should-error (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list clean dirty)) :type 'pearl-undo-conflict))
  (:then (equal "after\n" (with-temp-buffer (insert-file-contents clean) (buffer-string))))
  (:then (equal "the user was here\n" (with-temp-buffer (insert-file-contents dirty) (buffer-string)))))

;; A file nobody touched since the turn is not a conflict, while a file the
;; user changed since is: the two answers differ.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (clean (expand-file-name "still.txt" root))
           (dirty (expand-file-name "touched.txt" root))
           (turn nil))
          (with-temp-file clean (insert "before\n"))
          (with-temp-file dirty (insert "before\n")))
  (:when (progn (setq turn (pearl-undo-turn-begin root)) (with-temp-file clean (insert "after\n")) (with-temp-file dirty (insert "after\n")) (pearl-undo-turn-end turn) (with-temp-file dirty (insert "the user was here\n"))))
  (:then (not (pearl-undo-conflict-p (pearl-undo-turn-snapshot turn) clean)))
  (:then (pearl-undo-conflict-p (pearl-undo-turn-snapshot turn) dirty)))

;; Outside a git repository there is no snapshot, and that is a documented
;; state rather than a failure: the console says so.  Inside one there is,
;; which is what makes the nil mean "this root has none" rather than "never".
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (repo (support-tests-git-root)) (snapshot nil)))
  (:when (setq snapshot (pearl-undo-snapshot root)))
  (:then (null snapshot))
  (:then (pearl-undo-snapshot repo)))

;; The snapshot is of the turn's root, not of the repository the environment
;; pointed at: an Emacs started from a git hook inherits GIT_DIR, and one
;; started by a wrapper that pushes with `-c core.worktree=DIR' inherits that
;; setting too.  Either way a snapshot of that worktree records the wrong
;; files, so the turn's restore finds nothing to put back and calls the file
;; a conflict.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (other (support-tests-git-root))
           (file (expand-file-name "mine.txt" root))
           (leak (list (concat "GIT_DIR=" (expand-file-name ".git" other))
                       (concat "GIT_WORK_TREE=" other)
                       (format "GIT_CONFIG_PARAMETERS=core.worktree=%s" other)))
           (turn nil) (restored nil))
          (with-temp-file file (insert "before\n")))
  (:when (let ((process-environment (append leak process-environment)))
           (setq turn (pearl-undo-turn-begin root))
           (with-temp-file file (insert "after\n"))
           (pearl-undo-turn-end turn)
           (setq restored (pearl-undo-restore (pearl-undo-turn-snapshot turn)
                                              (list file)))))
  (:then (equal (list file) restored))
  (:then (equal "before\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; Attach is the wiring the loop may not do itself: a turn that wrote a
;; file leaves a snapshot behind, and the restore from it puts the file back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "attached.txt" root))
           (provider (pearl-provider-fake '((tool-call "c1" "write" nil) (stop "end") (text "done") (stop "end"))))
           (tool (pearl-tool-define "write" '(:description "write") (lambda (_arguments _context) (with-temp-file file (insert "after\n")) "wrote")))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session (pearl-session-new) :emit #'ignore))
           (unsubscribe nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq unsubscribe (pearl-undo-attach loop root "story")) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (support-tests-wait (lambda () (not (pearl-loop-busy-p loop))))))
  (:then (equal (list file) (pearl-undo-restore (pearl-undo-latest root "story") (list file))))
  (:then (equal "before\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:cleanup (when unsubscribe (funcall unsubscribe))))

;; A turn that changed nothing drops its snapshot: nothing to take back
;; means nothing kept.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (provider (pearl-provider-fake '((text "just talking") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))
           (unsubscribe nil)))
  (:when (progn (setq unsubscribe (pearl-undo-attach loop root "story")) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (support-tests-wait (lambda () (not (pearl-loop-busy-p loop))))))
  (:then (equal 1 (length (pearl-provider-fake-requests provider))))
  (:then (null (pearl-undo-latest root "story")))
  (:cleanup (when unsubscribe (funcall unsubscribe))))

;; The snapshot is written through a temporary index: the user's staged
;; changes are exactly what they were before the turn began.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (staged (expand-file-name "staged.txt" root))
           (other (expand-file-name "other.txt" root))
           (turn nil) (before nil) (after nil))
          (with-temp-file staged (insert "staged\n"))
          (support-tests-git root "add" "staged.txt"))
  (:when (progn (setq before (support-tests-git root "diff" "--cached")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file other (insert "moved\n")) (pearl-undo-turn-end turn) (setq after (support-tests-git root "diff" "--cached"))))
  (:then (equal before after))
  (:then (not (equal "" before))))

;; The snapshot respects .gitignore: an ignored file's changes are not the
;; turn's to report or to take back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (ignored (expand-file-name "ignored.txt" root))
           (other (expand-file-name "other.txt" root))
           (turn nil) (moved nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "ignored.txt\n"))
          (with-temp-file ignored (insert "one\n")))
  (:when (progn (setq turn (pearl-undo-turn-begin root)) (with-temp-file ignored (insert "two\n")) (with-temp-file other (insert "moved\n")) (setq moved (pearl-undo-turn-end turn))))
  (:then (equal (list other) moved))
  (:then (not (member ignored moved))))

;; A path the snapshot does not contain was created after it, so taking the
;; turn back deletes it: a restore that only writes files back leaves every
;; file a command made behind (ADR 23).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "created.txt" root))
           (turn nil)))
  (:when (progn
           (setq turn (pearl-undo-turn-begin root))
           (with-temp-file file (insert "made by a command\n"))
           (pearl-undo-turn-end turn)
           (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list file))))
  (:then (not (file-exists-p file))))

;; A path the snapshot contains and the worktree does not is written back from
;; it: deleting a file is a change a restore has to be able to reverse.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "gone.txt" root))
           (turn nil) (content nil))
          (with-temp-file file (insert "keep me\n")))
  (:when (progn
           (setq turn (pearl-undo-turn-begin root))
           (delete-file file)
           (pearl-undo-turn-end turn)
           (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list file))
           (setq content
                 (with-temp-buffer
                   (insert-file-contents file)
                   (buffer-string)))))
  (:then (equal "keep me\n" content)))

;; The last turn is the session's own: two consoles on one root must not offer
;; each other's history to undo (ADR 23).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (left-file (expand-file-name "left.txt" root))
           (right-file (expand-file-name "right.txt" root))
           (write-left (pearl-tool-define
                        "write-left" '(:description "write left")
                        (lambda (_arguments _context)
                          (with-temp-file left-file (insert "l\n")) "ok")))
           (write-right (pearl-tool-define
                         "write-right" '(:description "write right")
                         (lambda (_arguments _context)
                           (with-temp-file right-file (insert "r\n")) "ok")))
           (left-loop (pearl-loop-new
                       :provider (pearl-provider-fake
                                  '((tool-call "c1" "write-left" nil)
                                    (stop "end") (text "one") (stop "end")))
                       :tools (list write-left)
                       :session (pearl-session-new) :emit #'ignore))
           (right-loop (pearl-loop-new
                        :provider (pearl-provider-fake
                                   '((tool-call "c1" "write-right" nil)
                                     (stop "end") (text "one") (stop "end")))
                        :tools (list write-right)
                        :session (pearl-session-new) :emit #'ignore))
           (left-unsub nil) (right-unsub nil)))
  (:when (progn
           (setq left-unsub (pearl-undo-attach left-loop root "left"))
           (setq right-unsub (pearl-undo-attach right-loop root "right"))
           (pearl-loop-submit left-loop (pearl-contract-message 'user "go"))
           (pearl-loop-submit right-loop (pearl-contract-message 'user "go"))
           (support-tests-wait
            (lambda ()
              (and (not (pearl-loop-busy-p left-loop))
                   (not (pearl-loop-busy-p right-loop)))))))
  (:then (equal (list left-file) (pearl-undo-restore (pearl-undo-latest root "left") (list left-file))))
  (:then (equal (list right-file) (pearl-undo-restore (pearl-undo-latest root "right") (list right-file))))
  (:then (not (equal (pearl-undo-latest root "left")
                     (pearl-undo-latest root "right"))))
  (:cleanup (when left-unsub (funcall left-unsub))
            (when right-unsub (funcall right-unsub))))

;; A path outside the root is not in the turn's snapshot, so a command that
;; writes it needs its own copy: the pre-image is what makes a confirmed write
;; revertible (ADR 25).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (outside (ert-gwt--temp-file "pearl-preimage-outside.txt"))
           (preimage nil))
          (with-temp-file outside (insert "before\n")))
  (:when (progn
           (setq preimage (pearl-undo-preimage root (list outside)))
           (with-temp-file outside (insert "after\n"))
           (pearl-undo-preimage-restore preimage)))
  (:then (equal "before\n"
                (with-temp-buffer
                  (insert-file-contents outside)
                  (buffer-string)))))

;; A path that did not exist when the pre-image was taken is recorded as
;; absent, and taking it back deletes whatever appeared there.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (outside (make-temp-name (expand-file-name "pearl-preimage-new-"
                                                      temporary-file-directory)))
           (preimage nil)))
  (:when (progn
           (setq preimage (pearl-undo-preimage root (list outside)))
           (with-temp-file outside (insert "made by a command\n"))
           (pearl-undo-preimage-restore preimage)))
  (:then (not (file-exists-p outside))))

;; A path over the limit is not copied, and the pre-image says so rather than
;; reporting half a rollback as a rollback: the small path is still captured
;; and restored, so the limit reads as a limit and not as a blanket refusal.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (outside (ert-gwt--temp-file "pearl-preimage-big.txt"))
           (small (ert-gwt--temp-file "pearl-preimage-small.txt"))
           (pearl-undo-preimage-limit 10)
           (preimage nil) (restored nil))
          (with-temp-file outside (insert "far too large to copy\n"))
          (with-temp-file small (insert "before\n")))
  (:when (progn
           (setq preimage (pearl-undo-preimage root (list outside small)))
           (with-temp-file outside (insert "after\n"))
           (with-temp-file small (insert "after\n"))
           (setq restored (pearl-undo-preimage-restore preimage))))
  (:then (not (pearl-undo-preimage-revertible-p preimage)))
  (:then (equal (list small) restored))
  (:then (equal "after\n"
                (with-temp-buffer
                  (insert-file-contents outside)
                  (buffer-string))))
  (:then (equal "before\n"
                (with-temp-buffer
                  (insert-file-contents small)
                  (buffer-string)))))

;; What a session's record adds up to: the plan that takes it back, and the
;; question whether a patch can still be reversed against the file as it is.

(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b")) (plan nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil patch)) (pearl-session-append session (pearl-contract-entry 'message '(:content "hi"))) (setq plan (pearl-undo-plan session))))
  (:then (equal '("a.el") (delete-dups (mapcar (lambda (p) (plist-get p :file)) plan)))))

(ert-gwt-deftest
  (:given ((empty-patch '(:tool "delete" :file "a.el" :old-text "" :new-text ""))
           (conflict nil)))
  (:when (setq conflict (pearl-undo-patch-conflict-p empty-patch "")))
  (:then (eq t conflict)))

;; The plan reverses in reverse order: the last write on a path is the
;; first one taken back, because it was written against the one before it.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (first '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second '(:tool "edit" :file "a.el" :old-text "b" :new-text "c"))
           (plan nil)))
  (:when (progn (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil first)) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil second)) (setq plan (pearl-undo-plan session))))
  (:then (equal 2 (length plan)))
  (:then (equal "c" (plist-get (car plan) :new-text)))
  (:then (equal "b" (plist-get (cadr plan) :new-text))))

;; Rollback is path-scoped to the active branch: a patch on an abandoned
;; branch is never in the plan, whatever the head once was.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (root-entry (pearl-contract-entry 'message '(:content "root")))
           (left-patch '(:tool "edit" :file "left.el" :old-text "a" :new-text "b"))
           (right-patch '(:tool "edit" :file "right.el" :old-text "a" :new-text "b"))
           (plan nil)))
  (:when (progn (pearl-session-append session root-entry) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil left-patch)) (pearl-session-set-head session root-entry) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil right-patch)) (setq plan (pearl-undo-plan session))))
  (:then (equal '("right.el") (mapcar (lambda (p) (plist-get p :file)) plan))))

;; The plan stops at the entry it is given: taking a turn back never
;; reaches past the turn the user named.
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (first-patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second-patch '(:tool "edit" :file "b.el" :old-text "a" :new-text "b"))
           (boundary nil) (plan nil)))
  (:when (progn (setq boundary (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil first-patch))) (pearl-session-append session (pearl-contract-entry 'tool '(:name "edit") nil second-patch)) (setq plan (pearl-undo-plan session boundary))))
  (:then (equal '("b.el") (mapcar (lambda (p) (plist-get p :file)) plan))))

;; A patch whose written text is still in the file can be reversed: no
;; conflict, and the restore may proceed.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (conflict t)))
  (:when (setq conflict (pearl-undo-patch-conflict-p patch "b")))
  (:then (null conflict)))

;; A file the user changed since the write is a conflict: the written text
;; is gone, so reversing the patch would guess.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (conflict nil)))
  (:when (setq conflict (pearl-undo-patch-conflict-p patch "the user was here")))
  (:then (eq t conflict)))

;; A turn that only ran commands has no patch to reverse, so its plan cannot
;; be made of patches: it names the paths the command's scope proved and asks
;; the snapshot to restore them (ADR 23).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (path "/repo/out.txt")
           (entry (pearl-contract-entry
                   'tool (list :name "shell" :scope (list path) :content "ok")))
           (plan nil)))
  (:when (progn (pearl-session-append session entry)
                (setq plan (pearl-undo-plan session))))
  (:then (equal (list (list :restore path)) plan)))

;; A command that scribbled outside the snapshot is taken back from its own
;; pre-image, and the plan says which kind of step that is, so the console can
;; tell "this line comes back" from "this file comes back" from "this write was
;; captured before it happened" (ADR 25).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (path "/tmp/outside.txt")
           (entry (pearl-contract-entry
                   'tool (list :name "shell"
                               :scope (list path)
                               :preimage (list :id "p1")
                               :content "ok")))
           (plan nil)))
  (:when (progn (pearl-session-append session entry)
                (setq plan (pearl-undo-plan session))))
  (:then (equal (list (list :preimage path)) plan)))

;; A fork leaves the files where they are, so the console asks what the
;; abandoned turns wrote *before* moving the head.  The move itself applies
;; nothing, which is why asking afterwards answers nothing (ADR 29).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (root-entry nil) (before nil)
           (patch '(:tool "edit" :file "src/a.el" :old-text "a" :new-text "b")))
          (setq root-entry (pearl-session-append
                            session (pearl-contract-entry 'message '(:content "one"))))
          (pearl-session-append
           session (pearl-contract-entry 'tool '(:name "edit") nil patch))
          (setq before (pearl-undo-plan session root-entry)))
  (:when (pearl-session-set-head session root-entry))
  (:then (equal '("src/a.el") (mapcar (lambda (p) (plist-get p :file)) before)))
  (:then (null (pearl-undo-plan session root-entry))))

;; The page is the root's: two sessions come back as one list in the order
;; their entries happened, each node naming the session it belongs to.
(ert-gwt-deftest
  (:given ((a (pearl-session-new)) (b (pearl-session-new)) (timeline nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'message '(:content "a1")))
           (pearl-session-append b (pearl-contract-entry 'message '(:content "b1")))
           (pearl-session-append a (pearl-contract-entry 'message '(:content "a2")))
           (setq timeline (pearl-undo-timeline (list a b)))))
  (:then (equal '("a1" "b1" "a2")
                (mapcar (lambda (n)
                          (plist-get (plist-get (plist-get n :entry) :data)
                                     :content))
                        timeline)))
  (:then (eq b (plist-get (cadr timeline) :session))))

;; A node from another session is undone by naming it: the plan is that
;; session's own, from its head back to the node and nothing else.
(ert-gwt-deftest
  (:given ((a (pearl-session-new)) (b (pearl-session-new))
           (first '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second '(:tool "edit" :file "a.el" :old-text "b" :new-text "c"))
           (node nil) (plan nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'tool '(:name "edit") nil first))
           (pearl-session-append b (pearl-contract-entry 'message '(:content "unrelated")))
           (setq node (car (pearl-undo-timeline (list a b))))
           (pearl-session-append a (pearl-contract-entry 'tool '(:name "edit") nil second))
           (setq plan (pearl-undo-plan-at (list a b) node))))
  (:then (equal '("a.el") (mapcar (lambda (p) (plist-get p :file)) plan)))
  (:then (equal "c" (plist-get (car plan) :new-text))))

;; Reverting another session's work is allowed and has to name it: a later
;; entry on another branch that wrote the same path refuses the plan, and the
;; refusal carries the session and the entry to point at.
(ert-gwt-deftest
  (:given ((a (pearl-session-new)) (b (pearl-session-new))
           (first '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (second '(:tool "edit" :file "a.el" :old-text "b" :new-text "c"))
           (other '(:tool "edit" :file "a.el" :old-text "c" :new-text "d"))
           (node nil) (err nil)))
  (:when (progn
           (pearl-session-append a (pearl-contract-entry 'tool '(:name "edit") nil first))
           (setq node (car (pearl-undo-timeline (list a b))))
           (pearl-session-append a (pearl-contract-entry 'tool '(:name "edit") nil second))
           (pearl-session-append b (pearl-contract-entry 'tool '(:name "edit") nil other))
           (condition-case e
               (pearl-undo-plan-at (list a b) node)
             (error (setq err e)))))
  (:then (eq 'pearl-undo-conflict (car err)))
  (:then (eq b (plist-get (cdr err) :session)))
  (:then (equal "a.el" (plist-get (cdr err) :path))))

;; A delegate writes to its own record, so a plan that stopped at the parent
;; would restore a tree the child had changed and attribute it to nobody.
(ert-gwt-deftest
  (:given ((parent (pearl-session-new))
           (child (pearl-session-new nil "child-1"))
           (plan nil))
          (pearl-session-append
           parent (pearl-contract-entry
                   'tool (list :name "delegate" :content "delegated"
                               :delegate "child-1")))
          (pearl-session-append
           child (pearl-contract-entry
                  'tool (list :name "edit" :content "done")
                  nil '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))))
  (:when (setq plan (pearl-undo-plan (list parent child))))
  (:then (equal 1 (length plan)))
  (:then (equal "a.el" (plist-get (car plan) :file))))

;; A delegate's writes are on the child's record, so a conflict check that
;; read only the parent would call a changed tree unchanged.
(ert-gwt-deftest
  (:given ((parent (pearl-session-new nil "parent-1"))
           (child (pearl-session-new nil "child-2"))
           (other (pearl-session-new nil "other-3"))
           (target nil)
           (err nil))
          (setq target
                (pearl-session-append
                 parent (pearl-contract-entry
                         'tool (list :name "first" :content "before"))))
          (plist-put target :timestamp 1.0)
          (plist-put
           (pearl-session-append
            parent (pearl-contract-entry
                    'tool (list :name "delegate" :content "delegated"
                                :delegate "child-2")))
           :timestamp 2.0)
          (plist-put
           (pearl-session-append
            child (pearl-contract-entry
                   'tool (list :name "edit" :content "done")
                   nil '(:tool "edit" :file "a.el" :old-text "a" :new-text "b")))
           :timestamp 3.0)
          (plist-put
           (pearl-session-append
            other (pearl-contract-entry
                   'tool (list :name "edit" :content "later")
                   nil '(:tool "edit" :file "a.el" :old-text "b" :new-text "c")))
           :timestamp 4.0))
  (:when (condition-case e
             (pearl-undo-plan-at
              (list parent child other)
              (list :entry target :session parent))
           (error (setq err e))))
  (:then (eq 'pearl-undo-conflict (car err)))
  (:then (equal "a.el" (plist-get (cdr err) :path)))
  (:then (eq other (plist-get (cdr err) :session))))

(provide 'undo-tests)

;;; undo-tests.el ends here
