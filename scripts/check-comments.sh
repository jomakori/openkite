#!/usr/bin/env bash
# Fails when a comment carries a ticket or PR id — the PR body owns that link.
set -euo pipefail

pattern='^[[:space:]]*(//|///|//!|/\*|\*|#)[^[:alnum:]]*.*(\b(OKT|OKA|PLY|JIRA)-[0-9]{1,5}\b|\b(PR|GH|issue|closes|fixes) ?#[0-9]{1,5})'

if hits=$(grep -nHE "$pattern" "$@"); then
  printf '%s\n' "$hits" >&2
  echo "comment: ticket/PR ids belong in the PR body, not in source" >&2
  exit 1
fi
