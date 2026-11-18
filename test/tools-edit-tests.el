;;; tools-edit-tests.el --- stories about writing through a buffer  -*- lexical-binding: t; -*-

;;; Commentary:

;; One edit call is one undo step: that is the promise, and the change
;; group is where it is kept.

;;; Code:

(require 'support-tests)
(require 'pearl-tools-edit)
(require 'ert-gwt)

;; The everyday path: an applied edit lands in the buffer and a clean buffer
;; is saved, so what the model changed is what a build sees.  The single undo
;; step that takes it back is the C-/ story below.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-edit.el")) (buffer nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new"))))
                                    (list :root (file-name-directory file)))))
  (:then (equal "new\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "new\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (not (buffer-modified-p buffer))))

;; A stale anchor is a loud error and a round trip: the buffer text, the
;; undo list and the file are all as they were, because a failed tool is
;; not an undo step (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-stale.el")) (buffer nil) (undo-before nil) (result nil))
          (with-temp-file file (insert "old\n"))
          (setq buffer (find-file-noselect file))
          (with-current-buffer buffer
            (goto-char (point-max))
            (insert "scratch")
            (undo-boundary)
            (undo 1)
            (setq undo-before (copy-tree buffer-undo-list))))
  (:when (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "missing" :new-text "new"))))
                                            (list :root (file-name-directory file)))))
  (:then (plist-get result :error))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string))))
  (:then (and undo-before
              (equal undo-before (with-current-buffer buffer (copy-tree buffer-undo-list)))))
  (:then (equal "old\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; Several replacements in one call are still one undo step, under a real
;; major mode: the whole call is one change group (M0.5).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-many.el")) (buffer nil)
           (edits (list (list :old-text "one" :new-text "1") (list :old-text "two" :new-text "2")))))
  (:when (progn (with-temp-file file (insert "one\ntwo\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits edits))
                                    (list :root (file-name-directory file))) (with-current-buffer buffer (undo-boundary) (undo 1))))
  (:then (equal "one\ntwo\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "1\n2\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (buffer-modified-p buffer)))

;; An anchor that matches twice is ambiguous, and the whole call is
;; refused naming the anchor: a guess is worse than a round trip (ADR 4).
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-ambiguous.el")) (buffer nil) (result nil))
          (with-temp-file file (insert "PLACEHOLDER\nPLACEHOLDER\n"))
          (setq buffer (find-file-noselect file)))
  (:when (setq result (pearl-tools-edit-run
                       (list (cons 'path file)
                             (cons 'edits
                                   (list (list :old-text "PLACEHOLDER" :new-text "y"))))
                       (list :root (file-name-directory file)))))
  (:then (plist-get result :error))
  (:then (string-match-p "PLACEHOLDER" (format "%S" (plist-get result :error))))
  (:then (equal "PLACEHOLDER\nPLACEHOLDER\n" (with-current-buffer buffer (buffer-string)))))

;; Overlapping anchors are refused before anything is written: a
;; half-applied edit is not a state the user can reach.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-overlap.el")) (buffer nil)
           (edits (list (list :old-text "ab" :new-text "1") (list :old-text "bc" :new-text "2")))
           (result nil)))
  (:when (progn (with-temp-file file (insert "abc\n")) (setq buffer (find-file-noselect file)) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits edits))
                                    (list :root (file-name-directory file))))))
  (:then (plist-get result :error))
  (:then (equal "abc\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "abc\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; A buffer the user had already changed is not saved by the tool: the
;; user's unsaved work is theirs, and the result says so.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-dirty.el")) (buffer nil) (result nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (with-current-buffer buffer (goto-char (point-min)) (insert ";; note\n")) (setq result (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new"))))
                                    (list :root (file-name-directory file))))))
  (:then (equal ";; note\nnew\n" (with-current-buffer buffer (buffer-string))))
  (:then (eq t (plist-get result :unsaved)))
  (:then (buffer-modified-p buffer))
  (:then (equal "old\n" (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; Apply is the one caller of the change-group API: it returns how many
;; replacements landed and the patch the entry will carry.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-apply.el")) (buffer nil)
           (edits (list (list :old-text "one" :new-text "1") (list :old-text "two" :new-text "2")))
           (applied nil)))
  (:when (progn (with-temp-file file (insert "one\ntwo\n")) (setq buffer (find-file-noselect file)) (setq applied (with-current-buffer buffer (pearl-tools-edit-apply buffer edits)))))
  (:then (equal 2 (car applied)))
  (:then (equal file (plist-get (cdr applied) :file)))
  (:then (equal "1\n2\n" (with-current-buffer buffer (buffer-string)))))

;; The declaration matches Pi's edit schema (ADR 8): edits[], each an
;; oldText and a newText, so a model trained on one works with the other.
(ert-gwt-deftest
  (:given ((declaration nil)))
  (:when (setq declaration (pearl-tools-edit-declaration)))
  (:then (equal "edit" (plist-get declaration :name)))
  (:then (equal '(edits) (mapcar #'car (plist-get declaration :arguments)))))

