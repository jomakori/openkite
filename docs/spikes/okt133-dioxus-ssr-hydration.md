# OKT-133 spike — dioxus 0.7 SSR + hydration proof

> **Verdict: GO (conditional).** Two real OpenKite surfaces render server-side from
> one dioxus 0.7 crate and hydrate in a real browser with **zero** console
> messages, **zero** page errors, and **336/336 server-rendered nodes reused**.
> A hydrated interaction round-trips through a server-side gateway. The
> conditions are payload size and two host-side wiring requirements, both
> recorded below — neither blocks OKT-132/OKT-134.
>
> **Status:** spike complete · **Date:** 2026-09-25 · **Revision of the app under
> port:** openkite `05995b0` · **Spike crate:** `/opt/data/work/okt133-spike`
> (outside the repo; nothing in the repo changed except this document).

## 1. Verdict and the gate answers

| Question the gate asks | Answer | Deciding measurement |
|---|---|---|
| Can the crate render server-side? | **Yes** | `dioxus_ssr::pre_render` over the same `VirtualDom` the desktop crate uses: 2 surfaces, **26,097 B** page, 192 hydration ids, byte-identical across runs |
| Does it hydrate in a browser? | **Yes** | Chromium 153: **0** console messages, **0** page errors, 12/12 table rows + header + 14/14 status pills + refresh button all kept their pre-hydration DOM identity |
| Is it interactive? | **Yes** | Time-to-interactive **32.7 ms** median (nav → hydrated, 5 cold runs, 32.1–51.4 ms) |
| Does one hydrated interaction reach the server without client credentials? | **Yes** | Sort (client-only) reorders the table; Refresh POSTs `/api/gateway`, server counter +1 in 5/5 runs, DOM updates in **47 ms** median |
| What does the migration trade on payload? | **2.33× raw / 3.05× gzip** | wasm+glue 760,748 B raw (283,054 B gzip) vs the React console's 326,013 B raw (92,928 B gzip) |
| Fallback if hydration had failed | **Not needed** | SSR-only page still serves both surfaces complete (12 rows, "Connected", "Overview") with the client bundle absent |

## 2. What was built, and how to reproduce it

Three crates in one workspace, mirroring the shape OKT-132 proposes:

| Path | What it is |
|---|---|
| `ui/` | **The shared UI crate.** Contains the two surfaces and the gateway call. Compiles for native and wasm32 unchanged |
| `host/` | Native server: SSR + hydration payload + `POST /api/gateway` (owns the data) |
| `web/` | wasm client: `VirtualDom` built from the server's snapshot, `launch_virtual_dom(.., Config::hydrate(true))`, then `wasm-bindgen --target web` |

The two surfaces are **real code**, not re-drawn mockups:

- **Pods table** — `ui/src/resource_table.rs` and `ui/src/status_badge.rs` are
  byte-for-byte copies of `src/components/resource_table.rs` (740 lines) and
  `src/components/status_badge.rs`; the ported file's own 11 unit tests run
  unchanged in the spike (11 resource_table + 3 status_badge = 14/14 pass in the
  crate).
  `ui/src/pods.rs` copies `pod_columns()` / `pod_row()` / the status mapping /
  the age humanizer from `src/workloads.rs`; the only edit is the input type
  (k8s-openapi `Pod` → the wire `PodSummary`).
- **Cluster overview** — `ui/src/overview.rs` is a port of `web/src/Overview.tsx`
  (the surface the browser console shows at `/cluster`, which the Rust console
  currently mounts from React); `cluster_health()` and its phase sets are
  copied verbatim from the TSX.

Fixture: 12 pods, fixed names/phases/restarts, name-sorted, deterministic.
`host/tests/ssr.rs` asserts SSR determinism (a mismatch waiting to happen
otherwise), hydration-id presence, both surfaces present, render cost, and that
the SSR-only page has no client bundle.

Reproduce:

```bash
cd /opt/data/work/okt133-spike
cargo test -p okt133-host -- --nocapture                 # SSR side + this doc's SSR numbers
cargo build -p okt133-web --target wasm32-unknown-unknown --release
./bin/wasm-bindgen --target web --no-typescript --out-dir web/dist \
    target/wasm32-unknown-unknown/release/okt133-web.wasm
./bin/wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o web/dist/okt133-web_bg.wasm web/dist/okt133-web_bg.unopt.wasm
OKT133_DIST=$PWD/web/dist OKT133_PORT=8917 ./target/debug/okt133-host &
cd measure && PLAYWRIGHT_BROWSERS_PATH=/opt/data/home/.cache/ms-playwright \
    OKT133_RUNS=5 node measure.mjs            # -> results.json; python3 summarize.py
```

