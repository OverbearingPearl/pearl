.PHONY: lint test perf perf-report checks-doc checks-doc-verify dist release-check hooks help

help:
	@echo "make lint              run the check suite and verify CHECKS.md"
	@echo "make test              run the ERT test suite"
	@echo "make perf              measure the hot paths against their budgets"
	@echo "make perf-report       print the measurements, never fail"
	@echo "make hooks             install the commit, commit-msg and pre-push hooks"
	@echo "make checks-doc        regenerate CHECKS.md"
	@echo "make dist              build release artifacts from HEAD"
	@echo "make release-check     verify the git tag matches Version:"

lint:
	@ci/run-checks.sh
	@ci/run-checks.sh --verify-docs CHECKS.md

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

checks-doc:
	@ci/run-checks.sh --write-docs CHECKS.md

checks-doc-verify:
	@ci/run-checks.sh --verify-docs CHECKS.md

dist:
	@ci/build-dist.sh

release-check:
	@ci/check-version-tag.sh
