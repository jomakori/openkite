#!/usr/bin/env bash
#
# check-version-source.sh — one resolver for the reported version (OKT-107).
#
# `env!("CARGO_PKG_VERSION")` is a compile-time constant. The workspace sits at
# the 0.0.0 placeholder on main and a released package may be a reused PR
# artifact, so any surface that reads the constant directly reports a version
# that is not the release's. `crates/openkite-desktop/src/version.rs` resolves
# the reported version at runtime; every read outside it is a regression.
#
# Usage: check-version-source.sh [crates-dir]   (default: crates)
set -euo pipefail

dir="${1:-crates}"
resolver="crates/openkite-desktop/src/version.rs"
status=0

while IFS= read -r hit; do
  [ -n "$hit" ] || continue
  case "$hit" in
    "$resolver":*) ;;
    *)
      echo "  version read outside ${resolver}: $hit" >&2
      status=1
      ;;
  esac
done < <(grep -rn --include='*.rs' 'CARGO_PKG_VERSION' "$dir" 2>/dev/null || true)

if [ "$status" -ne 0 ]; then
  echo >&2
  echo "Resolve it through openkite::version::reported() instead." >&2
  exit 1
fi

echo "check-version-source: OK ($dir)"
