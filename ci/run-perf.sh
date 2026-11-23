#!/usr/bin/env bash
# `make perf` (--report) and `make perf-check` (the nightly gate).
#
# Runs criterion benches and reads target/criterion/<name>/new/estimates.json.
# --report prints the timings and never fails. Without --report, each budget
# in [perf] is enforced: mean must be <= max (and >= min when given). A bench
# with no budget is reported, not gated. No benches at all is a clean exit.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
REPORT=0
[ "${1:-}" = "--report" ] && REPORT=1

if ! ls "$ROOT"/crates/*/benches/*.rs >/dev/null 2>&1 && ! ls "$ROOT"/benches/*.rs >/dev/null 2>&1; then
    printf 'perf: no benches found; nothing to measure.\n'
    exit 0
fi

cargo bench --workspace --manifest-path "$ROOT/Cargo.toml" >/dev/null 2>&1 || true

python3 - "$ROOT" "$HERE" "$REPORT" <<'PY'
import json, os, subprocess, sys
root, here, report = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
tools = os.path.join(here, "tools", "target", "debug", "ci-tools")
crit = os.path.join(root, "target", "criterion")
means = {}
if os.path.isdir(crit):
    for name in sorted(os.listdir(crit)):
        est = os.path.join(crit, name, "new", "estimates.json")
        if os.path.isfile(est):
            try:
                with open(est) as fh:
                    means[name] = json.load(fh)["mean"]["point_estimate"]
            except (ValueError, KeyError):
                pass
if not means:
    print("perf: no criterion estimates produced.")
    sys.exit(0)
print("perf report (mean ns, lower is better):")
for name, ns in means.items():
    print("  %-32s %12.1f ns" % (name, ns))
if report:
    sys.exit(0)
# Gate mode: enforce configured budgets.
def budget(name):
    try:
        out = subprocess.run([tools, "config", "get", "--root", root,
                              "--suite", here, "perf." + name],
                             capture_output=True, text=True)
        if out.returncode != 0:
            return None
        kv = dict(p.split("=", 1) for p in out.stdout.split() if "=" in p)
        return kv
    except Exception:
        return None
fail = 0
for name, ns in means.items():
    b = budget(name)
    if not b:
        continue
    if b.get("max") and ns > float(b["max"]):
        print("FAILED: %s mean %.1f ns exceeds max %s ns" % (name, ns, b["max"]))
        fail += 1
    if b.get("min") and ns < float(b["min"]):
        print("FAILED: %s mean %.1f ns below min %s ns" % (name, ns, b["min"]))
        fail += 1
sys.exit(1 if fail else 0)
PY