;;; config.el --- Project policy for the check suite -*- lexical-binding: t; -*-

;;; Commentary:

;; Single source of policy.  Everything a check is allowed to tune lives
;; here.

;; The layout this policy describes is the standard MELPA one:
;;
;;   <pkg>.el          package entry point, the only .el at the repo root
;;   lisp/*.el         business modules
;;   test/*-tests.el   test modules
;;   ci/*.el           tooling: style checks only, no packaging rules
;;
;; Nothing else needs to change when this tree is copied into another
;; package with that layout.
;;
;; Test modules are free: neither the file name nor the definition
;; names inside it are required to carry the package prefix.  A trailing
;; -tests suffix is recommended anyway, because every module must
;; (provide) a feature named after its own file, and `test/<name>.el'
;; would provide the same feature as `lisp/<name>.el'; the suffix keeps
;; the two from shadowing each other on load-path.  Emacs itself does the
;; same (test/lisp/foo-tests.el against lisp/foo.el), and MELPA's default
;; file spec excludes *-tests.el, so tests never reach a published
;; package.

;;; Code:

(defvar p/config
  '(:emacs-versions ("29.4")
    :submodule-dir "lisp"
    :test-dir "test"
    :tooling-dir "ci"

    ;; Which module classes a given class may require.  Features outside
    ;; the repository are ignored.
    ;; What a module class *is*, in one table.  Every question a check
    ;; asks about a class is answered here: which headers it must (or
    ;; must not) carry, which classes it may require, and how long its
    ;; Commentary may be.  A class missing from this table is a mistake,
    ;; never a deliberate exemption; the `policy' check enforces that.
    ;;
    ;;   :headers      (:require SET) or (:forbid SET), see :header-sets
    ;;   :may-require  classes this class may `require'
    ;;   :commentary   (:min N :max M), nil :max means unbounded
    ;;
    ;; Published modules carry the package headers and need room to say
    ;; what they do.  Test and tooling modules are not published, so they
    ;; forbid the package headers (a `;; Version:' line in a file that is
    ;; not a package module is a false claim about the file) and keep
    ;; their Commentary short.
    :profiles
    ((main     :headers (:require :package)
               :may-require (business)
               :commentary (:min 2 :max nil))
     (business :headers (:require :submodule :forbid :entry-only)
               :may-require (business)
               :commentary (:min 2 :max nil))
     (test     :headers (:forbid :package)
               :may-require (main business test)
               :commentary (:min 1 :max 3))
     (tooling  :headers (:forbid :package)
               :may-require (tooling)
               :commentary (:min 1 :max nil)))

    ;; Every header line that marks a file as package metadata.  This is
    ;; the only place these strings appear; the sets below are derived.
    :package-headers
    ("Copyright" "Author" "Assisted-by" "URL" "Version"
     "SPDX-License-Identifier" "Package-Requires" "Keywords")

    ;; Three views of that one list:
    ;;   :package    every package header (the entry point carries all of
    ;;               them; files that are not the package carry none)
    ;;   :submodule  all but the entry-point-only ones
    ;;   :entry-only the entry-point-only ones, forbidden in submodules:
    ;;               a module does not version itself and does not
    ;;               declare its own dependencies or keywords
    :header-sets
    ((:package    :all)
     (:submodule  (:but "Version" "Package-Requires" "Keywords"))
     (:entry-only (:only "Version" "Package-Requires" "Keywords")))

    ;; Banned constructs: regex -> suggested replacement.
    :banned ((";; package-lint: disable" .
              "fix the underlying issue instead of suppressing it")
             ("\\_<declare-function\\_>" .
              "require the providing feature instead")
             ("\\_<ignore-errors\\_>" .
              "use condition-case with a specific error type")
             ("\\_<eval-and-compile\\_>" .
              "move the form or require it explicitly")
             ("\\_<when-let\\_>" . "use when-let* instead")
             ("\\_<if-let\\_>" . "use if-let* instead"))

    ;; Definition forms subject to the module prefix rule.
    :def-kinds (defun defmacro defsubst defvar defvar-local defcustom
                defconst defalias cl-defun cl-defmacro ert-deftest)

    :commentary-width 80
    :checkdoc-autofix never

    ;; Budgets for `make perf'.  Absolute numbers are generous multiples of
    ;; the measurements recorded in docs/architecture.md, wide enough to pass
    ;; on a loaded laptop; a ratio is used where a slow machine would
    ;; otherwise fail a good implementation.  `:max' fails above the value,
    ;; `:min' fails below it.
    :perf
    ((insert-per-chunk      :max 0.10)  ; measured 0.001 ms
     (insert-size-ratio     :max 3.0)   ; measured about 1.0: no O(n) insert
     (redisplay-per-chunk   :max 4.00)  ; measured 0.94 to 1.86 ms
     (batch-ratio           :min 4.0)   ; measured about 8x
     (filter-events-per-call :max 2.0)  ; measured 1.0: no coalescing
     (filter-gap-spread     :max 25.0)  ; measured 1 to 8 ms depending on load
     (json-per-event        :max 0.10)))  ; measured 0.01 to 0.03 ms
  "Project-wide policy for the check suite.")

(defun p/config-load-project ()
  "Load the project's own overrides, when it has any.
The suite is shared and gets replaced wholesale; the project's policy is
`config/ci.el` next to it, which a replacement never touches. Absent is a
normal state: the defaults above then apply."
  (let* ((here (file-name-directory (or load-file-name buffer-file-name)))
         (project (expand-file-name "../config/ci.el" here)))
    (when (file-exists-p project)
      (load project nil t))))

(p/config-load-project)

(defun p/config-value (key)
  "Return the value for KEY in `p/config'."
  (plist-get p/config key))

(defun p/class-prop (class prop)
  "Return property PROP of module CLASS from the :profiles table."
  (plist-get (cdr (assq class (p/config-value :profiles))) prop))

(defun p/config-export (args)
  "Print the value named by the first element of ARGS.
Keys are `submodule' (the business module directory), `test-dir' (the
test module directory), `main' (main file basename), `package' and
`version'."
  (setq args (delete "--" (copy-sequence args)))
  (let ((key (car args)))
    (princ
     (pcase key
       ("submodule" (p/config-value :submodule-dir))
       ("test-dir" (p/config-value :test-dir))
       ("main" (file-name-nondirectory (p/main-file)))
       ("package" (p/package-name))
       ("version" (or (p/package-version) ""))
       (_ (error "Unknown export key: %S" key))))))
