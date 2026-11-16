;;; perf.el --- A project's own performance benchmarks  -*- lexical-binding: t; -*-

;;; Commentary:

;; Loaded by ci/perf.el after the shared benchmarks, which measure Emacs
;; rather than this package and are therefore not this file's business.
;; Register one here with `p/defperf' and put its budget in the :perf table
;; of config/ci.el, or it reports as unbudgeted.

;;; Code:

;; A benchmark of this package's own hot path. The name must match a budget
;; key in the :perf table of config/ci.el, or the table reports it as
;; unbudgeted. Add :tty t for one that needs a terminal to mean anything.
;;
;; (p/defperf first-turn
;;   "One turn through the fake provider, in ms."
;;   (let ((start (float-time)))
;;     (run-one-fake-turn)
;;     (/ (* 1000 (- (float-time) start)) 1)))

(provide 'perf)

;;; perf.el ends here
