# Browser E2E harness

This suite exercises OpenKite's shipped `openkite-web` host in a real headless
Chromium process. It checks the SSR document first, then waits for the production
wasm-bindgen client to hydrate the exact SSR app root, and clicks the hydrated
Refresh action against the host's `/api/gateway` route. A deterministic local
kubeconfig points the host at a loopback endpoint so cluster state and
credentials are not prerequisites; no Kubernetes API request is expected.

The runner invokes Chrome's DevTools Protocol directly over Node 22+'s built-in
WebSocket. It does not install browser drivers, packages, or Playwright. Supply
an already provisioned Chromium/Chrome binary and a previously built web host
and hydration assets:

```bash
OPENKITE_WEB_BIN=/path/to/openkite-web \
OPENKITE_WEB_ROOT=/path/to/hydration-assets \
OPENKITE_BROWSER=/path/to/chromium \
./e2e/browser/run.sh
```

`OPENKITE_WEB_ROOT` must contain `openkite-web-client.js` and
`openkite-web-client_bg.wasm`, matching the production Dockerfile build recipe.
The script retains `report.json` under a temporary artifact directory on
failure; set `OPENKITE_ARTIFACT_DIR` to select a stable output directory.

The harness intentionally does not build Rust or fetch/install dependencies.
Run it on a CI runner after the regular native `openkite-web` and wasm hydrate
builds complete; provision Chromium separately there. This suite is not wired
into a workflow yet.
