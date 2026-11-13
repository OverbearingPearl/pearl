.PHONY: lint test checks-doc checks-doc-verify dist release-check help

help:
	@echo "make lint              run the check suite and verify ci/CHECKS.md"
	@echo "make test              run the ERT test suite"
	@echo "make checks-doc        regenerate ci/CHECKS.md"
	@echo "make dist              build release artifacts from HEAD"
	@echo "make release-check     verify the git tag matches Version:"

lint:
	@ci/run-checks.sh
	@ci/run-checks.sh --verify-docs ci/CHECKS.md

test:
	@ci/run-tests.sh

checks-doc:
	@ci/run-checks.sh --write-docs ci/CHECKS.md

checks-doc-verify:
	@ci/run-checks.sh --verify-docs ci/CHECKS.md

dist:
	@ci/build-dist.sh

release-check:
	@ci/check-version-tag.sh
