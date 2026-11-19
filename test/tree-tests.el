;;; tree-tests.el --- stories about which tree a verdict is about  -*- lexical-binding: t; -*-

;;; Commentary:

;; A check, an edit and a commit all talk to a directory another session may
;; be writing.  These stories pin the name for that directory: what the
;; working tree looked like, and which paths moved between two looks.

;;; Code:

(require 'support-tests)
(require 'ert-gwt)

;; Nothing written between two reads is the same tree: a command that ran
;; between them can say its verdict is about the tree that is still there.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (once nil)
           (twice nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\n")))
  (:when (progn (setq once (pearl-tree-epoch root))
                (setq twice (pearl-tree-epoch root))))
  (:then once)
  (:then (equal once twice)))

;; A file written after the epoch was read makes the next epoch differ, and
;; the drift names it: the name is what lets a reader see whose half-finished
;; work the verdict ran over.
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (before nil)
           (after nil)
           (drift nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\n")))
  (:when (progn (setq before (pearl-tree-epoch root))
                (with-temp-file (expand-file-name "a.txt" root)
                  (insert "two\n"))
                (setq after (pearl-tree-epoch root))
                (setq drift (pearl-tree-drift before after))))
  (:then (not (equal before after)))
  (:then (equal '("a.txt") drift)))

;; Two regions of one file are one path, not two epochs: the fingerprint
;; answers "which paths moved", and a refusal is not built on it -- two
;; sessions may change different regions of one file and both be right
;; (ADR 33).
(ert-gwt-deftest
  (:given ((root (support-tests-git-root))
           (before nil)
           (after nil)
           (drift nil))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\ntwo\nthree\n")))
  (:when (progn (setq before (pearl-tree-epoch root))
                (with-temp-file (expand-file-name "a.txt" root)
                  (insert "ONE\ntwo\nthree\n"))
                (with-temp-file (expand-file-name "a.txt" root)
                  (insert "ONE\ntwo\nTHREE\n"))
                (setq after (pearl-tree-epoch root))
                (setq drift (pearl-tree-drift before after))))
  (:then (equal '("a.txt") drift))
  (:then (not (equal before after))))

;; Outside a repository the tree has no name: nil is a state the caller
;; reports, not a claim that nothing changed.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (epoch t))
          (with-temp-file (expand-file-name "a.txt" root)
            (insert "one\n")))
  (:when (setq epoch (pearl-tree-epoch root)))
  (:then (null epoch)))

(provide 'tree-tests)

;;; tree-tests.el ends here
