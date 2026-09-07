#!/usr/bin/env python3
"""Append a markdown test summary to the GitHub Actions step summary.

Reads `cargo test` output (tee'd to a log) and writes aggregate pass/fail
counts plus the failing test names (when any) to $GITHUB_STEP_SUMMARY.

Usage: test-summary.py <cargo-test-log>
"""
import os
import re
import sys

log_path = sys.argv[1]
summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
if not summary_path:
    sys.exit("GITHUB_STEP_SUMMARY not set (not running under GHA?)")

text = open(log_path, errors="replace").read()

# Aggregate per-binary `test result:` lines.
results = re.findall(
    r"test result: (\w+)\.\s*(\d+) passed;\s*(\d+) failed;\s*(\d+) ignored;",
    text,
)
total_passed = sum(int(p) for _, p, f, i in results)
total_failed = sum(int(f) for _, p, f, i in results)
total_ignored = sum(int(i) for _, p, f, i in results)

# Cargo prints failing test names in a `failures:` block before the
# `test result:` line of that binary. Capture any `name` lines under it.
failures = re.findall(r"failures:\n((?:    [^\n]+\n)+)", text)
failed_names = []
for block in failures:
    failed_names += [
        line.strip()
        for line in block.splitlines()
        if line.strip() and not line.strip().startswith("----")
    ]

lines = []
lines.append("## Test results")
lines.append("")
lines.append("| Outcome | Count |")
lines.append("|---|---|")
lines.append(f"| Passed | {total_passed} |")
lines.append(f"| Failed | {total_failed} |")
lines.append(f"| Ignored | {total_ignored} |")
lines.append("")
verdict = "PASS" if total_failed == 0 else "FAIL"
lines.append(f"**Verdict: {verdict}**")
if failed_names:
    lines.append("")
    lines.append("Failing tests:")
    for name in failed_names[:25]:
        lines.append(f"- `{name}`")
    if len(failed_names) > 25:
        lines.append(f"- … and {len(failed_names) - 25} more")

with open(summary_path, "a") as fh:
    fh.write("\n".join(lines) + "\n")

print(f"test summary: {total_passed} passed, {total_failed} failed, {total_ignored} ignored")
sys.exit(0)
