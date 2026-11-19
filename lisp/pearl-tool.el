;;; pearl-tool.el --- tool shape, dispatch and validation  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The shape a tool has to have, and the trust boundary it crosses.  Whatever
;; a model emits is unvalidated text in a data structure; this module is the
;; one place that turns it into arguments.  After that point no module checks
;; again, and a tool that fails returns a result marked as an error rather than
;; signalling: the loop must be able to carry on and show the model what went
;; wrong.
;;
;; Which tools exist is not decided here.  A tool is data, and the list is one
;; of the three seams an extension may replace (ADR 6).
;;
;; Planned API
;;
;; `pearl-tool-define' (NAME DECLARATION RUN &optional VALIDATE) -> tool
;;      DECLARATION is the description and argument schema shown to the model,
;;      RUN is called with validated arguments and a context plist, VALIDATE is
;;      an optional predicate that runs before RUN.
;;
;; `pearl-tool-list' (&optional REGISTRY) -> list of tools
;;      The default registry is `pearl-tool-registry', the variable an
;;      extension rebinds; there is no discovery step.
;;
;; `pearl-tool-declarations' (TOOLS) -> list of declarations
;;      What the provider is told is available, in list order.
;;
;; `pearl-tool-validate' (TOOL ARGUMENTS) -> validated arguments
;;      Signals `pearl-tool-invalid' with the offending key.  Called once, at
;;      the boundary, and never again downstream.  Borrow: pi validates in one
;;      place for the same reason, pinned in ADR 8.
;;
;; `pearl-tool-run' (TOOL ARGUMENTS CONTEXT) -> tool result
;;      Validate, call, and turn every outcome into a result through
;;      `pearl-contract-tool-result': a signal becomes an error result, a
;;      missing field becomes an error result, and an `escape' from the user
;;      becomes an aborted result.  Never a signal.
;;
;;      A successful result may carry `:patch', which is present only for a
;;      tool that wrote outside the session.  It is opaque here: the tool
;;      layer carries it, the loop records it, the session never interprets
;;      it.  A result without one is a conversational entry and cannot be
;;      rolled back.
;;
;; `pearl-tool-error-result' (TOOL MESSAGE) -> tool result
;;      The error shape, so every failure reads the same in a transcript.
;;
;; `pearl-tool-truncate' (TEXT &key HEAD TAIL LIMIT) -> (TEXT . TRUNCATEDP)
;;      Output bounding with an explicit marker, because a model that cannot
;;      tell it saw a fragment will reason about the fragment as if it were the
;;      whole.  Borrow: pi's head and tail windows, pinned in ADR 8.

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'pearl-contract)

