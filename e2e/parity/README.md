# Gate G3 — cross-host parity (computed classes and tokens)

The desktop app and the browser host render the same console from the same
crate, so they must present the same design: the same class contract on the same
nodes, and the same resolved token values. G3 checks exactly that, per core
route, on both hosts.

It does **not** compare pixels. Pixel parity is G2's job (`e2e/visual/baselines`
for the desktop, `e2e/visual/browser/baselines` for the browser), and those
baselines legitimately differ: the hosts run on different data. Comparing
screenshots across hosts would either fail on data or, worse, be tuned until it
passes on nothing.

## What is graded

Per route (`/`, `/cluster`, `/workloads`, `/config`):

| Axis | Detail |
| --- | --- |
| **Tokens** | every custom property the stylesheet declares, resolved on the document element, compared value for value — including properties one host declares and the other does not |
| **Contract nodes** | presence, the class chain, the contract `data-*` attributes, and the computed values of a fixed property set for the nodes named in `crates/openkite-ui/assets/parity-snapshot.js` |
| **Route identity** | `[data-route]` must equal the route the gate asked for (the desktop webview has no URL history to rely on) |
| **Frame** | both hosts must sit in the same breakpoint band (800x600 is the graded frame); equal inner sizes are not required, a window manager may claim a title bar |

Reported for review, never graded:

- class-token inventories of the shell chrome and of the routed view,
- match counts per contract selector — data-dependent, so the desktop
  (disconnected) and the browser host (fixtures) are expected to differ,
- the absolute viewport sizes each host reported.

Normalised before grading — same design, different engine or hydration path:

- hydration bookkeeping (`data-dioxus-id`, `data-node-hydration`, `data-hydration-*`)
  on either host: Dioxus stamps them on hydrated nodes and the wry desktop host
  never serialises them, so they are framework state rather than contract;
- colour channels that differ inside half an 8-bit code: engines round sRGB at
  different depths (`0.646` vs `0.646745` for one declared token), so colours are
  parsed and compared channel-wise — a whole code of movement still fails;
- `auto` on one host against the length the other resolved for the box: Chromium
  reports a content-derived box as `auto` where WebKit resolves a length; any
  other keyword pairing is still a delta;
- `height` on a content-derived box is not a contract property: `.topbar`
  declares `min-height`, so the floor it declares is what parity compares;
- `data-surface` is graded only on the app root; elsewhere it marks host-supplied
  content (the OKT-137 console exists only in the browser host).

A contract node whose *presence* the host decides is marked `host_conditional` in
the extractor: the capability notice a chrome-less host renders, and the route
panel a host with its own surface replaces. A presence difference there is review
material; the node's classes, attributes and styling are still graded whenever
both hosts paint it.

A failing run prints every delta with both hosts' values, and the same content
lands in `parity-report.json`.

## One extractor, two hosts

`crates/openkite-ui/assets/parity-snapshot.js` is the whole measurement. It is:

- embedded in the UI crate as `openkite_ui::parity::SNAPSHOT_JS`, evaluated by
  the desktop hook through `document::eval`;
- read from disk by `e2e/visual/browser/capture.mjs` under `SNAPSHOT_DIR` and
  evaluated in Chromium through `Runtime.evaluate`.

Because both sides evaluate the same file, a change to what "parity" means can
never apply to one host only. Its last expression is a JSON string, which is
what both transports hand back.

The desktop hook is `crates/openkite-desktop/src/parity.rs`: with
`OPENKITE_PARITY_SNAPSHOT=<dir>` set it boots onto `OPENKITE_ROUTE`, waits one
settled frame (1200 ms, `OPENKITE_PARITY_SETTLE_MS`), writes
`<dir>/<slug>.desktop.json` and exits. Unset, it is a single `env::var` and
nothing else — a normal launch is untouched.

## The allowlist

`e2e/parity/allowlist.json` is the only way a deliberate delta can pass, and it
is reviewed like code:

```json
{
  "version": 1,
  "entries": [
    {
      "route": "/workloads",
      "kind": "class",
      "selector": ".tag",
      "key": "tag-muted",
      "why": "the browser fixtures carry a muted capability tag the disconnected desktop has no data for",
      "reviewed_by": "jomakori",
      "ticket": "OKT-192"
    }
  ]
}
```

- `kind` is one of `token`, `class`, `attr`, `style`, `node`.
- `route` is a route path or `"*"`.
- `selector`, `key` and `property` narrow the entry; every field present must
  match, so an entry can be as wide as `"*"` or as narrow as one property.
- `why` (≥ 15 chars) and `reviewed_by` are mandatory. An allowlist entry without
  a reviewer is not reviewed, and the gate rejects the file before comparing.
- An entry that matches no delta is **stale** and fails the run.

The list ships empty: the intended state is parity, and every entry should cost
an argument.

## Running

```bash
# in CI: the `parity` job, after the desktop binary and the wasm bundle exist
e2e/parity/run.sh target/debug/openkite artifacts/parity
```

```bash
# comparator only, against captured snapshots
node e2e/parity/compare.mjs --desktop DIR --web DIR \
  --allowlist e2e/parity/allowlist.json --report report.json
```

```bash
# comparator self-test: no browser, no build (runs in CI as its own job)
node e2e/parity/selftest.mjs
```

The self-test is mutation-based on purpose. It proves that one extra class, one
changed token value, one changed computed property, one missing node, a route
mismatch, a missing snapshot, a band mismatch and a stale or unreviewed
allowlist entry each fail the gate — a comparator that cannot fail proves
nothing.

## Boundaries

- Overlays (`/logs`, `/terminal`, the palette) and desktop-only routes are out
  of scope, same as the visual baselines.
- Only the computed values of the properties named in the extractor are graded.
  A property the design contract does not name is not part of parity yet; add it
  to the extractor's `CONTRACT` when the contract grows.
- The gate grades *rendered* state, not behaviour. Interaction parity is
  `crates/openkite-ui/tests/touch_parity.rs` and the user-flow suites.
