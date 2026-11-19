;;; size.el --- what each module costs and what is written twice -*- lexical-binding: t; -*-

;;; Commentary:

;; A run can take a unit whose proof is a number instead of a story: two
;; modules that spell the same five lines twice collapse into one call, and
;; nothing a user can observe changes. That unit needs two things this file
;; supplies and the check suite does not: how big each module is, and where
;; the same lines sit twice.
;;
;; It is a report and never a check. Size is not a fault: `pearl-console.el'
;; is the largest module because presentation is the widest thing here, so
;; a number that fails a build is a number somebody pads around. The counts
;; of definitions come from `scripts/contracts.el', which already knows what
;; a definition is and which of the files are business modules.
;;
;; What it cannot see, said plainly because the report is read as fact: a
;; run of lines that is one idea with different names is not reported. This
;; matches text, not intent, so no run here means nobody copy-pasted, not
;; that nothing is duplicated.

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'contracts)

(defconst size-window 4
  "How many consecutive significant lines make a repetition.
Three matched lines is a shape two modules share by accident; four is the
shortest run a reader would call copy-paste.")

(defconst size-shown 20
  "How many repeated runs the report prints.
The longest are the ones worth collapsing, and a list nobody finishes
reading is a list nobody reads.")

(defconst size-large 300
  "How many lines make a module worth naming on the size line.
Presentation and the socket reader are large without being wrong; the
number is here so that the shape of the tree is visible at a glance, not
to mark those two.")

(defun size-significant (line)
  "Return LINE trimmed when it is worth comparing, nil otherwise.
A blank line, a comment and a `require' are lines two modules share
without sharing anything: every file has them, and none of them is the
duplicated body the report is for."
  (let ((text (string-trim line)))
    (and (> (length text) 8)
         (not (string-prefix-p ";" text))
         (not (string-prefix-p "(require" text))
         (not (string-prefix-p "(provide" text))
         text)))

(defun size-lines (file)
  "Return a vector of (LINE-NUMBER . TEXT) for FILE's significant lines."
  (with-temp-buffer
    (insert-file-contents file)
    (goto-char (point-min))
    (let ((found '()))
      (while (not (eobp))
        (let ((text (size-significant
                     (buffer-substring-no-properties
                      (line-beginning-position) (line-end-position)))))
          (when text
            (push (cons (line-number-at-pos) text) found)))
        (forward-line 1))
      (vconcat (nreverse found)))))

(defun size-windows (lines)
  "Return a hash of WINDOW text to the indexes in LINES it starts at."
  (let ((table (make-hash-table :test #'equal))
        (last (- (length lines) (1- size-window))))
    (dotimes (start (max 0 last))
      (let ((key (mapconcat (lambda (at) (cdr (aref lines at)))
                            (number-sequence start
                                             (+ start (1- size-window)))
                            "\n")))
        (push start (gethash key table))))
    table))

(defun size-index (modules)
  "Return two hashes: the lines of each module, and the windows in them.
MODULES is an alist of file name to absolute path; a window is located by
the module name and the index into that module's lines."
  (let ((lines (make-hash-table :test #'equal))
        (windows (make-hash-table :test #'equal)))
    (dolist (module modules)
      (let ((text (size-lines (cdr module))))
        (puthash (car module) text lines)
        (maphash (lambda (key starts)
                   (dolist (start starts)
                     (push (cons (car module) start)
                           (gethash key windows))))
                 (size-windows text))))
    (cons lines windows)))

(defun size-extend (lines first second)
  "Return how many lines match, starting at FIRST and SECOND in LINES.
A window that matched is extended as far as it goes, so a copy-pasted
block is reported once at its full length rather than once per window."
  (let* ((one (gethash (car first) lines))
         (two (gethash (car second) lines))
         (length size-window)
         (at (cdr first))
         (other (cdr second)))
    (while (and (< (+ at length) (length one))
                (< (+ other length) (length two))
                (equal (cdr (aref one (+ at length)))
                       (cdr (aref two (+ other length)))))
      (setq length (1+ length)))
    length))

(defun size-pairs (windows)
  "Return every pair of adjacent positions that share a window."
  (let ((pairs '()))
    (maphash (lambda (_key positions)
               (let ((sorted (sort positions
                                   (lambda (a b)
                                     (if (equal (car a) (car b))
                                         (< (cdr a) (cdr b))
                                       (string< (car a) (car b)))))))
                 (while (cdr sorted)
                   (push (cons (car sorted) (cadr sorted)) pairs)
                   (setq sorted (cdr sorted)))))
             windows)
    pairs))

(defun size-runs (lines windows)
  "Return (FILE LINE FILE LINE LENGTH TEXT) for the repeats found."
  (let ((runs '()))
    (dolist (pair (size-pairs windows))
      (let* ((first (car pair))
             (second (cdr pair))
             (one (aref (gethash (car first) lines) (cdr first)))
             (two (aref (gethash (car second) lines) (cdr second))))
        (push (list (car first) (car one)
                    (car second) (car two)
                    (size-extend lines first second) (cdr one))
              runs)))
    runs))

(defun size-covered-p (run kept)
  "Return non-nil when one end of RUN lies inside a run already KEPT."
  (cl-some (lambda (other)
             (cl-some (lambda (at)
                        (and (equal (car at) (car other))
                             (<= (nth 1 other) (cdr at)
                                 (+ (nth 1 other) (1- (nth 4 other))))))
                      (list (cons (nth 0 run) (nth 1 run))
                            (cons (nth 2 run) (nth 3 run)))))
           kept))

(defun size-longest (runs)
  "Return RUNS without the ones that a longer run already covers."
  (let ((kept '()))
    (dolist (run (sort runs (lambda (a b) (> (nth 4 a) (nth 4 b)))))
      (unless (size-covered-p run kept)
        (push run kept)))
    (nreverse kept)))

(defun size-modules ()
  "Return an alist of module name to absolute path, business modules only."
  (mapcar (lambda (file) (cons (file-name-nondirectory file) file))
          (contracts-business-files)))

(defun size-lines-in (file)
  "Return how many lines FILE has."
  (with-temp-buffer
    (insert-file-contents file)
    (count-lines (point-min) (point-max))))

(defun size-definitions-in (file definitions)
  "Return how many of DEFINITIONS belong to FILE."
  (seq-count (lambda (entry) (equal (cdr entry) file)) definitions))

(defun size-sum (numbers)
  "Return the sum of NUMBERS, and zero when there are none."
  (seq-reduce #'+ numbers 0))

(defun size-name (module)
  "Return MODULE's name without the package prefix or the extension."
  (string-remove-suffix ".el" (string-remove-prefix "pearl-" module)))

(defun size-by-decreasing (sizes)
  "Return SIZES, largest module first."
  (sort (copy-sequence sizes) (lambda (a b) (> (cadr a) (cadr b)))))

(defun size-report ()
  "Print the size of each module and the lines that repeat.
Returns how many repeated runs were found, which a caller may ignore: this
is a report, so nothing printed here is a failure."
  (let* ((modules (size-modules))
         (definitions (contracts-definitions))
         (sizes (mapcar (lambda (module)
                          (list (car module)
                                (size-lines-in (cdr module))
                                (size-definitions-in (cdr module)
                                                     definitions)))
                        modules))
         (ordered (size-by-decreasing sizes))
         (large (seq-filter (lambda (entry) (>= (cadr entry) size-large))
                            ordered))
         (index (size-index modules))
         (runs (size-longest (size-runs (car index) (cdr index)))))
    (princ (format "size: %d lines in %d modules, %d definitions\n"
                   (size-sum (mapcar #'cadr sizes))
                   (length sizes)
                   (size-sum (mapcar (lambda (entry) (nth 2 entry))
                                     sizes))))
    (princ (format "size: largest %s, %d lines and %d definitions\n"
                   (car (car ordered)) (cadr (car ordered))
                   (nth 2 (car ordered))))
    (princ (format "size: %d over %d lines: %s\n"
                   (length large) size-large
                   (mapconcat (lambda (entry)
                                (format "%s %d" (size-name (car entry))
                                        (cadr entry)))
                              large ", ")))
    (princ (format "size: %d runs of %d lines or more, %d lines in them\n"
                   (length runs) size-window
                   (size-sum (mapcar (lambda (run) (nth 4 run)) runs))))
    (dolist (run (seq-take runs size-shown))
      (princ (format "size: %2d  %s:%d and %s:%d  %s\n"
                     (nth 4 run)
                     (nth 0 run) (nth 1 run) (nth 2 run) (nth 3 run)
                     (nth 5 run))))
    (length runs)))

(provide 'size)
;;; size.el ends here
