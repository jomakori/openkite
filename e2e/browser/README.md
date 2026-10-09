# Browser E2E harness

This suite exercises OpenKite's shipped `openkite-web` host in a real headless
Chromium process. It opens `/workloads` as a deep-link, inspects the server-rendered
page, waits for the production wasm-bindgen client to hydrate that SSR tree, and
clicks the hydrated Refresh action against `/api/gateway`. A deterministic probe
route is served from a loopback fixture without external network access.

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
