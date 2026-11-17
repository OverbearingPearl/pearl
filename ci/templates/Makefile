.PHONY: lint test perf perf-report dist release-check hooks help

help:
	@echo "make lint              run the check suite"
	@echo "make test              run the ERT test suite"
	@echo "make perf              measure the hot paths against their budgets"
	@echo "make perf-report       print the measurements, never fail"
	@echo "make hooks             install the commit, commit-msg and pre-push hooks"
	@echo "make dist              build release artifacts from HEAD"
	@echo "make release-check     verify the git tag matches Version:"

lint:
	@ci/run-checks.sh

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

dist:
	@ci/build-dist.sh

release-check:
	@ci/check-version-tag.sh
