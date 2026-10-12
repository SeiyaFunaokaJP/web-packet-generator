#!/usr/bin/env python3
"""Compares `wpg bench --json` / pacing results with thresholds (nightly CI).

Usage: scripts/bench-check.py THRESHOLDS.json RESULTS_DIR [--summary FILE]

THRESHOLDS.json is a list of checks:

  {"name": "...", "file": "null.json", "metric": "pps", "min": 5e6}
  {"name": "...", "file": "pacing.json",
   "select": {"wait": "hybrid", "clock": "tsc", "pps": 100000},
   "metric": "ratio", "min": 0.99, "max": 1.01}

`file` is relative to RESULTS_DIR. A bench result is one JSON object; a pacing
result (`cargo run --example pacing -- --json`) is a list whose element is picked
with `select`. A missing file fails the check unless the check has
`"optional": true` (e.g. a backend the runner cannot open). Writes a Markdown table
to --summary (e.g. $GITHUB_STEP_SUMMARY) and exits 1 if any check fails.
"""

import json
import sys
from pathlib import Path


def pick(data, select):
    if not select:
        return data
    for item in data:
        if all(item.get(k) == v for k, v in select.items()):
            return item
    return None


def fmt(v):
    if isinstance(v, float) and (abs(v) >= 1e4 or (v != 0 and abs(v) < 1e-2)):
        return f"{v:.3g}"
    return str(v)


def main(argv):
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    checks = json.loads(Path(argv[1]).read_text())
    results = Path(argv[2])
    summary = None
    if "--summary" in argv:
        summary = argv[argv.index("--summary") + 1]
    rows, failed = [], 0
    for c in checks:
        path = results / c["file"]
        status, value = "ok", None
        if not path.exists():
            status = "skipped" if c.get("optional") else "FAIL (no result)"
        else:
            item = pick(json.loads(path.read_text()), c.get("select"))
            value = None if item is None else item.get(c["metric"])
            if value is None:
                status = "FAIL (metric missing)"
            elif "min" in c and value < c["min"]:
                status = "FAIL (below min)"
            elif "max" in c and value > c["max"]:
                status = "FAIL (above max)"
        if status.startswith("FAIL"):
            failed += 1
        bound = " .. ".join(fmt(c[k]) if k in c else "" for k in ("min", "max"))
        rows.append((c["name"], c["metric"], fmt(value) if value is not None else "-", bound, status))
    lines = ["| check | metric | value | bounds (min .. max) | status |", "|---|---|---|---|---|"]
    lines += [f"| {' | '.join(r)} |" for r in rows]
    text = "\n".join(lines)
    print(text)
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write("## wpg bench nightly\n\n" + text + "\n")
    if failed:
        print(f"\n{failed} check(s) failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
