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
(require 'pearl-contract)

(define-error 'pearl-tool-invalid "A tool call is not valid")

(defvar pearl-tool-registry nil
  "The default tool list an extension rebinds (ADR 6).")

(defun pearl-tool-define (name declaration run &optional validate)
  "A tool called NAME, offering DECLARATION, running RUN after VALIDATE."
  (append (list :name name :run run :validate validate) declaration))

(defun pearl-tool-declarations (tools)
  "What TOOLS offer a model, in list order."
  (mapcar #'pearl-contract-tool-declaration tools))

(defun pearl-tool-list (&optional registry)
  "The tools in REGISTRY, or in `pearl-tool-registry'."
  (or registry pearl-tool-registry))

(defun pearl-tool-validate (tool arguments)
  "The ARGUMENTS TOOL accepts, or a signal naming the key that is wrong."
  (dolist (spec (plist-get tool :arguments))
    (when (and (plist-get (cdr spec) :required)
               (null (assoc (car spec) arguments)))
      (signal 'pearl-tool-invalid (list :key (car spec) :tool (plist-get tool :name)))))
  arguments)

(defun pearl-tool--result (tool content &optional patch)
  "The result of calling TOOL, carrying CONTENT and PATCH."
  (let ((result (pearl-contract-tool-result
                 (list :name (plist-get tool :name)) content)))
    (if patch (append result (list :patch patch)) result)))

(defun pearl-tool-run (tool arguments context)
  "Validate ARGUMENTS, run TOOL over them, and return a result."
  (condition-case error
      (let* ((validated (pearl-tool-validate tool arguments))
             (outcome (funcall (plist-get tool :run) validated context)))
        (if (and (listp outcome) (plist-get outcome :call))
            outcome
          (if (and (listp outcome) (plist-get outcome :content))
              (pearl-tool--result tool (plist-get outcome :content)
                                  (plist-get outcome :patch))
            (pearl-tool--result tool outcome))))
    ((quit) (pearl-tool-error-result tool "the user escaped"))
    (error (pearl-tool-error-result tool (error-message-string error)))))

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
