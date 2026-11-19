.PHONY: lint contracts test perf perf-report size dist release-check hooks help

help:
	@echo "make lint              run the check suite and the contract audit"
	@echo "make contracts         check the promises against the stubs and stories"
	@echo "make test              run the ERT test suite"
	@echo "make perf              measure the hot paths against their budgets"
	@echo "make perf-report       print the measurements, never fail"
	@echo "make size              cost of each module, and the lines written twice"
	@echo "make hooks             install the commit, commit-msg and pre-push hooks"
	@echo "make dist              build release artifacts from HEAD"
	@echo "make release-check     verify the git tag matches Version:"

lint:
	@ci/run-checks.sh
	@$(MAKE) --no-print-directory contracts

# The contracts are the module's promises and the stories are what proves
# them; the suite cannot check either against the other because it is shared,
# and knows nothing about how this package writes a promise down.
contracts:
	@emacs -Q --batch -L scripts -l contracts \
		--eval '(if (contracts-report) (kill-emacs 0) (kill-emacs 1))'

test:
	@ci/run-tests.sh

hooks:
	@pre-commit install
	@pre-commit install --hook-type commit-msg
	@pre-commit install --hook-type pre-push

perf:
	@ci/run-perf.sh

perf-report:
	@ci/run-perf.sh --report

# What the loop can take as a unit whose proof is a number: how large a module
# is, and where the same lines sit twice. Like `perf-report', this prints and
# never fails -- size is not a fault, and a number that fails a build is a
# number somebody pads around.
size:
	@emacs -Q --batch -L scripts -l size --eval '(size-report)'

dist:
	@ci/build-dist.sh

release-check:
	@ci/check-version-tag.sh
