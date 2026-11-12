# Architecture

The shape of the v2 workspace: what the engine is responsible for, what crosses
the wire, and where the boundary lines are. Behaviour that is specified but not
yet built is named in the last section rather than described here in the present
tense.

## 1. What this is

OpenKite is a Kubernetes IDE at Lens parity: every object kind, every verb,
exec and port-forward and logs, staying fluid on clusters where a
single deployment rollout produces thousands of watch events per second. Multi-cluster
is the one parity item deliberately deferred, and §6 says why. It is
one Rust workspace — a single `cargo` tree of six crates — built around one
collection engine and one Dioxus console. That console is rendered twice: by a
desktop shell that hosts it in a native webview over an in-process channel, and
by a browser shell that server-renders it and streams the same frames over a
WebSocket. The engine's defining decision is what it refuses to watch: collection
is a budgeted, demand-driven subscription problem, not a poll loop over every
kind on the cluster.

Three acceptance statements outrank everything else here, in this order: parity
(every surface renders from live cluster data, every Lens-class verb works),
scale (5,000 pods / 40,000 objects / a 5,000-event burst still leaves first paint
interactive, event-to-pixel under 300 ms, RSS under 600 MB desktop and 250 MB per
web session), and restraint (nothing is watched that no visible surface needs,
and a debug panel says so instead of degrading silently).

## 2. Crate map

| Crate | Owns |
| --- | --- |
| `openkite-engine` | Collection: interest registry, informer supervision, row projections and the full-object LRU, coalescing, delta encoding, discovery and catalogue cache, cluster transport. |
| `openkite-api` | The wire contract: delta frames, row and command types shared by the engine and every shell. Serialization-only dependencies. |
| `openkite-ui` | The console: Dioxus surfaces, rendered by either shell, with `web` / `desktop` crate features selecting the renderer. |
| `openkite-desktop` | The native shell: webview window, engine over an in-process channel. |
| `openkite-web` | The browser shell: axum server, server-rendered console, WebSocket delta stream. |
| `openkite-cli` | The binary entry point for scripted and headless use of the engine. |

Data flows one way, engine → api → ui → shell. Shells are thin: they own a
window, a session and a transport, and nothing about Kubernetes. The UI owns
presentation and interests; it never talks to an API server. The engine owns
every cluster call and knows nothing about presentation.

The boundary rule is mechanical, not stylistic:

- `openkite-engine` must never depend on `dioxus` or `openkite-ui`.
- `openkite-ui` depends only on `openkite-api` and `dioxus`.
- Shells depend on `openkite-ui`, and each other's crates not at all.

CI enforces the first clause: the `Guard the crate boundary` job in
`.github/workflows/lint-test.yml` fails when `cargo tree -p openkite-engine`
lists either crate. The rule exists because the engine runs headless in the CLI
and in tests on machines with no webview stack, and because a typed wire contract
is the hedge that keeps the UI framework decision reversible at exactly one crate
boundary.

## 3. Collection model

Nothing watches without an owner. Every watch is created by an
`Interest { gvk, scope, selector, class }` carrying a refcount, so two views on
one kind share one informer. When the last reference drops the informer is
parked after a grace period rather than aborted immediately — parking through
transitions is what keeps view switching from thrashing the API server.

Four interest classes, each with its own watch mode, cadence and park delay:

| Class | Example | Watch mode | Coalesce | Park after |
| --- | --- | --- | --- | --- |
| `Pinned` | namespaces, nodes, active cluster identity | metadata-only, cluster-scoped | 1 s | never |
| `Visible` | the kind in the current view | full objects if the columns need them, otherwise metadata | 120 ms | 5 s |
| `Adjacent` | owner lookups, the pod list under a deployment, events for the inspected object | field-selector-scoped | 1 s | 30 s |
| `Probe` | catalogue kinds, count badges | metadata-only count estimate (`limit=1` plus `remainingItemCount`) | 30 s poll | 5 min |

The store holds projections, not objects. A list row is a compact struct — UID,
name, namespace, kind, created, labels hash, 6–12 status scalars,
resourceVersion — and full objects live in a bounded LRU (300 objects / 64 MB)
that the inspector reads on demand. A pod row is roughly 300 B against 10–30 KB
for the object, which is the single largest memory and CPU win in this design:
5,000 pods cost about 1.5 MB of rows instead of about 60 MB of live objects.

Freeze is a first-class verb, not an absence of code. Every informer can be
paused, resumed, or frozen (parked with its snapshot kept), driven by window
visibility, route and the governor. The governor bounds the whole thing per
cluster: 16 concurrent watches, client-side 20 QPS with a burst of 40, 20,000
rows per kind, the LRU above. At a limit the governor parks the lowest-class
informer and says so in the collection-budget panel; it never silently degrades,
and it never grows an unbounded queue.

The framing matters more than any individual number: collection is a budgeted,
demand-driven subscription problem. Consequently the app does not watch every
kind at boot, does not watch Secrets cluster-wide, does not hold an unbounded
event or log buffer, does not poll metrics for invisible rows, and does not open
an exec channel that no visible terminal owns.

## 4. Transport

The wire frame is typed, not a snapshot blob:

```
{ seq, generated_at, reset: bool, upsert: [Row], delete: [uid], counts, health }
```

A full `reset` is emitted only on relist or resync. Everything else is a delta,
because a deployment rollout that touches 500 pods produces roughly 5,000 watch
events in 20 seconds — about 250 per second — which the old design turned into
5,000 snapshot serializations plus a clone per subscriber.

