;;; perf.el --- PEARL's own hot paths, against the roadmap's numbers  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console's two M4 budgets, measured on the package rather than on
;; Emacs: the per-delta insert cost, and one screenful of a 100k-line
;; transcript with a real redisplay.  A benchmark registers only once the
;; function it measures exists, so the red stages report the substrate
;; alone and no budget claims a pass before there is code to measure.
;;
;; The third measurement has no budget yet, and that is on purpose.  M9 takes
;; a snapshot of the tree before a turn's first tool call, so its cost is paid
;; on every turn and a tree of any size pays it; the number is what decides
;; where the budget goes, and a budget written first would be a guess wearing
;; a threshold's clothes.

;;; Code:

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../lisp" here)))

(require 'pearl-console)
(require 'pearl-event)
(require 'pearl-undo)

(defvar pearl-perf-git-root nil
  "The snapshot benchmark's tree, created once per run.
Building a 2000-file repository costs more than the snapshot of it, and the
three runs of one benchmark should not each pay for the scaffolding.")

(defun pearl-perf-make-git-root (files)
  "Return a committed git repository holding FILES small files."
  (let* ((root (make-temp-file "pearl-perf-tree" t))
         (default-directory root)
         (git (lambda (&rest args)
                (apply #'call-process "git" nil nil nil args))))
    (funcall git "init" "-q")
    (funcall git "config" "user.email" "perf@example.com")
    (funcall git "config" "user.name" "Perf")
    (dotimes (i files)
      (with-temp-file (expand-file-name (format "f%05d.el" i) root)
        (insert ";; a file the snapshot has to walk\n")))
    (funcall git "add" "-A")
    (funcall git "commit" "-q" "-m" "perf")
    (push (lambda () (delete-directory root t)) kill-emacs-hook)
    root))

(when (and (fboundp 'pearl-console--open)
           (fboundp 'pearl-console-insert-event))
  (p/defperf pearl-insert-per-chunk
    "One console delta inserted, in ms (M4: at most 0.3)."
    (let ((console (pearl-console--open nil temporary-file-directory "perf"))
          (event (pearl-event-new 'message-update '(:message "m" :delta "x"))))
      (unwind-protect
          (let ((start (float-time)))
            (dotimes (_ 600)
              (pearl-console-insert-event console event))
            (/ (* 1000 (- (float-time) start)) 600.0))
        (kill-buffer console)))))

(when (and (fboundp 'pearl-console--open)
           (fboundp 'pearl-console-insert-event)
           (fboundp 'pearl-console-redraw))
  (p/defperf pearl-screenful :tty t
    "One screenful of a 100k-line transcript, in ms (M4: at most 50)."
    (let ((console (pearl-console--open nil temporary-file-directory "perf-page"))
          (event (pearl-event-new 'message-update
                                  (list :message "m"
                                        :delta (apply #'concat (make-list 100000 "line\n"))))))
      (unwind-protect
          (progn
            (pearl-console-insert-event console event)
            (pearl-console-redraw console)
            (switch-to-buffer console)
            (with-current-buffer console (goto-char (point-min)))
            (let ((start (float-time)))
              (scroll-up-command)
              (redisplay t)
              (* 1000 (- (float-time) start))))
        (kill-buffer console)))))

;; The snapshot a turn takes before its first tool call, measured on the tree
;; size a project actually reaches rather than on a toy one: a snapshot that
;; costs 30 ms on ten files can cost seconds on ten thousand, and the delay is
;; in front of every turn, not once at start-up.
(when (fboundp 'pearl-undo-snapshot)
  (p/defperf pearl-snapshot-per-turn
    "One turn's snapshot of a 2000-file tree, in ms (M9: budget not set yet)."
    (let ((root (or pearl-perf-git-root
                    (setq pearl-perf-git-root (pearl-perf-make-git-root 2000))))
          (start (float-time)))
      (pearl-undo-snapshot root)
      (* 1000 (- (float-time) start)))))

(provide 'pearl-perf)

;;; perf.el ends here
