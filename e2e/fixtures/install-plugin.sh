#!/usr/bin/env bash
# Install the G3 parity fixture bundle into a host's plugin root.
#
#   install-plugin.sh <plugins-root> [desktop|web]
#
# <plugins-root> is the directory the bundle is copied into; the caller owns
# where that root is, because the two hosts disagree:
#
#   desktop  ~/.openkite/plugins
#            crates/openkite-desktop/src/plugin_js.rs:106
#              pub fn plugins_dir() -> PathBuf { dirs::home_dir()…join(".openkite").join("plugins") }
#            The desktop host scans that root at boot
#            (plugin_js.rs:170 `discover_plugins` → `collect_bundles`), so the
#            bundle is live after one boot: no extra wiring.
#
#   web      $OPENKITE_WEB_ROOT/plugins
#            The browser host serves its root as static files and has no bundle
#            loader on main — crates/openkite-web/src/host.rs:112 publishes
#            `Capabilities::server_side()` (plugins: false), and nothing under
#            crates/openkite-web reads a plugins directory. Installing here
#            stages the bundle at /plugins/parity-fixture/… so a capture (or the
#            loader that lands next) can fetch it; it is NOT evaluated today.
#
# Idempotent: the destination is replaced, so a stale bundle cannot survive an
# install.
set -euo pipefail

usage() {
  echo "usage: $(basename "$0") <plugins-root> [desktop|web]" >&2
  exit 64
}

[ "$#" -ge 1 ] || usage
[ "$#" -le 2 ] || usage

target=$1
host=${2:-desktop}
case "$host" in
  desktop | web) ;;
  *)
    echo "$(basename "$0"): unknown host '$host' (expected desktop or web)" >&2
    exit 64
    ;;
esac

fixtures=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source_dir="$fixtures/plugins/parity-fixture"
[ -f "$source_dir/manifest.json" ] || {
  echo "$(basename "$0"): no fixture bundle at $source_dir" >&2
  exit 1
}

mkdir -p -- "$target"
dest="$target/parity-fixture"
rm -rf -- "$dest"
cp -R -- "$source_dir" "$dest"

for file in manifest.json main.js style.css; do
  [ -f "$dest/$file" ] || {
    echo "$(basename "$0"): $file missing from the installed bundle" >&2
    exit 1
  }
done

echo "installed parity-fixture bundle: $dest"
if [ "$host" = web ]; then
  echo "note: openkite-web has no bundle loader on main — this bundle is staged," >&2
  echo "      not loaded; the host still advertises plugins: false." >&2
fi
