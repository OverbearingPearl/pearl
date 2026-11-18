;;; compaction-tests.el --- stories about what a summary covers  -*- lexical-binding: t; -*-

;;; Commentary:

;; A compaction is an entry whose only effect is on the projection: nothing is
;; deleted, and what it covers can be read back.  The threshold is a question,
;; not a policy, because a policy that fires silently is a policy nobody saw.

;;; Code:

(require 'support-tests)
(require 'pearl-compaction)
(require 'pearl-session)
(require 'pearl-contract)
(require 'ert-gwt)

;; A compaction is an entry, not a rewrite: the entries it covers stop being
;; projected and stay on the branch, which is what makes a summary reversible
;; and a replay honest (ADR 27).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (compaction nil))
          (dolist (text '("one" "two" "three" "four"))
            (pearl-session-append
             session (pearl-contract-entry 'message (pearl-contract-message 'user text)))))
  (:when (setq compaction
               (pearl-compaction-append session "three rounds, summarised"
                                        :keep 1)))
  (:then (eq 'compaction (plist-get compaction :kind)))
  (:then (equal 3 (length (pearl-compaction-covers compaction))))
  (:then (equal 3 (length (pearl-compaction-covered session compaction))))
  (:then (equal 5 (length (pearl-session-entries session))))
  (:then (null (seq-find (lambda (message)
                           (equal "one" (plist-get message :content)))
                         (pearl-session-projection session nil))))
  (:then (seq-find (lambda (message)
                     (string-match-p "summarised"
                                     (format "%s" (plist-get message :content))))
                   (pearl-session-projection session nil))))

;; The threshold is a question the caller asks, and the answer is a name so a
;; second trigger can say its own.
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (tight nil) (roomy nil) (estimate nil))
          (pearl-session-append
           session (pearl-contract-entry
                    'message (pearl-contract-message
                              'user (make-string 4000 ?x)))))
  (:when (progn
           (setq estimate (pearl-compaction-estimate session))
           (setq tight (pearl-compaction-needed session 100))
           (setq roomy (pearl-compaction-needed session 1000000))))
  (:then (>= estimate 1000))
  (:then (eq 'threshold tight))
  (:then (null roomy)))

;; A summary that fits comfortably does not cover anything: the cut is where
;; the budget ran out, and a budget that was never spent leaves the whole tail
;; in front of the reader.
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (compaction nil) (folded nil))
          (dolist (text '("one" "two" "three"))
            (pearl-session-append
             session (pearl-contract-entry 'message (pearl-contract-message 'user text)))))
  (:when (progn
           (setq compaction (pearl-compaction-append session "nothing to fold"
                                                    :keep 10000))
           (setq folded (pearl-compaction-covers compaction))))
  (:then (null folded))
  (:then (equal 3
                (length (seq-filter
                         (lambda (message)
                           (not (eq 'system (plist-get message :role))))
                         (pearl-session-projection session nil))))))

(provide 'compaction-tests)

;;; compaction-tests.el ends here
