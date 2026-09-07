#!/usr/bin/env python3
"""Append a coverage-gate summary to the GitHub Actions step summary.

Reads the cobertura.xml that cargo-tarpaulin wrote and appends the line
coverage percentage + gate verdict to $GITHUB_STEP_SUMMARY. Exit code is
the tarpaulin gate verdict so a failing run shows red even though this
script itself always reports.

Usage: coverage-summary.py <cobertura.xml> <fail-under-percent>
"""
import os
import sys
import xml.etree.ElementTree as ET

xml_path = sys.argv[1]
fail_under = float(sys.argv[2])
summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
if not summary_path:
    sys.exit("GITHUB_STEP_SUMMARY not set (not running under GHA?)")

root = ET.parse(xml_path).getroot()
line_rate = float(root.get("line-rate", "0"))
covered = int(float(root.get("lines-covered", "0")))
valid = int(float(root.get("lines-valid", "0")))
pct = line_rate * 100.0
passed = pct >= fail_under

lines = []
lines.append("## Coverage gate")
lines.append("")
lines.append(f"- **Line coverage: {pct:.1f}%** (gate ≥ {fail_under:.0f}%)")
lines.append(f"- Lines: {covered}/{valid}")
lines.append(f"- **Verdict: {'PASS' if passed else 'FAIL'}**")
if not passed:
    lines.append("")
    lines.append(
        f"Coverage {pct:.1f}% is below the {fail_under:.0f}% gate — add tests "
        "for the uncovered paths before merging (Phase 2.5, OKT-54)."
    )

with open(summary_path, "a") as fh:
    fh.write("\n".join(lines) + "\n")

print(f"coverage: {pct:.1f}% ({covered}/{valid} lines) — {'PASS' if passed else 'FAIL'}")
sys.exit(0)
