# OKT-165 (T8) — Rust→JS push channel: decision of record

- Worktree read: `/opt/data/wt/okt165-push`, detached at `origin/main` = `9793a33be2c3400c16692093221f4df18659c8ad`.
- Read-only recon. No branch, no commit, no push, no PR; no writing git command was run.
- Spec of record: `/opt/data/plans/okt-phase3-ui-spec.md` §6.2, §7.8, §11.
- Every `file:line` below is quoted verbatim from that worktree.

---

## 1. Current state — what exists, with citations

### 1.1 The host channel exists and is complete

`crates/openkite-host/src/push.rs:91`
```
pub struct PushRegistry {
```
`crates/openkite-host/src/push.rs:99`
```
    pub fn subscribe(&mut self, kind: impl Into<String>, ns: Option<String>) -> u64 {
```
`crates/openkite-host/src/push.rs:173`
```
pub fn registry() -> &'static Mutex<PushRegistry> {
```
`crates/openkite-host/src/push.rs:183`
```
pub fn install() -> Option<UnboundedReceiver<PushMessage>> {
```
`crates/openkite-host/src/push.rs:214`
```
pub fn publish(kind: &str, ns: Option<&str>, rows: Vec<Value>) -> usize {
```
`crates/openkite-host/src/push.rs:65`
```
    pub fn to_js(&self) -> String {
```
`crates/openkite-host/src/push.rs:74`
```
            "window.openkite && window.openkite._pushState && \
```

The module's two documented constraints:

`crates/openkite-host/src/push.rs:22`
```
//! 1. **The eval must happen on the Dioxus side.** `document::eval` resolves
```
`crates/openkite-host/src/push.rs:27`
```
//! 2. **Only `GlobalSignal` needs the runtime.** A `Signal<_, SyncStorage>` is
```

And the coalescing constraint the implementer must honour:

`crates/openkite-host/src/push.rs:32`
```
//! Coalescing: [`publish`] sends one message per matching subscription per
```

**The channel is gated on a sender that only one host installs.**

`crates/openkite-host/src/push.rs:167`
```
static PUSH_TX: OnceLock<UnboundedSender<PushMessage>> = OnceLock::new();
```
`crates/openkite-host/src/push.rs:236`
```
    let Some(tx) = PUSH_TX.get() else {
```
`crates/openkite-host/src/push.rs:237`
```
        return 0;
```

### 1.2 What publishes today

`crates/openkite-host/src/state/live.rs:63`
```
        let delivered = crate::push::publish(&kind, None, payload);
```
called from the one reflector-start helper:
`crates/openkite-host/src/state/live.rs:51`
```
fn start_kind<T>(client: Client, kind: &str) -> ResourceState<T>
```

Lifecycle, as the module documents it:
`crates/openkite-host/src/state/live.rs:24`
```
//! [`start`] is idempotent per kind (`Option::get_or_insert_with`), so a
```
`crates/openkite-host/src/state/live.rs:119`
```
    pub fn start(&mut self, client: Client) -> usize {
```
`crates/openkite-host/src/state/live.rs:299`
```
pub fn start(client: Client) -> usize {
```
`crates/openkite-host/src/state/live.rs:310`
```
pub fn generation() -> u64 {
```
`crates/openkite-host/src/state/live.rs:304`
```
pub fn stop() {
```

And the ownership claim that makes this the right publish point:
`crates/openkite-host/src/state/live.rs:11`
```
//! 1. **The push channel could not work app-wide.** A reflector that only runs
```

### 1.3 What has no caller

`push::install` — exactly one caller, desktop only:
`crates/openkite-desktop/src/router.rs:203`
```
        if let Some(mut rx) = crate::push::install() {
```
`crates/openkite-desktop/src/router.rs:206`
```
                    document::eval(&msg.to_js());
```
(inside `AppShell`, `crates/openkite-desktop/src/router.rs:179`)
```
fn AppShell() -> Element {
```

`push::is_installed` — **zero callers**. Definition only:
`crates/openkite-host/src/push.rs:192`
```
pub fn is_installed() -> bool {
```
(verified by `grep -rn "is_installed" crates/ --include=*.rs` → one hit, the definition.)

