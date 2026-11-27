#!/usr/bin/env bash
# The test matrix for a platform that runs its suites apart: one shard per
# workspace member, discovered from the manifest rather than named by hand.
#
# A workflow cannot know how many crates a project has, and a list written
# into it goes stale the first time a crate is added. This prints the matrix
# GitHub Actions expects on stdout, so a job can fan out over whatever the
# workspace holds today:
#
#   shards: {"shard":[{"name":"core","packages":"core"}, ...]}
#
# CI_TEST_SHARD_SIZE groups that many members into one shard (default 1).
# Grouping trades parallelism for fewer cold builds; a workspace of tiny
# crates is better served by a handful of jobs than by one job each.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
SIZE="${CI_TEST_SHARD_SIZE:-1}"

cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT/Cargo.toml" |
    python3 -c '
import json, sys

size = max(1, int(sys.argv[1]))
members = json.load(sys.stdin)
names = sorted(p["name"] for p in members["packages"])
shards = []
for start in range(0, len(names), size):
    group = names[start:start + size]
    # A shard is named for its members, so a red job says which crates it
    # owned without opening the log.
    first = group[0]
    label = first if len(group) == 1 else "%s+%d" % (first, len(group) - 1)
    shards.append({"name": label, "packages": " ".join(group)})
print("shards=" + json.dumps({"shard": shards}))
' "$SIZE"
