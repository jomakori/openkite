# Web UI (`web/`)

The React 19 + Tailwind 4 UI layer has **two build targets from one source
tree**, selected by Vite `--mode` (`web/vite.config.ts`):

| Target | Command | Output | Consumer |
|---|---|---|---|
| Desktop | `npm run build` | `assets/vendored/openkite-react-spike/{app.js,app.css}` | Rust host `include_str!`s it (`src/react_spike.rs`) |
| Browser / staging | `npm run build:web` (wrapped by `web/build.sh`) | `web/dist/index.html` + hashed assets | PR-preview image (nginx) |

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
requires `web/dist/index.html` to exist. The PR-preview workflow
(`.github/workflows/pr-image.yml`) runs it, then the root `Dockerfile` stages
`web/dist/` and fails loudly if `index.html` is absent — an image is never
published with no UI behind it.

### Fixture data path

The desktop host answers `window.openkite.api.*` through the Rust asset
handler. A static host has no such handler, so `web/src/bridge.ts` falls back
to the kube-shaped fixtures in `web/src/fixtures.ts` when the host transport is
missing. The UI then renders with content instead of empty states. Desktop
behaviour is unchanged whenever `window.openkite` is present.

`npm run test:parity` builds `web/dist/` and server-renders the shared React
tree with fixture data, asserting the shell + table DOM and that both bundles
carry the same UI markers. It runs in CI's `bundle-freshness` job. No browser
binary is required.

## Container

`Dockerfile` is multi-stage:

1. `bundle` — stages `web/dist/` and fails loudly if `index.html` is absent.
2. `runtime` — `nginxinc/nginx-unprivileged:1.31-alpine` serving the bundle on
   port **8080** (non-root, uid 101), with an SPA fallback (`web/nginx.conf`) so
   client-side routes survive a hard refresh. 8080 matches the
   `openkite-preview` chart's `service.targetPort`, so the image and chart stay
   in agreement.

## Local check

```sh
./web/build.sh                      # produces web/dist/index.html
python3 -m http.server -d web/dist  # serve it at http://127.0.0.1:8000
```

Without the bundle, the image build is expected to fail:

```sh
docker build .        # -> "web/dist/index.html is missing"
```

With a bundle in place:

```sh
docker build -t openkite-web .
docker run --rm -p 8080:8080 openkite-web
```