`live::generation` — **zero callers**. Definition only (`crates/openkite-host/src/state/live.rs:310`), verified by the same grep method.

No Rust-side consumer of the push channel exists anywhere: `grep -rn "push::|_pushState|PushMessage" crates/ --include=*.rs` outside `push.rs` returns only `openkite-api/src/bridge.rs:46` (a doc comment), `openkite-desktop/src/router.rs:203`, `openkite-host/src/bridge.rs:113` and `:126`, `openkite-host/src/state/live.rs:63`, and `openkite-ui/src/plugin_api.rs:263`.

### 1.4 The bridge ops — served on **both** hosts

`crates/openkite-api/src/bridge.rs:47`
```
    Subscribe { kind: String, ns: Option<String> },
```
`crates/openkite-api/src/bridge.rs:49`
```
    Unsubscribe { sub: u64 },
```
`crates/openkite-host/src/bridge.rs:112`
```
            ApiRequest::Subscribe { kind, ns } => {
```
`crates/openkite-host/src/bridge.rs:123`
```
                Ok(serde_json::json!({ "sub": sub, "initial": initial }))
```
`crates/openkite-host/src/bridge.rs:125`
```
            ApiRequest::Unsubscribe { sub } => {
```
`crates/openkite-host/src/bridge.rs:95`
```
    pub async fn handle_post(&self, body: &str) -> ApiResponse {
```

`Bridge` is host-agnostic, and the web host mounts the same dispatch:
`crates/openkite-web/src/routes.rs:43`
```
        .route("/openkite", post(bridge_post))
```
`crates/openkite-web/src/routes.rs:74`
```
    Json(bridge.handle_post(&body).await)
```

**So `subscribe`/`unsubscribe` are answered on the web host too** — they allocate a registry id and return the one-shot `initial` snapshot. What the web host cannot do is deliver anything *after* that, because nothing calls `push::install()` there (§1.3) so `publish` returns `0` at `push.rs:237`.

### 1.5 The JS contract — already fully implemented

`crates/openkite-ui/src/plugin_api.rs:196`
```
pub const OPENKITE_BRIDGE_JS: &str = r##"(() => {
```
`crates/openkite-ui/src/plugin_api.rs:232`
```
    _pushHandlers: new Map(),
```
`crates/openkite-ui/src/plugin_api.rs:233`
```
    subscribe: function (opts, handler) {
```
`crates/openkite-ui/src/plugin_api.rs:238`
```
      call({ op: "subscribe", kind, ns })
```
`crates/openkite-ui/src/plugin_api.rs:247`
```
          if (res.initial !== undefined && handler) {
```
`crates/openkite-ui/src/plugin_api.rs:256`
```
      return function unsubscribe() {
```
`crates/openkite-ui/src/plugin_api.rs:263`
```
    _pushState: function (msg) {
```
`crates/openkite-ui/src/plugin_api.rs:264`
```
      const handler = window.openkite._pushHandlers.get(msg && msg.sub);
```

The JS half needs **no change**: handler map, initial-snapshot delivery, cancel-before-resolve race handling, and the `_pushState` dispatcher are all there.

**But that script is injected only by the desktop host.** Its only consumers:
`crates/openkite-desktop/src/lib.rs:166`
```
        plugin_api::OPENKITE_BRIDGE_JS,
```
inside
`crates/openkite-desktop/src/lib.rs:162`
```
fn bootstrap_head() -> String {
```

The browser document does not contain it. `render_page` emits only these scripts:
`crates/openkite-web/src/ssr.rs:142`
```
pub fn render_page(snapshot: &Snapshot, options: &RenderOptions) -> String {
```
`crates/openkite-web/src/ssr.rs:149`
```
            r#"<script>window.hydrate_queue = [];</script>
```
`crates/openkite-web/src/ssr.rs:151`
```
<script id="openkite-snapshot" type="application/json">{json}</script>
```

So in the browser **`window.openkite` is undefined**: `openkite.subscribe` is unreachable from the page even though the server answers the op.

### 1.6 The crate's state seam, and where a consumer would subscribe

