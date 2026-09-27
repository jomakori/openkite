#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="${OPENKITE_BUNDLE_OUT:-$here/out/bundle}"

"$root/web/build.sh"

if [ ! -f "$root/web/dist/index.html" ]; then
  echo "build-bundle: ERROR: web/build.sh finished but web/dist/index.html is absent." >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out"
cp -a "$root/web/dist/." "$out/"

files="$(find "$out" -type f | wc -l | tr -d ' ')"
echo "build-bundle: staged $files file(s) into $out"
echo "build-bundle: the Tiltfile syncs tilt/out/bundle -> ${OPENKITE_SYNC_ROOT:-/usr/share/nginx/html} on the running preview pod"

if [ -n "${OPENKITE_BASE_TAG:-}" ] && [ -d "$root/crates" ]; then
  changed="$(git -C "$root" diff --name-only "${OPENKITE_BASE_TAG}..HEAD" -- crates 2>/dev/null || true)"
  if [ -n "$changed" ]; then
    echo "build-bundle: NOTE: this branch also changes Rust under crates/ (${OPENKITE_BASE_TAG}..HEAD):" >&2
    printf 'build-bundle:   %s\n' $changed >&2
    echo "build-bundle: the bundle overlay does not carry compiled behaviour; if the diff reaches the host that runs in the container, compile-and-swap instead — tilt/build-server.sh, tilt/README.md." >&2
  fi
fi