Toolchain: rustc 1.98.0, dioxus/`dioxus-ssr`/`dioxus-web` 0.7.10, wasm-bindgen
0.2.128, binaryen 123 (`wasm-opt -Oz`), Playwright 1.63.0, Chromium 153.0.8010.12,
release profile matching the desktop crate (`opt-level=3`, `lto="thin"`,
`codegen-units=1`, `strip="symbols"`).

## 3. Measurements

**SSR (native, 12-pod fixture, debug-profile host, 1 core).**

| Metric | Value |
|---|---|
| Body bytes (2 surfaces) | 21,734 |
| Full page bytes (hydrate / SSR-only) | 26,097 / 25,612 |
| Hydration ids (`data-node-hydration`) | 192 |
| Hydration payload (`initial_dioxus_hydration_data`) | 4 B base64 (`gA==`) — empty `Vec<Option<Vec<u8>>>`, valid CBOR |
| Render cost | median **2.93 ms**, p95 3.14 ms (n=26) |
| Determinism | two renders of the same snapshot byte-identical |

**Hydration (Chromium 153, headless, localhost, cold; 5 runs).**

| Metric | Value |
|---|---|
| nav → hydrated | median **32.7 ms** (runs 32.1 / 32.7 / 32.7 / 36.5 / 51.4) |
| wasm start → hydrated | median **11.5 ms** |
| nav → wasm start | median 22 ms |
| DOM reuse (`data-ssr` marker set by the page before the module ran) | tagged before **336** → after **336**; data rows **12/12**, header **1/1**, pills **14/14**, refresh button **1/1** |
| Client model vs server markup | `clientPods` (written by the wasm client itself) = **12** = rows rendered by the server |
| Console messages / page errors | **0 / 0** in 5/5 runs |
| React global present | **false** |

**Interactions.**

| Interaction | Result |
|---|---|
| Click "Name" header (client-only) | click 1 → unchanged (fixture already ascending), click 2 → descending order (`web-*`, `web-*`, `redis-0`); listeners are live |
| Row DOM across a reorder | **0/12** rows keep their SSR tag after the reorder — keyed reorder re-creates row nodes (see §5) |
| Click "Refresh" (server round-trip) | `POST /api/gateway` counted server-side in 5/5 runs (`gateway_calls` 0→1, 1→2 … ), probe `data-round` 0→1, server-mutated restart counter visible in the DOM; **47 ms** median round trip |

**Payload, measured on the wire and on disk.**

| Artifact | Raw | gzip | Stage |
|---|---|---|---|
| cargo wasm (release) | 1,762,682 | — | before wasm-bindgen |
| `okt133-web.js` (wasm-bindgen glue) | 64,269 | 10,206 | shipped |
| `okt133-web_bg.wasm` | 775,496 | — | post wasm-bindgen |
| `okt133-web_bg.wasm` | **696,479** | **272,848** | post `wasm-opt -Oz` (shipped) |
| **dioxus total** | **760,748** | **283,054** | |
| React console baseline (`assets/vendored/openkite-react-spike/app.js`) | 277,904 | 84,011 | current |
| React CSS baseline (`app.css`) | 48,109 | 8,917 | current |
| **React total** | **326,013** | **92,928** | |

So the wasm path is **2.33× the React payload raw and 3.05× gzipped**. No
wasm-split, no per-route chunking, no `-O4`, no brotli in these numbers.

**Negative control (proves the reuse metric can fail).** The host can embed a
snapshot the SSR pass did not render (`/?skew=1`: one pod renamed/re-phased, one
extra pod). Result: `clientPods` **13** vs **12** rendered rows, **0** console
messages, **0** hydration warnings — dioxus 0.7 hydration does **not** validate
the markup — and the first click then dies with
`TypeError: Cannot read properties of undefined (reading 'setAttribute')`, the
refresh never reaching the server (`data-round` stayed 0). The positive runs are
therefore a real signal, not a tautology: matching inputs give 336/336 reuse and
0 errors; mismatched inputs give an inconsistent model and a hard failure on
first interaction.

## 4. What this proves — and what stays unmeasured

**Proven.** (a) The crate renders the two surfaces server-side with hydration ids
and deterministic output; (b) the browser hydrates that markup with dioxus 0.7.10
and reuses it node for node, with no console noise; (c) the hydrated tree is live
for both local state and a server round-trip; (d) the client bundle holds no
credentials and has exactly one network path (`POST /api/gateway` — the wasm
contains no `kubeconfig` / `BEGIN CERTIFICATE` / `Bearer ` / `rustls` strings, and
`cargo tree -p okt133-web --depth 1` is dioxus, dioxus-web, the UI crate,
serde_json).