`crates/openkite-ui/src/runtime.rs:1`
```
//! Console state the host publishes and the components read.
```
`crates/openkite-ui/src/runtime.rs:4`
```
//! Every global here is written by a host (at boot, on connect, or on a user
```
`crates/openkite-ui/src/runtime.rs:162`
```
pub fn set_published_capabilities(caps: Option<Capabilities>) {
```
`crates/openkite-ui/src/runtime.rs:186`
```
pub fn capabilities() -> Option<Capabilities> {
```

`runtime.rs` holds **no** push state and no live-rows state — grep for `push` in it returns nothing. `shell.rs` is the pure model layer, no transport:
`crates/openkite-ui/src/shell.rs:4`
```
//! The pure-logic half of the shell: the unified sidebar model (core
```
`crates/openkite-ui/src/shell.rs:217`
```
pub struct ShellState {
```
`crates/openkite-ui/src/shell.rs:288`
```
pub fn status_bar_model(
```

The crate's Rust views do **not** need push: on desktop they read reflector signals directly (`crates/openkite-host/src/state/live.rs:320`)
```
pub fn pods_signal() -> Option<Signal<Vec<Arc<Pod>>, SyncStorage>> {
```

### 1.7 The browser host: what carries state to the page today

Reflectors **do** run there:
`crates/openkite-web/src/host.rs:83`
```
    let started = crate::headless::start_reflectors(client);
```
`crates/openkite-web/src/headless.rs:23`
```
pub fn start_reflectors(client: Client) -> usize {
```

State reaches the page by exactly two paths, both pull:
1. The SSR snapshot in the document (`crates/openkite-web/src/routes.rs:88`)
```
async fn ssr_root(State(bridge): State<SharedBridge>) -> Response {
```
2. A client-initiated POST (`crates/openkite-web/src/routes.rs:45`)
```
        .route("/api/gateway", post(gateway_post))
```
`crates/openkite-web/src/client.rs:19`
```
pub async fn fetch_snapshot() -> Result<Option<Snapshot>, String> {
```

And that POST fires **only on a user click** — there is no timer anywhere:
`crates/openkite-web/src/app.rs:67`
```
    let refetch = move || {
```
`crates/openkite-web/src/app.rs:86`
```
    let on_refresh = move |_: ()| refetch();
```
`crates/openkite-web/src/app.rs:129`
```
    let actions = vec![TopBarAction {
```

**The browser host does not poll today.** It renders a snapshot and waits for a human to press Refresh. "Degrades to polling" therefore means *adding* a poll, not reusing one.