(define-error 'pearl-tool-invalid "A tool call is not valid")

(defvar pearl-tool-registry 'default
  "The tools an extension rebinds (ADR 6).
`default' is the set this package ships, a list replaces it, and nil
takes every tool away.")

(defun pearl-tool-define (name declaration run &optional validate)
  "A tool called NAME, offering DECLARATION, running RUN after VALIDATE."
  (append (list :name name :run run :validate validate) declaration))

(defun pearl-tool-declarations (tools)
  "What TOOLS offer a model, in list order."
  (mapcar #'pearl-contract-tool-declaration tools))

(defun pearl-tool-list (&optional registry)
  "The tools in REGISTRY, or in `pearl-tool-registry'."
  (or registry pearl-tool-registry))

(defun pearl-tool--argument-key (key)
  "Return KEY as a keyword, in kebab case.
The model is shown the schema's own spelling, so a key arrives as
`oldText', as a string, or as `:old-text'; a tool is written against one
of those and this is where the three become one."
  (let ((name (cond ((keywordp key) (symbol-name key))
                    ((symbolp key) (symbol-name key))
                    ((stringp key) key)
                    (t (format "%s" key)))))
    (intern (concat ":" (downcase
                         (let ((case-fold-search nil))
                           (replace-regexp-in-string
                            "\\([a-z0-9]\\)\\([A-Z]\\)" "\\1-\\2"
                            (string-remove-prefix ":" name))))))))

(defun pearl-tool-normalize-arguments (arguments)
  "Return ARGUMENTS as a plist whose keys are kebab-case keywords.
A plist, an alist and the JSON that produced either are all accepted,
and nested objects are converted the same way, because a tool should be
able to name its own arguments once."
  (pearl-tool--normalize-pairs arguments))

(defun pearl-tool--alist-p (value)
  "Non-nil when VALUE is a list of conses whose keys are names.
A keyword key is read as a plist holding an object, not as an alist key,
because a list of plists is what a schema with a list argument produces."
  (and (consp value) (consp (car value))
       (let ((key (car (car value))))
         (and (not (keywordp key)) (or (symbolp key) (stringp key))))))

(defun pearl-tool--object-p (value)
  "Non-nil when VALUE is a plist or an alist."
  (and (consp value) (or (keywordp (car value)) (pearl-tool--alist-p value))))

(defun pearl-tool--normalize-value (value)
  "Return VALUE with the keys of every object inside it normalised."
  (cond
   ((pearl-tool--object-p value) (pearl-tool-normalize-arguments value))
   ((and (consp value) (consp (car value)))
    (mapcar #'pearl-tool--normalize-value value))
   (t value)))

(defun pearl-tool--normalize-pairs (arguments)
  "Convert ARGUMENTS, a plist or an alist, to a plist of keywords."
  (let ((pairs (if (keywordp (car arguments))
                   (cl-loop for (key value) on arguments by #'cddr
                            collect (cons key value))
                 arguments))
        result)
    (dolist (pair pairs)
      (push (pearl-tool--argument-key (car pair)) result)
      (push (pearl-tool--normalize-value (cdr pair)) result))
    (nreverse result)))

(defun pearl-tool-validate (tool arguments)
  "The ARGUMENTS TOOL accepts, or a signal naming the key that is wrong."
  (dolist (spec (plist-get tool :arguments))
    (when (and (plist-get (cdr spec) :required)
               (null (plist-get arguments (pearl-tool--argument-key (car spec)))))
      (signal 'pearl-tool-invalid
              (list :key (car spec) :tool (plist-get tool :name)))))
  arguments)

(defun pearl-tool--result (tool content &optional patch)
  "The success shape for TOOL, carrying CONTENT and PATCH."
  (let ((result (pearl-contract-tool-result
                 (list :name (plist-get tool :name)) content nil)))
    (if patch (plist-put result :patch patch) result)))

(defun pearl-tool-run (tool arguments context)
  "Validate ARGUMENTS, run TOOL over them, and return a result.
ARGUMENTS arrive in whatever shape the wire used; they are read as one
kind of plist here so every tool below names its own arguments once."
  (condition-case error
      (let* ((arguments (pearl-tool-normalize-arguments arguments))
             (validated (pearl-tool-validate tool arguments))
             (outcome (funcall (or (plist-get tool :run-async)
                                   (plist-get tool :run))
                               validated context)))
        (cond
         ((pearl-tool-pending-p outcome) outcome)
         ((and (listp outcome) (plist-get outcome :call)) outcome)
         ((and (listp outcome) (plist-get outcome :content))
          (pearl-tool--result tool (plist-get outcome :content)
                              (plist-get outcome :patch)))
         (t (pearl-tool--result tool outcome))))
    ((quit) (pearl-tool-error-result tool "the user escaped"))
    (error (pearl-tool-error-result tool (error-message-string error)))))

(defun pearl-tool-pending-p (outcome)
  "Non-nil when OUTCOME is a tool that is still running.
A pending outcome is a plist with a starter under `:start' and a canceller
under `:cancel'; the loop calls the starter with a callback and the
canceller when the run is stopped, which is what lets a command be stopped
while it runs instead of being waited for."
  (and (listp outcome)
       (functionp (plist-get outcome :start))
       (functionp (plist-get outcome :cancel))))

(defun pearl-tool-error-result (tool message)
  "The failure shape for TOOL, carrying MESSAGE."
  (pearl-contract-tool-result (list :name (plist-get tool :name)) message t))

(defconst pearl-tool--limit 4000
  "How much output a tool result carries before it is bounded.")

(defun pearl-tool--marker (omitted)
  "Return the marker for OMITTED characters, stating how many are missing."
  (format "\n[%d characters omitted]\n" omitted))

(cl-defun pearl-tool-truncate (text &key head tail limit)
  "TEXT bounded to HEAD and TAIL characters, or to LIMIT twice."
  (let* ((limit (or limit pearl-tool--limit))
         (length (length text)))
    (cond
     ((and head tail)
      (if (<= length (+ head tail))
          (cons text nil)
        (cons (concat (substring text 0 head)
                      (pearl-tool--marker (- length head tail))
                      (substring text (- length tail)))
              t)))
     (head
      (if (<= length head)
          (cons text nil)
        (cons (concat (substring text 0 head)
                      (pearl-tool--marker (- length head)))
              t)))
     (tail
      (if (<= length tail)
          (cons text nil)
        (cons (concat (pearl-tool--marker (- length tail))
                      (substring text (- length tail)))
              t)))
     (t
      (if (<= length limit)
          (cons text nil)
        (let ((half (ceiling limit 2.0)))
          (cons (concat (substring text 0 half)
                        (pearl-tool--marker (- length limit))
                        (substring text (- length (- limit half))))
                t)))))))

(provide 'pearl-tool)

;;; pearl-tool.el ends here
