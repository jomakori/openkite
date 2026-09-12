#!/usr/bin/env bash
#
# check-portable-sed.sh — CI portability guard (OKT-103).
#
# macOS runners ship BSD sed, which does not accept GNU's bare `sed -i`: it
# consumes the following argument as the backup suffix, so the expression never
# runs and the step dies with:
#
#   sed: 1: "Cargo.toml\n": invalid command code C
#
# GNU sed (Linux, Git Bash on Windows) and BSD sed (macOS) only agree on the
# attached-suffix form:
#
#   sed -i.bak '<expr>' <file> && rm -f <file>.bak
#
# This guard fails when a workflow uses a bare `-i` or another GNU-only flag.
#
# Usage: check-portable-sed.sh [workflow-dir]   (default: .github/workflows)
set -euo pipefail

dir="${1:-.github/workflows}"
status=0

flag() {
  printf '  %s\n' "$1"
  status=1
}

# Only the content after `file:line:` is inspected; comment-only lines are
# skipped so documentation that names the anti-pattern does not trip the guard.
scan() {
  grep -rnE --include='*.yml' --include='*.yaml' "$1" "$dir" \
    | grep -vE ':[0-9]+:[[:space:]]*#' || true
}

# 1) Bare `sed -i` with no attached backup suffix. Matches `-i `, `-i"`, `-i'`,
#    `-i -e` and a trailing `-i`; an attached suffix (`-i.bak`) passes.
while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  flag "GNU-only bare 'sed -i': $hit"
done < <(scan 'sed[[:space:]]+-i([^[:alnum:].]|$)')

# 2) GNU-only long options — BSD sed has no long options at all.
while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  flag "GNU-only sed long option: $hit"
done < <(scan 'sed[[:space:]].*(--in-place|--regexp-extended)')

if [ "$status" -ne 0 ]; then
  echo
  echo "Rewrite as: sed -i.bak '<expr>' <file> && rm -f <file>.bak"
  echo "GNU sed and BSD sed (macOS) only agree on the attached-suffix form."
  exit 1
fi

echo "check-portable-sed: OK ($dir)"
