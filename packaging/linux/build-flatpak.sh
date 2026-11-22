#!/usr/bin/env bash
# Builds a Flatpak bundle from the release binary already in target/release.
set -euo pipefail

arch="${1:?usage: build-flatpak.sh <x86_64|aarch64> <version>}"
version="${2:?usage: build-flatpak.sh <x86_64|aarch64> <version>}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cp "$repo_root/target/release/openkite-desktop" "$work/openkite-desktop"
cp "$here/io.github.jomakori.OpenKite.desktop" "$work/"
cp "$here/io.github.jomakori.OpenKite.svg" "$work/"

flatpak-builder --arch="$arch" --force-clean --repo="$work/repo" \
  "$work/build" "$here/io.github.jomakori.OpenKite.yml"
flatpak build-bundle "$work/repo" \
  "$repo_root/dist/openkite_${version}_linux_${arch}.flatpak" \
  io.github.jomakori.OpenKite