Delta frames are coalesced per subscription class: a dirty-UID set accumulates
and one frame leaves per window, 120 ms for `Visible` and 1 s for the background
classes, adapting to 500 ms / 5 s when the frame rate would exceed 20 per second.
The same 5,000-event burst therefore costs roughly 160 frames instead of 5,000.

Backpressure is bounded and lossless in effect. Watcher to coalescer is a bounded
channel; on overflow the engine sets `dirty_all`, drops per-object detail, and
builds the next frame as a `reset` from the store — idempotent, still correct,
never an unbounded queue.

Rows are projections under 1 KB, and the projection is where privacy and size are
decided. `metadata.managedFields`, `kubectl.kubernetes.io/last-applied-configuration`
and other large annotations are stripped before anything reaches the store. Secret
`data` and `stringData` never leave the engine at all: they are fetched only when
a user explicitly reveals them, so no Secret value can appear in a row, a frame,
or a log line.

List and watch follow the API's own contract: initial list with `limit=500` plus
`continue` paging, then a watch from the returned `resourceVersion` with
`allowWatchBookmarks=true` and `timeoutSeconds=300`; `410 Gone` triggers exactly
one full relist and one `reset` frame; each informer resyncs every 10 minutes
±20 % jitter. Metadata-only lists and watches (`PartialObjectMetadata`) are the
default for tables and counts, which is 10–50× fewer bytes than full objects.

There is one wire contract and no second fetch path. Desktop receives the frames
over an in-process channel, the browser over a WebSocket; both see the same
`Delta` type from `openkite-api`. That is the structural reason desktop and web
behaviour cannot drift.

## 5. Discovery and catalogue

The catalogue is a function of discovery, never a hand-written table. A cluster
returns hundreds of resource types, and the long tail is exactly the part a
hand-maintained list loses.

Aggregated discovery is the only discovery path: `GET /api` and `/apis` with
`Accept: application/json;v=v2;g=apidiscovery.k8s.io;as=APIGroupDiscoveryList`,
merged into one resource map and cached at connect plus every 10 minutes. The
legacy `/api/v1` + per-group `/apis/<group>/<version>` fan-out exists only behind
a feature flag as a fallback, so the normal case is one or two requests instead
of 1+N.

Categories are data. A `ResourceCategory` is a config entry matched by
`apiGroups`, `coreKinds` and `excludeKinds`, seeded with the prototype's nine
groups as the curated order. Any kind that matches no category falls through to a
synthesized category named after its API group, so no kind is unreachable without
an allowlist and no allowlist is needed to ship a new CRD.

Columns come from the cluster, not from Rust. For every non-bespoke kind the
column set is `additionalPrinterColumns[].jsonPath`, evaluated engine-side
(including `type: date` formatting) so the result is still a projection and never
a full object on the UI side. Bespoke behaviour — column set, projection
function, inspector tab list — is data keyed by GVK.

One shared table and one shared inspector path serve every kind, core or custom.
CRD instances reuse the same list and detail components rather than a parallel
CRD page, and the row interaction contract is uniform: clicking a row opens the
inspector, `Enter`/`Space` opens it when the row itself has focus, and nested
controls (selection checkbox, namespace chip, row action menu, drill link) act on
themselves and never open the row.

There is deliberately no per-kind class hierarchy. Lens gives each resource a
class plus registry entries; that is how "full Kubernetes coverage" becomes a
300-file maintenance bill and why a new kind on a user's cluster is only visible
through the CRD path. OpenKite keeps one path and varies only data.

Coverage is a gate, not a promise: CI checks that every kind in the discovery
snapshot resolves to a nav route (an exclusion needs a written reason), and that
the row interaction contract holds for every kind in the same snapshot. A new CRD
on a user's cluster appears in the sidebar the first time they connect.

## 6. What is deliberately deferred

These are decisions, not backlog accidents. Each names where the specification
records it.

| Deferred | Recorded in the spec as | Why it is deferred |
| --- | --- | --- |
| Touch and mobile surfaces: bottom nav, sheet inspector, pull-to-refresh, swipe rows | §3 row 16, status `later (companion)` | The mobile target is the least mature one; it ships as a read-only companion rather than a v1 promise. |
| Multi-cluster: tabs, per-cluster connection, switcher | §3 row 17, status `build (v1.1)` | Single-cluster is the v1 shape; the interest key already carries scope, so a second cluster is additive. |
| Plugin runtime: sandboxed iframe UI plugins with a capability-scoped host API | §6, "Honest costs" — the iframe plus capability-broker design | Load-bearing in principle (no Rust UI stack has a runtime plugin ABI), but not v1 work; without it the framework decision would have to be revisited. |
| Legacy discovery fan-out (`/api/v1` + per-group requests) | §3.2.1 (a), "legacy fallback behind a feature flag" | A fallback path, not a maintained second implementation; aggregated discovery is the only default. |
| Any Lens-parity claim on exec, attach, port-forward and exec credential plugins | §6, "Honest costs" — the conformance suite | kube-rs behaviour across cloud providers is unverified; parity is claimed only once a suite diffs it against kubectl/client-go. |

Two surfaces in the coverage table are also not deferrals but new design with no
prototype counterpart — the permission/denied/disconnected/stale states (§3 row
19) and the collection-budget debug panel (§3 row 20) — and are specified as gaps
rather than inherited from the reference.