**Not measured (do not quote these as done).**

- **Not a live cluster.** The gateway serves a fixture. Every number is
  transport/rendering; real kube-rs wiring in the axum host is untouched.
- **No React TTI baseline.** The 278 KB JS was measured as bytes, not as
  time-to-interactive on the same page; comparing the two TTIs needs the React
  console served next to this one.
- **One engine, one host.** Chromium only (no WebKitGTK/wry, no Firefox), and
  SSR cost is from a debug-profile host.
- **12 rows, no virtualization stress.** The pods table's virtualized window
  (ROW_HEIGHT 36, ~17 visible + overscan) was never pushed past the viewport, so
  scroll-driven re-render and DOM reuse under windowing are unmeasured.
- **No gzip/brotli on the wire** (the spike server does not compress); the gzip
  column is computed offline.
- **No suspense/streaming resources.** The hydration payload is the empty
  vector; `#[server]`-style data functions, streaming and suspense hydration are
  untested, so nothing is claimed about dioxus-fullstack.
- **Cosmetic CSS only.** `host/static/app.css` mirrors class names; no design
  tokens, themes (OPALINE), or a11y audit.

## 5. Gotchas the migration inherits

1. **The SSR host must emit two globals itself.** `dioxus_web`'s hydrate path
   reads `window.initial_dioxus_hydration_data` (base64 CBOR — `gA==` is fine for
   an empty context) and the wasm-bindgen JS snippet indexes
   `window.hydrate_queue` unconditionally. Missing `hydrate_queue` is not a
   warning: the first hydrate call throws
   `Cannot read properties of undefined (reading 'length')` from
   `register_rehydrate_chunk_for_streaming_debug`, i.e. hydration never starts.
   `dx` generates both for you; a hand-written axum host (the OKT-118 shape) must
   emit them. Observed in this spike, then fixed by adding
   `window.hydrate_queue = []` + the hydration-data script.
2. **`wasm-bindgen --target web` glue needs an `import`/`init()` step.** A plain
   `<script type="module" src=...>` loads the glue and silently never
   instantiates the module — no request for `*_bg.wasm`, no error. The host must
   boot it explicitly (`import init from "…"; init();`), which is what `dx`
   writes for you. Pin the CLI to the exact `wasm-bindgen` version cargo resolves
   (0.2.128 here; the repo lock is on 0.2.127).
3. **Hydration is not self-validating.** A server/client data disagreement
   produces no warning; it shows up as a model/DOM inconsistency and then a
   panic on the first interaction (§3, negative control). The migration needs a
   single snapshot source on both sides and a CI assertion that the client model
   equals the server markup — the per-surface measurement this spike used
   (`data-ssr` survival + client-published counts) is the cheap version of that
   gate.
4. **Reordering a hydrated table re-creates row nodes.** After the sort click
   **0/12** rows kept their pre-hydration identity although the header, the
   overview and the buttons did. Cosmetic risk (scroll position, focus, transient
   flicker) rather than a correctness failure, but it is a regression risk
   against the current React table and should be asserted in the migration's
   parity tests.
5. **Payload discipline is a design constraint, not a detail.** 696 KB of wasm
   for two surfaces with no chunking is 3× the React bundle gzipped. Before
   OKT-132 lands, decide the budget (route-level wasm split, `wasm-opt` flags,
   brotli) — an unbounded single-chunk wasm host is the most likely way this
   migration degrades the browser console.

## 6. Recommendation

Proceed with the shared-UI-crate plan (OKT-132/OKT-134), with these conditions:

1. **Ship the crate as a lib with no host assumptions**, exactly as `ui/` here:
   surfaces + the gateway client behind one same-origin call; the server keeps
   credentials.
2. **Copy the host-side plumbing once** (`hydrate_queue`, hydration payload,
   `init()` boot, hydration-data script) into the axum host, and pin the
   `wasm-bindgen` CLI to the locked version.
3. **Make SSR-vs-client data equality a CI gate** (client-published counts vs
   rendered markup, per surface); do not rely on hydration warnings — there
   aren't any.
4. **Set a payload budget before the port starts** and re-measure with the React
   console on the same page so TTI is comparable, not just bytes.
5. **Keep the desktop target first-class**: `ui/` compiles unchanged for native;
   that is the point of the crate and it is already true in this spike.

**Fallback cost if hydration is ever abandoned** (not needed today): the
SSR-only page is 25,612 B, fully rendered (12 rows, health, counts) with the
client bundle absent — so "SSR + a thin fetch API" would cost every interaction
a round trip (47 ms locally) plus a full-surface re-render, and would give up the
client-side interactivity (sort, filter, virtualized scroll) that the React
console has today.