Axum is already the router (`crates/openkite-web/src/routes.rs:40`)
```
    Router::new()
```
with the dependency in place — `crates/openkite-web/Cargo.toml:54`
```
axum = "0.8"
```
and `tokio` already carries `time`: `crates/openkite-web/Cargo.toml:55`
```
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "time", "io-util"] }
```
so an SSE endpoint is a **native addition to the existing router**, not a new transport stack. (`tower-http` has no SSE involvement; axum 0.8's `Sse` responder is in-tree.)

### 1.8 Tests: what a push test could mount

- `crates/openkite-host/tests` **does not exist** — `ls -a crates/openkite-host` returns only `Cargo.toml` and `src`. All host coverage today is in-module `#[cfg(test)]`, e.g. `crates/openkite-host/src/push.rs:248`
```
mod tests {
```
  This makes `crates/openkite-host/src/bridge.rs:3` a stale claim:
```
//! Coverage note: the module's unit + integration coverage is exercised by
```
- `crates/openkite-web/tests/` can mount the real router in-process: `crates/openkite-web/tests/support/mod.rs:44`
```
pub fn app(bridge: Arc<Bridge>, web_root: &Path) -> Router {
```
  via `tower::ServiceExt::oneshot` (`crates/openkite-web/tests/support/mod.rs:58`)
```
pub async fn post_json(app: Router, path: &str, body: Value) -> (StatusCode, Value) {
```
  and a real boot against a fake API server: `crates/openkite-web/tests/boot.rs:21`
```
async fn booted_host_answers_the_console_over_http() {
```
- `crates/openkite-web/tests/ssr.rs:10` imports `render_page`, so the emitted document is directly assertable.
- Desktop `document::eval` is **not** mountable in tests — there is no webview in a test process; only `PushMessage::to_js()` (a pure `String`) is assertable, which `push.rs:329` already does.

---

## 2. Options per host

### Desktop

| Option | Transport | Cost |
|---|---|---|
| **D1 — reuse the `document::eval` pump** (exists) | `push::install()` → `AppShell` task → `document::eval(msg.to_js())` (`router.rs:203`,`:206`) | **Zero new transport.** Already wired; honours the module's constraint 1 (`push.rs:22`). Cost is the existing one: full-snapshot JSON per reflector event per subscription (`live.rs:63` publishes every row, `push.rs:224` clones per subscriber). |
| D2 — native Rust signal consumer | read `live::pods_signal()` directly | Also zero transport, but bypasses JS entirely — does not satisfy "a watch update reaches JS". Already how the Rust views work; not a T8 deliverable. |

### Browser

| Option | Transport | Cost |
|---|---|---|
| **B1 — inject the bridge JS + a poll timer** | `OPENKITE_BRIDGE_JS` into `render_page`; `use_future` interval calling the existing `fetch_snapshot()` | Small. Two edits (`ssr.rs:142` head, `app.rs:67` timer). Makes `window.openkite.subscribe` resolve with its `initial` snapshot (`bridge.rs:123`) and makes state arrive unattended. **Still polling** — does not satisfy "without polling" on this host. |
| B2 — SSE `GET /openkite/events` on the existing axum router | axum `Sse` + a second `push::install`-style fan-out that survives without a Dioxus runtime; `EventSource` in JS calling `_pushState` | Medium. Needs a *broadcast* sender (the current `UnboundedSender` is single-consumer and `OnceLock`-set once, `push.rs:167`), per-connection subscription lifecycle, reconnect/`Last-Event-ID`, and the bridge JS in the document. New public surface on the host crate. |
| B3 — WebSocket | new upgrade handler | Strictly more than B2 for the same one-way need. Rejected. |

---

## 3. Recommendation and the scope boundary

**Recommended: D1 + B1 for T8; B2 stays Phase 4. The Phase-4 SSE ticket is NOT a duplicate.**

> **T8 is the crate/bridge consumer and the desktop transport. It does not own the browser transport.**

Answering the brief's question directly: the channel already exists and already works end-to-end on desktop; what T8 owes is a *proven* consumer, a documented cost, and a browser host that degrades honestly instead of silently dropping pushes (`push.rs:237` returns `0` there, with no log and no error). The browser *push* transport is a separate problem — the page has no `window.openkite` at all (§1.5) and the current sender is single-consumer and one-shot-installed (`push.rs:167`, `push.rs:183`), which an SSE fan-out cannot reuse as-is. That is a distinct design, so workitem `52f5d42f-06b6-44c9-a3a9-dd5cb5bf9d39` ("[Phase 4] Live push — SSE /openkite/events so the browser stops being a snapshot") remains valid and non-overlapping.

**Scope boundary, stated so the implementation task has no ambiguity:**

T8 (OKT-165) **includes**:
1. Proving `publish → pump → PushMessage` delivery with an installed pump (no eval).
2. Publishing a push-mode status through `openkite-ui::runtime` so the console can state which mode it is in.
3. Injecting `OPENKITE_BRIDGE_JS` into the browser document so `window.openkite` and the `subscribe` op exist on both hosts (contract parity; the op is already served, `routes.rs:43`).
4. A polling fallback timer in the browser host's `App`, which is the degradation path.
5. Documenting the cost (snapshot-per-event, per-subscription clone).

T8 **excludes** (→ Phase 4, workitem `52f5d42f`):
1. `GET /openkite/events` and any axum `Sse` responder.
2. Converting `PUSH_TX` from a single-consumer `UnboundedSender` to a broadcast fan-out.
3. `EventSource` wiring, reconnect, `Last-Event-ID` / revision-gap recovery.
4. Removing the browser poll (Phase 4 turns the poll into the fallback, not the primary).

Effort: **Medium (1–2 d)**.

---

## 4. Degradation path and the stated bound

**Detection** — three states, decided by the host, not guessed by the view:
1. `Push` — the host called `push::install()` and got `Some(rx)` (`push.rs:183`). Desktop only today (`router.rs:203`). The already-defined, never-called `push::is_installed()` (`push.rs:192`) becomes the predicate; this is also what gives that function its first caller.
2. `Polling` — `window.openkite.subscribe` resolved (so the server answered `{sub, initial}`, `bridge.rs:123`) but no `_pushState` arrived within the liveness window. The JS side needs no change to detect this: `revision` is already monotonic per subscription (`push.rs:56`, `push.rs:155`), so "no revision > 0 within the window" is the signal.
3. `Off` — the `subscribe` call itself failed (`plugin_api.rs:255` already logs this path).

**The bound to write the acceptance criterion at:**

- **Push active: a cluster change is visible within 2 s.** Justification: the reflector publishes synchronously inside its watch callback (`live.rs:63`), the channel is unbounded (`push.rs:184`) and the pump evals on the next Dioxus tick (`router.rs:206`). The only unbounded term is kube watch latency. 2 s is the honest budget; sub-second is achievable but not worth promising without the Xvfb re-measure the spec still owes (§6.1, spec line 131).
- **Degraded to polling: visible within 15 s** — a 10 s poll interval plus one round-trip and render. Write both numbers into the criterion; a single number cannot describe a channel whose acceptance explicitly requires a fallback.

**Re-render bound** ("an update does not re-render the whole view"): the existing payload is a full snapshot per kind (`live.rs:59`–`:62`), so this must be satisfied *downstream* of the channel by keying rows on `metadata.uid`, not by making the channel delta-based. Changing the payload to deltas is out of T8's scope and would break the `initial`-snapshot shape JS already relies on (`plugin_api.rs:247`).

---

## 5. Exact implementation plan

### Files to change

| File | Change |
|---|---|
| `crates/openkite-ui/src/runtime.rs` | **EDIT.** Add `PushMode { Push, Polling, Off }` + `PUSH_MODE: GlobalSignal<PushMode>` + `set_push_mode`/`push_mode`, following the existing host-publishes/crate-reads pattern (`runtime.rs:4`). ~20 lines. No new module: this is exactly the state class `runtime.rs` holds. |
| `crates/openkite-ui/src/shell.rs` | **EDIT.** One status-bar entry from `PushMode` in `status_bar_model` (`shell.rs:288`) so the mode is visible, not implicit. |
| `crates/openkite-desktop/src/router.rs` | **EDIT.** In the existing `use_hook` at `:202`, set `PushMode::Push` on `Some(rx)`. ~2 lines. The pump itself is unchanged. |
| `crates/openkite-web/src/ssr.rs` | **EDIT.** Emit `<script>{openkite_ui::plugin_api::OPENKITE_BRIDGE_JS}</script>` in `render_page`'s head (`ssr.rs:142`), before the hydration module. |
| `crates/openkite-web/src/app.rs` | **EDIT.** `use_future` 10 s interval calling the existing `refetch` (`app.rs:67`); set `PushMode::Polling` at mount. No new dependency — `tokio` already has `time` (`Cargo.toml:55`). |
| `crates/openkite-host/src/push.rs` | **EDIT, docs only.** State the per-event snapshot cost and that `publish` returning `0` on a host with no pump is the degradation signal, not an error. |

No change to `openkite-api`, no change to `OPENKITE_BRIDGE_JS` (`plugin_api.rs:196`), so **the plugin contract is unchanged** — which is the acceptance criterion the spec states twice (spec line 148, line 220).

### Tests to add

| Test file | Test | Runnable here? |
|---|---|---|
| `crates/openkite-host/tests/push_pump.rs` **(NEW FILE, NEW DIR)** | `installed_pump_receives_one_message_per_matching_subscription`: `install()`, `registry().subscribe("pods", None)`, `publish("pods", None, rows)`, assert `rx.try_recv()` yields `sub`/`kind`/`rows`/`revision == 1`, and a second publish yields `revision == 2`. | **Not verified here** — no local `cargo`. Proof: `cargo test -p openkite-host --test push_pump`. |
| ↑ **why a new file, not a new `#[cfg(test)]` case** | `PUSH_TX` is a process-global `OnceLock` (`push.rs:167`) and the existing unit test `publish_without_a_pump_is_a_no_op_not_a_panic` (`push.rs:349`) asserts `sent == 0`. Installing a pump in the same test binary would make that assertion order-dependent and flaky. An integration test gets its own process. | — |
| `crates/openkite-web/tests/routes.rs` **(EXISTING)** | `subscribe_op_answers_with_a_sub_id_and_initial_snapshot` and `unsubscribe_op_removes_it` via `support::post_json` (`support/mod.rs:58`), plus `publish_on_the_web_host_delivers_nothing` asserting `openkite_host::push::publish(...) == 0` — pinning the degradation contract. | **Not verified here.** Proof: `cargo test -p openkite-web --test routes`. |
| `crates/openkite-web/tests/ssr.rs` **(EXISTING)** | `rendered_page_defines_the_openkite_bridge`: `render_page` output contains `window.openkite` and `_pushState`. | **Not verified here.** Proof: `cargo test -p openkite-web --test ssr`. |
| `crates/openkite-ui/tests/shell.rs` **(EXISTING)** | `status_bar_states_the_push_mode`: one entry per `PushMode`. | **Not verified here.** Proof: `cargo test -p openkite-ui --test shell`. |

**Cannot be proven by any test in this repo:** that `document::eval` actually reaches the page. There is no webview in a test process (§1.8), and `push.rs:22` records that the failure mode is silent. Only the Xvfb/WebKitGTK run the spec still owes (spec line 207: `//! **Unverifiable from here:** whether the desktop host (wry/WebKitGTK) renders these routes correctly`) can prove the last hop. T8 should assert `to_js()` (already done at `push.rs:329`) and state the gap, not fake it.

---

## 6. What would falsify this recommendation

Written as claims, each checkable in one command:

1. **`openkite_host::push::is_installed()` is `false` in the browser host process after `serve()` returns its first response.** If it is ever `true`, the browser already has a pump, B1 is unnecessary, and T8 should own the browser transport outright. Check: assert it in `crates/openkite-web/tests/boot.rs`.
2. **`render_page`'s output contains no occurrence of `window.openkite`.** If it does, §1.5 is wrong, the browser already has the JS contract, and the only remaining browser work is the SSE endpoint — making T8 purely a consumer ticket. Check: `grep -c 'window.openkite' <(render_page output)` in `tests/ssr.rs`.
3. **`PUSH_TX` (`push.rs:167`) is a single-consumer `UnboundedSender` set through a `OnceLock`.** If it were a `broadcast::Sender`, SSE would be a ~40-line addition to the existing router and the Phase-4 ticket *would* collapse into T8. Check: read `push.rs:167` and `push.rs:183`.

If (1) or (2) is falsified, the scope split in §3 is wrong and T8 must absorb the browser transport. If (3) is falsified, the Phase-4 SSE ticket is a duplicate and should be closed into T8.

---

## Could not verify

- **No local build.** The workspace cannot be linked here (no local `cargo`); `cargo fmt` is a parse check only. Every test in §5 is "not verified here" and carries the command that would prove it. No claim above depends on compilation — all are textual facts about the worktree at `9793a33`.
- **The Phase-4 ticket body.** Only its title and workitem id (`52f5d42f-06b6-44c9-a3a9-dd5cb5bf9d39`) were available; the Plane web UI is not reachable from this task. The duplicate/not-duplicate verdict is argued from the repo's transport facts (§1.5, §1.7, §2-B2), not from that ticket's acceptance text. **If its body claims the crate/bridge consumer as well as the endpoint, the overlap is in its scope, not T8's, and it should be trimmed rather than T8 widened.**
- **OKT-165's own body on the board** was not retrieved; the acceptance wording used here is the brief's restatement plus the spec of record (`/opt/data/plans/okt-phase3-ui-spec.md:148`, `:220`).
- **`crates/openkite-host/tests` does not exist** (the brief assumed it did). `crates/openkite-host/src/bridge.rs:3` claims integration coverage in `tests/` that is not in this crate — a stale doc comment, flagged but not fixed (read-only task).
- **The 2 s bound is a budget, not a measurement.** No latency was measured here; kube watch latency is unbounded in principle, and the WebKitGTK re-measure the spec owes (spec line 131, line 207) is still outstanding.
- **`publish` coalescing behaviour under a burst** (`push.rs:32`) was read, not exercised. Whether 13 reflectors starting at once (`live.rs:121`–`:147`) produce a thundering herd of full snapshots is unmeasured and is the main cost risk in the recommendation.
