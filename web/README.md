# Web UI (`web/`)

The React 19 + Tailwind 4 UI layer has **two build targets from one source
tree**, selected by Vite `--mode` (`web/vite.config.ts`):

| Target | Command | Output | Consumer |
|---|---|---|---|
| Desktop | `npm run build` | `assets/vendored/openkite-react-spike/{app.js,app.css}` | Rust host `include_str!`s it (`src/react_spike.rs`) |
| Browser / staging | `npm run build:web` (wrapped by `web/build.sh`) | `web/dist/index.html` + hashed assets | The console image, served by the Rust host |

Only the output shape differs — plugins, `define`, and sources are shared, so
both targets render the same UI.

## Desktop (vendored) target

Vite library mode bundles the UI as a fixed-name IIFE + CSS pair. The Rust host
`include_str!`s both files at compile time, so the committed bundle is a
**source input**, not a release artifact. It uses fixed filenames with no
hash/manifest — the host does a plain `include_str!`, matching the repo's
existing vendoring convention (`tools/build-xterm`,
`assets/vendored/codemirror/`).

**Vendor step — run after ANY change under `web/src/`:**

```sh
cd web
npm ci
npm run typecheck    # TypeScript gate
npm run build        # writes assets/vendored/openkite-react-spike/{app.js,app.css}
npm run bundle-size  # refreshes assets/vendored/openkite-react-spike/SOURCE.txt
```

Commit the regenerated `app.js`, `app.css`, and `SOURCE.txt` together with the
source change. A bundle that has drifted from `web/src/` still compiles (the
Rust build only sees the committed file), so re-running the step is what keeps
the two in sync. `npm run dev` serves the same source for browser iteration.

## Browser / staging target

`web/build.sh` installs dependencies when needed, runs `npm run build:web`, and
requires `web/dist/index.html` to exist. `build-image.yml` runs it (for
`pr-image.yml` and `release.yml` alike), then stages `web/dist/` into the image
and fails loudly if `index.html` is absent — an image is never published with no
UI behind it.

### Fixture data path

The desktop host answers `window.openkite.api.*` through the Rust asset
handler. A static host has no such handler, so `web/src/bridge.ts` falls back
to the kube-shaped fixtures in `web/src/fixtures.ts` when the host transport is
missing. The UI then renders with content instead of empty states. Desktop
behaviour is unchanged whenever `window.openkite` is present.

In the container image `/openkite` **is** answered (by `crates/openkite-web`), so
the fixture path does not apply there — what the console shows depends on what the
pod's ServiceAccount is allowed to read, and an access-denied bridge call is an
`error` envelope, not a fixture. Fixtures remain the behaviour of a purely static
host, such as `python3 -m http.server` over `web/dist/`.

`npm run test:parity` builds `web/dist/` and server-renders the shared React
tree with fixture data, asserting the shell + table DOM and that both bundles
carry the same UI markers. It runs in CI's `bundle-freshness` job. No browser
binary is required.

## Container

`Dockerfile` is multi-stage:

1. `bundle` — stages `web/dist/` and fails loudly if `index.html` is absent.
2. `runtime` — `debian:trixie-slim` running the release-built `openkite-web`
   binary, with `web/dist/` beside it at `/app/web/dist`.

The runtime is the Rust host (`crates/openkite-web`), not nginx: one image for
previews and releases, so a preview exercises what prod will run.

- **`OPENKITE_WEB_ROOT=/app/web/dist`** is the only address the image pins.
  `OPENKITE_ADDR` is left alone, so the host's own default (`0.0.0.0:8080`) stays
  the single source of truth — and 8080 is also the umbrella chart's
  `service.targetPort`, so the Service keeps resolving.
- **`USER 65532:65532`** — a numeric non-root uid with no `/etc/passwd` entry.
  The chart sets no `securityContext` for this app, so the image's uid is what
  the pod runs as. `/home/openkite` exists and is writable because
  `OpenKiteConfig::path()` resolves `.openkite/config.toml` from `HOME`.
- **The bundle is asserted twice in the image** — Stage 1's `test -f` over the
  staged bundle, and a second `RUN test -f /app/web/dist/index.html` after the
  `COPY` into the runtime — plus a third time in the binary:
  `crates/openkite-web` refuses to boot when `OPENKITE_WEB_ROOT` holds no
  `index.html`, so a console-less container CrashLoops instead of answering 404
  for every path and letting the console fall back to fixtures.
- **The binary is compiled on the runner, not inside the Dockerfile**, so
  `Swatinem/rust-cache` can make the release build affordable on the workflow
  that runs for every PR. `.dockerignore` re-includes exactly
  `target/release/openkite-web` and keeps multi-GB `target/` out of the context.
- **Base vs builder:** the runner is `ubuntu-24.04` (glibc 2.39) and the base is
  `debian:trixie-slim` (glibc 2.41). That direction is safe; dropping the base
  below the runner is a boot failure. `build-image.yml`'s `Smoke-test the image`
  step is what proves it, before anything is pushed.

## Local check

Serving the bundle statically is still the quickest way to look at the UI — it is
just not what the image does:

```sh
./web/build.sh                      # produces web/dist/index.html
python3 -m http.server -d web/dist  # serve it at http://127.0.0.1:8000
```

`docker run --rm -p 8080:8080 openkite-web` no longer boots the image: the
runtime is the host, and the host exits when it cannot reach a cluster
(`Client::try_default`). Hand it a stub kubeconfig, which `try_default` parses
without ever touching the network:

```sh
cargo build --release -p openkite-web   # the binary the runtime stage COPYs
docker build -t openkite-web .
printf '%s' '{"apiVersion":"v1","kind":"Config","clusters":[{"name":"stub","cluster":{"server":"https://127.0.0.1:1"}}],"contexts":[{"name":"stub","context":{"cluster":"stub","user":"stub"}}],"current-context":"stub","users":[{"name":"stub","user":{}}]}' > /tmp/stub-kubeconfig
docker run --rm -p 8080:8080 -e KUBECONFIG=/stub/kubeconfig \
  -v /tmp/stub-kubeconfig:/stub/kubeconfig:ro openkite-web
curl -fsS http://127.0.0.1:8080/ | head -1     # the console shell
```

The real check is CI: `build-image.yml`'s `Smoke-test the image` step runs the
built image before the push and asserts the served bundle bytes, the SPA
fallback, the `/openkite` envelope, the uid, and the boot log's bundle root.

Failing builds are expected to be loud:

```sh
docker build .        # -> Stage 1: "web/dist/index.html is missing"
                      # -> Stage 2: "target/release/openkite-web: not found"
                      #    until `cargo build --release -p openkite-web` has run
```
