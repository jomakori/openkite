#!/usr/bin/env bash
#
# Build the browser-runnable staging bundle into web/dist/.
#
# This is the target the PR-preview pipeline runs
# (.github/workflows/pr-image.yml) and that the root Dockerfile serves with
# nginx. It emits a standard Vite app — dist/index.html plus hashed assets —
# unlike `npm run build`, which emits the fixed-name vendored bundle the Rust
# desktop host include_str!s. Both targets build the same web/src tree; only
# the output shape differs (see web/vite.config.ts).
#
# The script must produce dist/index.html: the workflow and the Dockerfile
# both refuse to publish an image without it.
set -euo pipefail

cd "$(dirname "$0")"

# The workflow checks out a clean tree, so node_modules is absent there. A
# local run may already have it; skip the reinstall when it does.
if [ ! -d node_modules ]; then
  npm ci
fi

npm run build:web

if [ ! -f dist/index.html ]; then
  echo "web/build.sh: ERROR: dist/index.html was not produced" >&2
  exit 1
fi

assets=0
if [ -d dist/assets ]; then
  assets=$(find dist/assets -type f | wc -l | tr -d ' ')
fi
echo "web/build.sh: wrote dist/index.html and ${assets} asset(s)"
