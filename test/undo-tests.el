;;; undo-tests.el --- stories about taking a change back  -*- lexical-binding: t; -*-

;;; Commentary:

;; Two layers: the change group the user reverts with C-/, and the
;; snapshot that survives the buffer and the editor.

;;; Code:

(require 'pearl-support-tests)
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
  (:given ((root (pearl-support-git-root))
           (file (expand-file-name "written.txt" root))
           (turn nil) (restored nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (pearl-undo-turn-end turn) (setq restored (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list file)))))
  (:then (equal (list file) restored))
  (:then (equal "before\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; turn-end reports what moved: the paths the turn changed, and nothing
;; else.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root))
           (file (expand-file-name "moved.txt" root))
           (turn nil) (moved nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (setq moved (pearl-undo-turn-end turn))))
  (:then (equal (list file) moved)))

;; A turn that changed nothing has nothing to report and nothing to keep.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root)) (turn nil) (moved nil)))
  (:when (progn (setq turn (pearl-undo-turn-begin root)) (setq moved (pearl-undo-turn-end turn))))
  (:then (null moved))
  (:then (null (pearl-undo-latest root))))

;; A file the user changed after the turn is a conflict, refused by name:
;; a wrong restore is worse than a refused one.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root))
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
  (:given ((root (pearl-support-git-root))
           (clean (expand-file-name "clean.txt" root))
           (dirty (expand-file-name "dirty.txt" root))
           (turn nil)))
  (:when (progn (with-temp-file clean (insert "before\n")) (with-temp-file dirty (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file clean (insert "after\n")) (with-temp-file dirty (insert "after\n")) (pearl-undo-turn-end turn) (with-temp-file dirty (insert "the user was here\n"))))
  (:then (should-error (pearl-undo-restore (pearl-undo-turn-snapshot turn) (list clean dirty)) :type 'pearl-undo-conflict))
  (:then (equal "after\n" (with-temp-buffer (insert-file-contents clean) (buffer-string))))
  (:then (equal "the user was here\n" (with-temp-buffer (insert-file-contents dirty) (buffer-string)))))

;; A file nobody touched since the turn is not a conflict.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root))
           (file (expand-file-name "still.txt" root))
           (turn nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq turn (pearl-undo-turn-begin root)) (with-temp-file file (insert "after\n")) (pearl-undo-turn-end turn)))
  (:then (not (pearl-undo-conflict-p (pearl-undo-turn-snapshot turn) file))))

;; Outside a git repository there is no snapshot, and that is a documented
;; state rather than a failure: the console says so.
(ert-gwt-deftest
  (:given ((root (pearl-support-temp-root)) (snapshot nil)))
  (:when (setq snapshot (pearl-undo-snapshot root)))
  (:then (null snapshot)))

;; Attach is the wiring the loop may not do itself: a turn that wrote a
;; file leaves a snapshot behind, and the restore from it puts the file back.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root))
           (file (expand-file-name "attached.txt" root))
           (provider (pearl-provider-fake '((tool-call "c1" "write" nil) (stop "end") (text "done") (stop "end"))))
           (tool (pearl-tool-define "write" '(:description "write") (lambda (_arguments _context) (with-temp-file file (insert "after\n")) "wrote")))
           (loop (pearl-loop-new :provider provider :tools (list tool) :session (pearl-session-new) :emit #'ignore))
           (unsubscribe nil)))
  (:when (progn (with-temp-file file (insert "before\n")) (setq unsubscribe (pearl-undo-attach loop root)) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (pearl-support-wait (lambda () (not (pearl-loop-busy-p loop))))))
  (:then (pearl-undo-latest root))
  (:then (pearl-undo-restore (pearl-undo-latest root) (list file)))
  (:then (equal "before\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:cleanup (when unsubscribe (funcall unsubscribe))))

;; A turn that changed nothing drops its snapshot: nothing to take back
;; means nothing kept.
(ert-gwt-deftest
  (:given ((root (pearl-support-git-root))
           (provider (pearl-provider-fake '((text "just talking") (stop "end"))))
           (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit #'ignore))
           (unsubscribe nil)))
  (:when (progn (setq unsubscribe (pearl-undo-attach loop root)) (pearl-loop-submit loop (pearl-contract-message 'user "go")) (pearl-support-wait (lambda () (not (pearl-loop-busy-p loop))))))
  (:then (null (pearl-undo-latest root)))
  (:cleanup (when unsubscribe (funcall unsubscribe))))

(provide 'undo-tests)

;;; undo-tests.el ends here
