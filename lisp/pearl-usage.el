;;; pearl-usage.el --- what a request cost, and who counted it  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A price is a fact about a request and not part of one: it is written
;; beside the turn that paid it and never travels back to the model.  The
;; provider may report the count; when it does not, the estimator answers,
;; and the entry says which of the two spoke.
;;
;; The arithmetic is here rather than in the loop or the console, because a
;; turn's price, a session's sum and a page's total must be the same numbers
;; added the same way, and `pearl-compaction-tokens' is already the tree's
;; one definition of what a token is.
;;
;; Planned API
;;
;; `pearl-usage-prompt' (MESSAGES) -> integer
;;      The estimated prompt tokens of MESSAGES, which is what a provider
;;      that reports nothing leaves a reader to estimate from.
;;
;; `pearl-usage-entry' (PROMPT COMPLETION &rest KEYS) -> entry
;;      The entry a finished request writes: PROMPT and COMPLETION in
;;      tokens, and KEYS `:first-delta' (seconds from the request to the
;;      first token of the answer), `:total' (seconds the request took) and
;;      `:reported' (non-nil when a provider counted rather than an
;;      estimator).  A count nobody reported is still a count, and the flag
;;      is what keeps the page from printing a guess as a fact.

;;; Code:

(require 'cl-lib)
(require 'pearl-compaction)
(require 'pearl-contract)
(require 'pearl-session)

(defconst pearl-usage-kind 'usage
  "The entry kind a request's price is written under.")

(defun pearl-usage-prompt (messages)
  "Return the estimated prompt tokens of MESSAGES."
  (cl-loop for message in messages
           sum (pearl-compaction-tokens
                (or (pearl-contract-text message) ""))))

(defun pearl-usage-entry (prompt completion &rest keys)
  "Record what a request cost, in PROMPT and COMPLETION tokens, with KEYS.
The flags that ride along are `:first-delta', `:total' and `:reported'."
  (pearl-contract-entry
   pearl-usage-kind
   (list :prompt prompt
         :completion completion
         :first-delta (plist-get keys :first-delta)
         :total (plist-get keys :total)
         :reported (and (plist-get keys :reported) t))))

(provide 'pearl-usage)

;;; pearl-usage.el ends here
