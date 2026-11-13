;;; pearl.el --- Pi's agent loop in Emacs Lisp  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl

;; Author: OverbearingPearl
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; Version: 0.0.1
;; Package-Requires: ((emacs "29.4"))
;; Keywords: tools, processes
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; PEARL is a Lisp-native implementation of Pi's agent loop: a minimal,
;; model-agnostic loop that calls tools and feeds results back.
;;
;; This file is the package entry point.  The loop, tool dispatch,
;; message channels and session state live in the modules under lisp/.

;;; Code:

;;;###autoload
(defun pearl ()
  "Start a PEARL agent session."
  (interactive)
  (message "PEARL is not implemented yet"))

(provide 'pearl)

;;; pearl.el ends here
