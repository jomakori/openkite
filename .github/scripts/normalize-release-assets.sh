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
# that the expected assets exist, so `publish` can never attach a partial set to
# a release (artifact expiry, a cancelled macOS job, a force-push).
#
# Usage: normalize-release-assets.sh <dist-dir> <version> [expected-targets]
#
# expected-targets (optional) is a comma- or space-separated subset of:
#   linux_amd64 linux_arm64 macos_amd64 macos_arm64 windows_amd64 windows_arm64
#
#   - Empty / unset (the default, and the only value a push-triggered run can
#     produce) requires the FULL six-target set. This is the fail-safe gate: a
#     partial release fails right here and nothing is tagged or uploaded.
#   - A non-empty list is the deliberate-subset path. It may only be supplied
#     from an explicit `workflow_dispatch` input on release.yml (see the
#     `targets` input). Only those assets are required, and any other
#     `openkite_*` asset is removed so the release carries exactly the
#     requested targets instead of silently publishing more than asked.
#
# OKT-106 extends this same script into the release fail-safe gate rather than
# adding a second, competing preflight: `Publish release` calls it
# unconditionally, defaulting to the strict six-target set.
set -euo pipefail

dist="${1:?usage: normalize-release-assets.sh <dist-dir> <version> [expected-targets]}"
version="${2:?usage: normalize-release-assets.sh <dist-dir> <version> [expected-targets]}"
expected_input="${3:-}"

all_targets="linux_amd64 linux_arm64 macos_amd64 macos_arm64 windows_amd64 windows_arm64"

# The release asset filename suffix produced for a target token.
suffix_of() {
  case "$1" in
    linux_amd64) printf 'linux_amd64.AppImage' ;;
    linux_arm64) printf 'linux_arm64.AppImage' ;;
    macos_amd64) printf 'macos_amd64.dmg' ;;
    macos_arm64) printf 'macos_arm64.dmg' ;;
    windows_amd64) printf 'windows_amd64.exe' ;;
    windows_arm64) printf 'windows_arm64.exe' ;;
    *) return 1 ;;
  esac
}

# Resolve the expected set. Unknown tokens are a hard error: a typo in a
# deliberate subset must not silently relax the gate.
strict_subset=false
if [ -n "$expected_input" ]; then
  expected="$(printf '%s' "$expected_input" | tr ',' ' ')"
  for target in $expected; do
    case " $all_targets " in
      *" $target "*) ;;
      *)
        echo "::error title=Unknown release target::'$target' is not one of: ${all_targets}" >&2
        exit 1
        ;;
    esac
  done
  strict_subset=true
else
  expected="$all_targets"
fi

# `dist` may be absent when every download step was skipped (e.g. a dry run):
# creating it lets the completeness check report exactly which assets are
# missing instead of dying on `cd`.
mkdir -p "$dist"
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
for target in $expected; do
  want="openkite_${version}_$(suffix_of "$target")"
  [ -f "$want" ] || missing="${missing} ${want}"
done

if [ -n "$missing" ]; then
  echo "::error title=Incomplete release artifact set::missing:${missing}" >&2
  exit 1
fi

# A deliberate subset publishes exactly the requested targets; drop the rest so
# "fewer targets" stays a conscious act rather than an accidental outcome.
if [ "$strict_subset" = true ]; then
  keep=""
  for target in $expected; do
    keep="${keep} openkite_${version}_$(suffix_of "$target")"
  done
  for f in openkite_*; do
    [ -f "$f" ] || continue
    case " ${keep} " in
      *" $f "*) ;;
      *)
        echo "removing unrequested release asset: $f" >&2
        rm -f "$f"
        ;;
    esac
  done
fi

echo "release artifact set complete for v${version}:"
ls -1 openkite_*
