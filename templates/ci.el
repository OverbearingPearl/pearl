;;; ci-config.el --- Project policy for the check suite  -*- lexical-binding: t; -*-

;;; Commentary:

;; Written by ci/bootstrap.sh and owned by this repository: the suite in ci/
;; is shared and gets replaced, this file does not. It is loaded after
;; ci/config.el, so anything set here wins.
;;
;; Only the keys a project usually differs in are listed; the rest come from
;; ci/config.el. Delete a form to take the default back.

;;; Code:

;; Declared by ci/config.el; declared here as well so this file can be
;; byte-compiled on its own without a free-variable warning, which is what a
;; reviewer's byte-compile step would otherwise report.
(defvar p/config)

;; The oldest Emacs the package claims to support.
(setq p/config (plist-put p/config :emacs-versions '("29.4")))

;; Where the suite looks for modules, when the layout is not the default.
;; (setq p/config (plist-put p/config :submodule-dir "lisp"))
;; (setq p/config (plist-put p/config :test-dir "test"))

;; A benchmark of this package's own hot path belongs in config/perf.el, which
;; ci/perf.el loads when it exists: (p/defperf name "what it measures" body).
;; Its budget goes in the :perf table below, under the same name.

;; Performance budgets, when the defaults do not fit the machine or the code.
;; (setq p/config
;;       (plist-put p/config :perf
;;                  '((insert-per-chunk    :max 0.10)
;;                    (insert-size-ratio   :max 3.0)
;;                    (redisplay-per-chunk :max 4.00)
;;                    (batch-ratio         :min 4.0)
;;                    (filter-events-per-call :max 2.0)
;;                    (filter-gap-spread   :max 25.0)
;;                    (json-per-event      :max 0.10))))

(provide 'ci)

;;; ci-config.el ends here
