;;; perf.el --- PEARL's own hot paths, against the roadmap's numbers  -*- lexical-binding: t; -*-

;;; Commentary:

;; The console's two M4 budgets, measured on the package rather than on
;; Emacs: the per-delta insert cost, and one screenful of a 100k-line
;; transcript with a real redisplay.  A benchmark registers only once the
;; function it measures exists, so the red stages report the substrate
;; alone and no budget claims a pass before there is code to measure.

;;; Code:

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../lisp" here)))

(require 'pearl-console)
(require 'pearl-event)

(when (fboundp 'pearl-console-insert-event)
  (p/defperf pearl-insert-per-chunk
    "One console delta inserted, in ms (M4: at most 0.3)."
    (let ((console (pearl-console-open temporary-file-directory "perf"))
          (event (pearl-event-new 'message-update '(:message "m" :delta "x"))))
      (unwind-protect
          (let ((start (float-time)))
            (dotimes (_ 600)
              (pearl-console-insert-event console event))
            (/ (* 1000 (- (float-time) start)) 600.0))
        (kill-buffer console)))))

(when (and (fboundp 'pearl-console-insert-event)
           (fboundp 'pearl-console-redraw))
  (p/defperf pearl-screenful :tty t
    "One screenful of a 100k-line transcript, in ms (M4: at most 50)."
    (let ((console (pearl-console-open temporary-file-directory "perf-page"))
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

(provide 'pearl-perf)

;;; perf.el ends here
