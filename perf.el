;;; perf.el --- Performance budgets for the hot paths  -*- lexical-binding: t; -*-

;;; Commentary:

;; The numbers this package's design rests on, as budgets rather than prose.
;; `make perf' measures them and fails when one goes over; `make perf-report'
;; prints the same table and never fails, for a machine that is not quiet.
;;
;; Two kinds of budget live here.  Absolute ones are generous multiples of the
;; baselines in docs/architecture.md, wide enough to survive a loaded machine.
;; Relative ones compare two measurements taken in the same run, because a
;; ratio survives a slow machine where a millisecond does not.
;;
;; The benchmarks measure Emacs, not this package: what a buffer insert costs,
;; what a forced redisplay costs, how promptly a process filter is called, what
;; a JSON event costs to parse.  They are the substrate the design is budgeted
;; against, so they are worth guarding before there is code to guard.

;;; Code:

(require 'cl-lib)
(require 'json)

(defconst p/perf-runs 3
  "Runs per benchmark.  The median is taken, so one noisy run cannot fail a
build on its own.")

(defconst p/perf-chunks 300
  "Chunks inserted by the streaming benchmarks.  Small enough to keep the
suite under a few seconds, large enough that per-chunk cost is not dominated
by setup.")

(defvar p/perf-registry nil
  "Benchmarks, oldest first.  Each entry is a plist with :name, :doc,
:budget, :needs-tty and :fn.")

(defun p/perf-tty-p ()
  "Return non-nil when redisplay actually paints something.
Set by `ci/run-perf.sh' when it could run Emacs under a terminal."
  (equal (getenv "PERF_TTY") "1"))

(defmacro p/defperf (name &rest args)
  "Define a benchmark NAME.
ARGS is an optional :tty keyword, a docstring, then the body.  The body must
return a number to be measured, or nil to skip.  The budget is not here: it
lives in the policy file with every other number."
  (declare (indent 2))
  (let ((plist nil) (doc nil) (body nil))
    (while (keywordp (car args))
      (setq plist (append plist (list (pop args) (pop args)))))
    (setq doc (pop args) body args)
    `(setq p/perf-registry
           (append p/perf-registry
                   (list (list :name ',name :doc ,doc
                               :needs-tty ,(plist-get plist :tty)
                               :fn (lambda () ,@body)))))))

(defun p/perf-median (values)
  "Return the median of VALUES, a list of numbers."
  (let* ((sorted (sort (copy-sequence values) #'<))
         (n (length sorted)))
    (if (cl-evenp n)
        (/ (+ (nth (1- (/ n 2)) sorted) (nth (/ n 2) sorted)) 2.0)
      (float (nth (/ n 2) sorted)))))

(defun p/perf-chunk (i)
  "Return the stream chunk numbered I."
  (format "token%03d the loop and the session and the patch. " i))

(defun p/perf-fill (lines)
  "Insert LINES lines of already-read conversation into the current buffer."
  (dotimes (i lines)
    (insert (format "- old round %d: 3 files changed, 2 tests passed\n" i))))

(defun p/perf-setup (lines)
  "Return a console-like buffer holding LINES lines of history."
  (let ((buffer (get-buffer-create " *perf-console*")))
    (with-current-buffer buffer
      (erase-buffer)
      (text-mode)
      (font-lock-mode 1)
      (p/perf-fill lines))
    buffer))

;;; Benchmarks

(p/defperf insert-per-chunk
  "Insert one stream chunk without redisplay, in ms."
  (p/perf-insert-cost 2000))

(defun p/perf-insert-cost (lines)
  "Return the per-chunk insert cost, in ms, in a buffer of LINES lines."
  (with-current-buffer (p/perf-setup lines)
    (let ((start (float-time)))
      (dotimes (i p/perf-chunks)
        (goto-char (point-max))
        (insert (p/perf-chunk i)))
      (/ (* 1000 (- (float-time) start)) (float p/perf-chunks)))))

(p/defperf insert-size-ratio
  "Insert cost in a long buffer over the cost in an empty one."
  (let ((small (p/perf-insert-cost 0))
        (large (p/perf-insert-cost 20000)))
    (if (zerop small) 999.0 (/ large small))))

(p/defperf redisplay-per-chunk :tty t
  "Insert one chunk and force a redisplay, in ms."
  (with-current-buffer (p/perf-setup 2000)
    (switch-to-buffer (current-buffer))
    (let ((start (float-time)))
      (dotimes (i p/perf-chunks)
        (goto-char (point-max))
        (insert (p/perf-chunk i))
        (redisplay t))
      (/ (* 1000 (- (float-time) start)) p/perf-chunks))))

(p/defperf batch-ratio :tty t
  "How many times cheaper a redisplay every 20 chunks is, as a ratio."
  (let (each batched)
    (with-current-buffer (p/perf-setup 0)
      (switch-to-buffer (current-buffer))
      (let ((start (float-time)))
        (dotimes (i p/perf-chunks)
          (goto-char (point-max))
          (insert (p/perf-chunk i))
          (redisplay t))
        (setq each (- (float-time) start))))
    (with-current-buffer (p/perf-setup 0)
      (switch-to-buffer (current-buffer))
      (let ((start (float-time)))
        (dotimes (i p/perf-chunks)
          (goto-char (point-max))
          (insert (p/perf-chunk i))
          (when (zerop (% (1+ i) 20))
            (redisplay t)))
        (setq batched (- (float-time) start))))
    (if (zerop batched) 999.0 (/ each batched))))

(defun p/perf-filter-facts ()
  "Return (EVENTS-PER-CALL . GAP-SPREAD) for a 20 ms producer.
Events per filter call says whether the process layer coalesces deliveries;
the spread of the gaps says whether a delivery was ever stalled.  Both are
independent of the producer's exact period, which is why they are measured
instead of the gap itself."
  (let* ((start (float-time))
         (arrivals nil)
         (calls 0)
         (events 0)
         (proc (make-process
                :name "perf-consumer"
                :command (list "sh" "-c"
                               (concat "i=0; while [ $i -lt 40 ]; do "
                                       "echo 'data: x'; echo; "
                                       "i=$((i+1)); sleep 0.02; done"))
                :connection-type 'pipe
                :coding 'utf-8-unix
                :noquery t
                :filter (lambda (_p chunk)
                          (setq calls (1+ calls))
                          (dolist (line (split-string chunk "\n" t))
                            (when (string-prefix-p "data:" line)
                              (setq events (1+ events))
                              (push (- (float-time) start) arrivals)))))))
    (while (process-live-p proc)
      (accept-process-output proc 0.05))
    (accept-process-output nil 0.05)
    (let ((sorted (sort arrivals #'<))
          (gaps nil))
      (while (cdr sorted)
        (push (* 1000 (- (cadr sorted) (car sorted))) gaps)
        (setq sorted (cdr sorted)))
      (cons (if (zerop calls) 0.0 (/ (float events) calls))
            (if gaps (- (cl-reduce #'max gaps) (cl-reduce #'min gaps)) 0.0)))))

(p/defperf filter-events-per-call
  "Events delivered per process-filter call.  Above one means coalescing."
  (car (p/perf-filter-facts)))

(p/defperf filter-gap-spread
  "Difference between the widest and narrowest delivery gap, in ms."
  (cdr (p/perf-filter-facts)))

(p/defperf json-per-event
  "Parse one streamed event object, in ms."
  (let* ((events (cl-loop for i below 300
                          collect (format "{\"i\":%d,\"s\":\"text\"}" i)))
         (start (float-time)))
    (dolist (event events)
      (with-temp-buffer
        (insert event)
        (goto-char (point-min))
        (json-parse-buffer :object-type 'plist)))
    (/ (* 1000 (- (float-time) start)) (float (length events)))))

;;; Runner

(defun p/perf-budget (name)
  "Return the budget plist registered for benchmark NAME."
  (cdr (assq name (p/config-value :perf))))

(defun p/perf-expectation (entry)
  "Return the budget plist for ENTRY, read from the policy file."
  (p/perf-budget (plist-get entry :name)))

(defun p/perf-verdict (value budget)
  "Return `ok', `warn' or `skip' for VALUE against BUDGET."
  (cond
   ((numberp (plist-get budget :max))
    (if (<= value (plist-get budget :max)) 'ok 'over))
   ((numberp (plist-get budget :min))
    (if (>= value (plist-get budget :min)) 'ok 'under))
   (t 'unbudgeted)))

(defun p/perf-measure (entry)
  "Return the median measurement for ENTRY, or nil when it does not apply."
  (let ((fn (plist-get entry :fn)))
    (when (or (not (plist-get entry :needs-tty)) (p/perf-tty-p))
      (p/perf-median
       (cl-loop repeat p/perf-runs
                for value = (funcall fn)
                when (numberp value) collect value)))))

(defun p/perf-line (entry value budget verdict)
  "Return the report line for ENTRY with VALUE, BUDGET and VERDICT."
  (let ((name (symbol-name (plist-get entry :name))))
    (format "%-20s %9s %11s  %-4s  %s"
            name
            (if value (format "%.4f" value) "-")
            (cond ((numberp (plist-get budget :max))
                   (format "max %.2f" (plist-get budget :max)))
                  ((numberp (plist-get budget :min))
                   (format "min %.2f" (plist-get budget :min)))
                  (t "-"))
            (pcase verdict ('ok "ok") ('over "OVER") ('under "UNDER")
                   ('skip "skip") (_ "?"))
            (plist-get entry :doc))))

(defun p/perf-main (args)
  "Run every benchmark in ARGS and report.  Return the exit status.
`--report' prints the same table but never fails.  The table goes to the file
named by PERF_OUT; the status, to PERF_STATUS, so a wrapper such as `script'
cannot lose it."
  (let ((report-only (member "--report" args))
        (failures 0)
        (out (or (getenv "PERF_OUT") nil))
        (table nil))
    (push (format "%-20s %9s %11s  %-4s  %s"
                  "benchmark" "median" "budget" "stat" "what it measures")
          table)
    (dolist (entry p/perf-registry)
      (let* ((value (p/perf-measure entry))
             (budget (p/perf-expectation entry))
             (verdict (if (numberp value) (p/perf-verdict value budget) 'skip)))
        (when (memq verdict '(over under)) (setq failures (1+ failures)))
        (push (p/perf-line entry value budget verdict) table)))
    (setq table (concat (mapconcat #'identity (nreverse table) "\n") "\n"))
    (if out
        (with-temp-file out (insert table))
      (princ table))
    (let ((status (if (and (> failures 0) (not report-only)) 1 0))
          (status-file (getenv "PERF_STATUS")))
      (when status-file
        (with-temp-file status-file (insert (format "%d" status))))
      status)))

(provide 'perf)

;;; perf.el ends here
