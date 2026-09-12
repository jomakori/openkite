#!/usr/bin/env bash
#
# normalize-release-assets.sh — rename downloaded build artifacts to the release
# asset convention and refuse to publish a partial set.
#
# Version-agnostic PR artifacts are named
#   openkite_<token>_<os>_<arch>.<ext>
# where <token> is the version the build workflow embedded (the analyzed version
# on the release FALLBACK path, or the 0.0.0 placeholder for PR builds). This
# rewrites whatever token is present to the release version and then asserts
# that all six expected assets exist, so `publish` can never attach a partial
# set to a release (artifact expiry, a cancelled macOS job, a force-push).
#
# Usage: normalize-release-assets.sh <dist-dir> <version>
#
# NOTE: this is the artifact-set completeness check. The broader release
# fail-safe gate is owned by OKT-106; this script deliberately does not try to
# reproduce it.
set -euo pipefail

dist="${1:?usage: normalize-release-assets.sh <dist-dir> <version>}"
version="${2:?usage: normalize-release-assets.sh <dist-dir> <version>}"

cd "$dist"

# Rewrite the version token in place. `sed -E` (not `sed -i`) keeps this
# portable across GNU and BSD sed without needing a backup suffix.
for f in openkite_*; do
  [ -f "$f" ] || continue
  renamed="$(printf '%s' "$f" \
    | sed -E "s/^openkite_[^_]+_(linux|macos|windows)_(amd64|arm64)\./openkite_${version}_\1_\2./")"
  [ "$renamed" = "$f" ] || mv -f "$f" "$renamed"
done

missing=""
for want in linux_amd64.AppImage linux_arm64.AppImage \
  macos_arm64.dmg macos_amd64.dmg windows_amd64.exe windows_arm64.exe; do
  [ -f "openkite_${version}_${want}" ] || missing="${missing} openkite_${version}_${want}"
done

if [ -n "$missing" ]; then
  echo "::error title=Incomplete release artifact set::missing:${missing}" >&2
  exit 1
fi

echo "release artifact set complete for v${version}:"
ls -1 openkite_*