;; C-/ is buffer-scoped, and the spec says so out loud: after the undo the
;; text is the user's again, the saved file still holds the edit until the
;; next save, and the buffer is modified so the difference is visible.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-scope.el")) (buffer nil)))
  (:when (progn (with-temp-file file (insert "old\n")) (setq buffer (find-file-noselect file)) (pearl-tools-edit-run (list (cons 'path file) (cons 'edits (list (list :old-text "old" :new-text "new"))))
                                    (list :root (file-name-directory file))) (with-current-buffer buffer (undo-boundary) (undo 1))))
  (:then (equal "old\n" (with-current-buffer buffer (buffer-string))))
  (:then (equal "new\n" (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (buffer-modified-p buffer)))

;; A write the turn's snapshot cannot take back is not one this tool makes: a
;; path outside the root is refused by name, and the file is as it was.  The
;; shell has had this rule since ADR 16; the tool that writes for a living did
;; not, which made the net cover half the writes.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (outside (ert-gwt--temp-file "pearl-outside.el"))
           (result nil))
          (with-temp-file outside (insert "not yours\n")))
  (:when (setq result
               (pearl-tools-edit-run
                (list (cons 'path outside)
                      (cons 'edits
                            (list (list :old-text "not yours" :new-text "mine"))))
                (list :root root))))
  (:then (plist-get result :error))
  (:then (string-match-p "outside" (format "%S" (plist-get result :error))))
  (:then (equal "not yours\n"
                (with-temp-buffer
                  (insert-file-contents outside)
                  (buffer-string)))))

;; No root, no proof: a call with no context is refused rather than written
;; somewhere nothing will take it back.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-no-root.el")) (result nil))
          (with-temp-file file (insert "old\n")))
  (:when (setq result
               (pearl-tools-edit-run
                (list (cons 'path file)
                      (cons 'edits
                            (list (list :old-text "old" :new-text "new"))))
                nil)))
  (:then (plist-get result :error))
  (:then (equal "old\n"
                (with-temp-buffer
                  (insert-file-contents file)
                  (buffer-string)))))

;; Another writer changed a part of the file this edit does not touch, between
;; the read and the write: both changes land, because the edit is applied to
;; the file as it is now and not to the copy the buffer remembers (ADR 23).
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "shared.el" root))
           (buffer nil) (result nil))
          (with-temp-file file (insert "one\ntwo\n"))
          (setq buffer (find-file-noselect file)))
  (:when (progn
           (with-temp-file file (insert "one\ntwo\nthree\n"))
           (setq result
                 (pearl-tools-edit-run
                  (list (cons 'path file)
                        (cons 'edits
                              (list (list :old-text "two" :new-text "TWO"))))
                  (list :root root)))))
  (:then (null (plist-get result :error)))
  (:then (equal "one\nTWO\nthree\n"
                (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (equal "one\nTWO\nthree\n"
                (with-current-buffer buffer (buffer-string)))))

;; The same writer touches the anchored text instead: the whole call is refused
;; by name and the file is left as the other writer left it.  Merging here
;; would be guessing which "two" the model meant.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "contended.el" root))
           (buffer nil) (result nil))
          (with-temp-file file (insert "one\ntwo\n"))
          (setq buffer (find-file-noselect file)))
  (:when (progn
           (with-temp-file file (insert "one\nTWO-BY-SOMEONE\n"))
           (setq result
                 (pearl-tools-edit-run
                  (list (cons 'path file)
                        (cons 'edits
                              (list (list :old-text "two" :new-text "TWO"))))
                  (list :root root)))))
  (:then (plist-get result :error))
  (:then (string-match-p "two" (format "%S" (plist-get result :error))))
  (:then (string-match-p "changed" (format "%S" (plist-get result :error))))
  (:then (equal "one\nTWO-BY-SOMEONE\n"
                (with-temp-buffer (insert-file-contents file) (buffer-string)))))

;; The user has unsaved work and the file moved underneath it: refuse, name
;; both facts, and write neither.  There is no principled winner between them.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root))
           (file (expand-file-name "unsaved.el" root))
           (buffer nil) (result nil))
          (with-temp-file file (insert "one\ntwo\n"))
          (setq buffer (find-file-noselect file))
          (with-current-buffer buffer
            (goto-char (point-max))
            (insert ";; the user's own note\n")))
  (:when (progn
           (with-temp-file file (insert "one\ntwo\nthree\n"))
           (setq result
                 (pearl-tools-edit-run
                  (list (cons 'path file)
                        (cons 'edits
                              (list (list :old-text "two" :new-text "TWO"))))
                  (list :root root)))))
  (:then (plist-get result :error))
  (:then (plist-get result :unsaved))
  (:then (plist-get result :changed-on-disk))
  (:then (equal "one\ntwo\nthree\n"
                (with-temp-buffer (insert-file-contents file) (buffer-string))))
  (:then (with-current-buffer buffer (buffer-modified-p))))

(provide 'tools-edit-tests)

;;; tools-edit-tests.el ends here
