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
  (:then (with-current-buffer buffer (undo 1)))
  (:then (equal "before\n" (with-current-buffer buffer (buffer-string)))))

;; The one step holds under a real major mode, with several changes in
;; the group: font-lock and friends do not split the step (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-mode.el")) (buffer nil) (handle nil)))
  (:when (progn (with-temp-file file (insert "(defvar a 1)\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (emacs-lisp-mode) (setq handle (pearl-undo-group-prepare (current-buffer))) (goto-char (point-max)) (insert "(defvar b 2)\n") (insert "(defvar c 3)\n") (pearl-undo-group-accept handle))))
  (:then (with-current-buffer buffer (undo 1)))
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

;; A turn that changed nothing has nothing to report and nothing to keep.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root)) (turn nil) (moved nil)))
  (:when (progn (setq turn (pearl-undo-turn-begin root)) (setq moved (pearl-undo-turn-end turn))))
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

;; A file nobody touched since the turn is not a conflict.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (file (expand-file-name "still.txt" root))
           (turn nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (pearl-undo-turn-end turn)))
  (:then (not (pearl-undo-conflict-p (pearl-undo-turn-snapshot turn) file))))

;; Outside a git repository there is no snapshot, and that is a documented
;; state rather than a failure: the console says so.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (snapshot nil)))
  (:when (setq snapshot (pearl-undo-snapshot root)))
  (:then (null snapshot)))

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
  (:then (pearl-undo-latest root "story"))
  (:then (pearl-undo-restore (pearl-undo-latest root "story") (list file)))
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
          (let ((default-directory root)) (call-process "git" nil nil nil "add" "staged.txt")))
  (:when (progn (let ((default-directory root)) (setq before (with-temp-buffer (call-process "git" nil t nil "diff" "--cached") (buffer-string)))) (setq turn (pearl-undo-turn-begin root)) (with-temp-file other (insert "moved\n")) (pearl-undo-turn-end turn) (let ((default-directory root)) (setq after (with-temp-buffer (call-process "git" nil t nil "diff" "--cached") (buffer-string))))))
  (:then (equal before after))
  (:then (not (equal "" before))))

;; The snapshot respects .gitignore: an ignored file's changes are not the
;; turn's to report or to take back.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (ignored (expand-file-name "ignored.txt" root))
           (turn nil) (moved nil))
          (with-temp-file (expand-file-name ".gitignore" root) (insert "ignored.txt\n"))
          (with-temp-file ignored (insert "one\n")))
  (:when (progn (setq turn (pearl-undo-turn-begin root)) (with-temp-file ignored (insert "two\n")) (setq moved (pearl-undo-turn-end turn))))
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
  (:then (pearl-undo-latest root "left"))
  (:then (pearl-undo-latest root "right"))
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
;; reporting half a rollback as a rollback.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (outside (ert-gwt--temp-file "pearl-preimage-big.txt"))
           (pearl-undo-preimage-limit 4)
           (preimage nil))
          (with-temp-file outside (insert "far too large to copy\n")))
  (:when (setq preimage (pearl-undo-preimage root (list outside))))
  (:then (not (pearl-undo-preimage-revertible-p preimage))))

(provide 'undo-tests)

;;; undo-tests.el ends here
