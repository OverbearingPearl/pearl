#!/usr/bin/env bash
# Run a cargo tool that emits JSON diagnostics and translate them into the
# suite's TSV finding records. The three tool checks (rustdoc, cargo-lint,
# build) share this so the orchestrator treats them like any other check.

# ci_cargo_findings NAME ROOT -- <cargo args...>
# Runs cargo from ROOT with --message-format=json, parses compiler-message
# diagnostics, prints TSV findings. A nonzero cargo exit with no diagnostic
# (a hard failure) becomes one ERROR record so the gate still trips.
ci_cargo_findings() {
    local name="$1" root="$2"
    shift 2
    [ "${1:-}" = "--" ] && shift
    local out rc
    out="$(cd "$root" && cargo "$@" --message-format=json 2>/dev/null)"
    rc=$?
    printf '%s\n' "$out" | python3 -c '
import json, sys
name = sys.argv[1]
rc = int(sys.argv[2])
seen = False
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") != "compiler-message":
        continue
    msg = m.get("message", {})
    level = msg.get("level", "")
    if level not in ("error", "warning"):
        continue
    text = msg.get("message", "")
    code = msg.get("code") or {}
    if isinstance(code, dict) and code.get("code"):
        text = "%s [%s]" % (text, code["code"])
    spans = msg.get("spans") or []
    primary = next((s for s in spans if s.get("is_primary")), spans[0] if spans else {})
    f = primary.get("file_name", "<cargo>")
    ln = primary.get("line_start", 1)
    sev = "ERROR" if level == "error" else "WARN"
    text = text.replace("\t", " ").replace("\n", " ")
    print("%s\t%s\t%s\t%s\t%s" % (sev, name, f, ln, text))
    seen = True
if rc != 0 and not seen:
    print("ERROR\t%s\t<cargo>\t1\tcargo exited %d with no diagnostic" % (name, rc))
' "$name" "$rc"
}