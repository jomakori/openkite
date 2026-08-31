# OKT-33 — Workload views (Pods → CronJobs)

**Ticket:** [OKT-33 — Workload views (Pods → CronJobs)](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/1aa27b65-488c-48a0-98b3-668b5fa876a3)
**Branch:** `feat/okt33-workload-views` (from `origin/main`)
**PR title:** `feat(openkite): workload views (P2 UI) (OKT-33)`
**Workitem UUID (PR body hyperlink):** `1aa27b65-488c-48a0-98b3-668b5fa876a3`
**Plane state (start):** Backlog (`phase-2` label)
**Status (start):** about to move to **In Progress** once the plan is approved.

## 0. What this plan is — and what is already on `main`

This is a P2-UI plan grounded in ground-truth that has shifted since the ticket was filed. **The P1 OKT-10 work, the OKT-29 design system, and the OKT-32 ResourceTable are all already merged to `main` as of 2026-08-30.** The plan must therefore describe a *delta* on top of the existing tree, not a green-field build. Concretely:

| Ticket | What is already on `main`                                                                | Where                                                                  |
|--------|------------------------------------------------------------------------------------------|------------------------------------------------------------------------|
| OKT-10 | P1 foundation — per-kind column defs, row mappers, status mapping, `WorkloadKind` enum   | `src/workloads.rs` (pure logic) + `src/views/workloads.rs` (Dioxus)    |
| OKT-29 | Design system — `.panel`, `.chip`, `.btn`, `.pill`, `.health-dots`, `.resource-name`, `.ns-chip`, etc.; `--bg-*`/`--fg-*`/`--accent`/`--green`/`--yellow`/`--red` tokens | `assets/main.css` + `src/design/{mod.rs, tokens.rs}` (PR #49, `bd67b14`) |
| OKT-32 | Generic `ResourceTable` — virtualized, sortable, filterable, namespace chips              | `src/components/resource_table.rs` (sort/filter/windowing + the Dioxus `#[component] ResourceTable`) |

The OKT-33 ticket description ("per-kind columns + status from workload row mapping (OKT-10)") is therefore already substantially satisfied. The **actual delta** that OKT-33 needs to land:

1. **Mockup-aligned column expansion** — the existing `pod_columns()` ships `Name | Status | Ready | Restarts` (4 cols). The mockup (`openkite-console.html:192`) prescribes `Name | Namespace | Health | Restarts | Controller | Node | QoS | Age | Status` (9 cols) for Pods and an equivalent richness for the other kinds.
2. **Per-kind age column** — every kind gets a relative-age cell from `metadata.creation_timestamp`.
3. **Multi-select namespace chips** (per the mockup at `openkite-console.html:174-180`: a row of `.chip` toggles, count badges, "All" sentinel). The current `ResourceTable` (`resource_table.rs:206`, `:308-322`) is single-select toggle. The OKT-29 `.chip` class (`assets/main.css:221-239`) is already multi-select-ready (no state-encapsulation in the CSS — the consumer picks).
4. **Per-kind health/control columns** — Pods only for the dot-row (the rest of the kinds get a `Controller` cell from `owner_references` / `kind`).
5. **The view mount already exists** — `src/router.rs:352-355` mounts `crate::views::workloads::WorkloadView`, which already dispatches across the 7 kinds via a tab strip (`src/views/workloads.rs:81-106`). No router change needed.
6. **No new deps** — `k8s-openapi` 0.28, `kube` 4, `dioxus` 0.7 already in `Cargo.toml:36-41` carry everything OKT-33 needs.

The plan below builds that delta in PR-friendly chunks, each compiling on its own, each ending in a green `cargo fmt --check`.

## 1. Ticket scope

### What ships in this PR (P2-UI per-kind views + design-system application)

- **7 per-kind `*_columns()` outputs** in `src/workloads.rs`, aligned to the mockup (Namespaces column removed from the table body — it is in the chip filter; Controller/Node/QoS/Health/Age added per kind).
- **7 per-kind `*_row()` outputs** that emit the expanded cells using the existing pure mapping primitives. No Dioxus import in `src/workloads.rs` (per the skill's "split pure logic from view" rule).
- **Multi-select namespace chips** in the `ResourceTable` toolbar — `ns-chip` class stays (the existing single-select chip in `resource_table.rs:308-322` is extended to a `Vec<String>` of selected namespaces, with an "All" sentinel that maps to empty-selection).
- **Per-kind health indicator** — Pod rows get a `.health-dots` cell (`assets/main.css:301-305` already exposes `.dot.ok`/`.warn`/`.err`). Non-pod rows get a text health from the same `replicas_status()` mapping the existing P1 code already returns.
- **Per-kind age cell** — derived from `metadata.creation_timestamp` via a small pure helper `age_cell(ts) -> Cell` that produces a humanized string (e.g. `2d`, `12h`, `38m`) plus a numeric sort key (seconds since now, frozen at render time — see Risks).
- **A `tests/workload_row.rs` expansion** covering all 7 kinds, all status branches, and the new age/health cells. The existing `tests/workloads.rs:38-97` covers Pod and Deployment; the PR extends it to all seven.
- **A `tests/resource_table.rs`** (or extension of `tests/workloads.rs`) covering the multi-select chip filter — the matrix "all / one / many / none" with a 3-namespace fixture.

### What is explicitly deferred (intentional, ticket↔ticket boundary)

- **Per-kind live watch streams per cell** (status dot pulses, restart-counter interpolation): OKT-40/41 — the current reflector + `set()` is already event-driven; per-cell streaming is a different cadence and a different ticket. Tracked in the OKT-40 ticket description.
- **Pod detail / drilldown interaction** (clicking a row opens an inspector with the container list from `src/pod.rs:58-83`): OKT-34. The mockup's `.inspector` slide-over (`assets/main.css:362-396`) is already shipped by the design system; OKT-34 wires the click handler + content.
- **Per-row action buttons** (`RowActions { on_delete, on_edit, on_scale }` is already declared at `resource_table.rs:178-183`): wired up in OKT-43 (CRUD UI). OKT-33 passes `None` for `row_actions`.
- **The kind-tab strip** is already a single-component set of `button` elements (`src/views/workloads.rs:86-94`) styled ad-hoc; the design-system equivalent is `.chip` (44px min-height) — minor visual migration is a one-liner per button but is deferred so this PR stays a pure column/row mapping change.
- **Dioxus 0.7 component wrappers around the existing `StatusBadge`** (`.status-badge` → `.pill` rename, per the openkite-dev skill "Design-system primitive class names overlap with existing components" gotcha): deferred. The existing `StatusBadge` is still rendered inside `ResourceTable::render_table_cell` (`resource_table.rs:382-401`); OKT-43 (CRUD) is the natural consumer that can absorb the rename.
- **ArgoCD app views, services, ingress, configmaps, secrets** — separate OKT-N tickets. OKT-33 is *only* the seven workload kinds the ticket title lists.
- **YAML view, log view, terminal view, exec** — separate tickets (referenced in `src/router.rs:348-360` placeholders).

## 2. File structure

### Layout

```
src/
├── lib.rs                              # No change. `pub mod` list at L3-29 is alphabetical;
│                                       # `workloads` already present at L28; `views` at L27.
├── workloads.rs                        # EXTEND in place. Pure logic — no Dioxus import.
│                                       # New: kind_age_cell(), pod_health_dots(), controller_of(),
│                                       # per-kind column+row updates (Pods only adds Health/Age;
│                                       # the rest add Age; Deployments/STS/DS get a Controller cell
│                                       # derived from owner_references). Existing helpers stay.
├── views/
│   ├── mod.rs                          # No change. Single `pub mod workloads;` line at L2.
│   └── workloads.rs                    # EXTEND in place. Tab strip stays; per-kind tables keep
│                                       # using the same `*_columns()`/`*_row()` shape. Pass
│                                       # `row_actions: None` to ResourceTable.
├── components/
│   ├── mod.rs                          # No change. Three modules: resource_table, status_badge,
│                                       # theme_selector.
│   └── resource_table.rs               # EXTEND in place. `namespace` signal goes from
│                                       # `Signal<Option<String>>` (L206) to `Signal<HashSet<String>>`
│                                       # (multi-select). Add an "All" sentinel chip.
│                                       # Toolbar renders the `.chip` class from the design system
│                                       # (currently renders `.ns-chip` ad-hoc at L156-163 of main.css;
│                                       # the plan keeps `.ns-chip` for backward compat with the
│                                       # topbar selector and uses `.chip` for the table toolbar).
│   └── status_badge.rs                 # No change. Still renders `<span class="status-badge">` —
│                                       # the .pill rename is deferred to OKT-43.
├── state/
│   ├── mod.rs                          # No change.
│   └── resources.rs                    # No change. `drive_reflector` and `ResourceState` already
│                                       # serve the workload view; OKT-33 consumes the same API.
└── router.rs                           # No change. `/workloads` already mounts
                                       # `crate::views::workloads::WorkloadView` at L353-355.

tests/
├── workloads.rs                        # EXTEND in place. New: STS/DS/RS/Job/CronJob row tests,
│                                       # age_cell, pod_health_dots, controller_of. (Existing Pod +
│                                       # Deployment tests at L38-97 stay.)
└── resource_table.rs                   # NEW. Multi-select chip filter: tests of
                                       # `selected_nses(rows, &selected)` over a 3-ns fixture
                                       # (all / empty / one / many / "All" sentinel). Pure logic
                                       # only — no Dioxus runtime; the table's pure filter function
                                       # is exposed as a free fn `pub fn namespace_filter(rows, selected)`.
```

### Why this layout (not the layout in the ticket brief)

The ticket brief proposes `src/views/workloads/{pod,deployment,…}.rs` (one file per kind). The current P1 tree has the **pure logic in `src/workloads.rs`** and the **Dioxus view in `src/views/workloads.rs`** — a clean split that is consistent with the foundation-first pattern (`src/views/workloads.rs:7-12` already carries the "free of Dioxus makes it testable in isolation" doc comment). Splitting per-kind across 7 files is **YAGNI**:

- The pure mapping fits in one file today (420 lines, `src/workloads.rs`); per-kind it averages 30-50 lines.
- Per-kind split buys nothing the per-function split (already in place) does not already buy. The skill's `references/openkite-console-mockup.md` and the OKT-29 plan's foundation-first style both favour "smallest split that holds".
- If a kind grows complex (e.g. CronJob with its own schedule editor), split then, not now.

If a reviewer insists on per-kind files, the path is: 7 new files under `src/views/workloads/`, a `mod.rs` re-exporting each, and a `pub mod` insertion in `lib.rs` (alphabetical — between `views` and `workloads` at L27-28). The plan keeps the **simpler layout** as the default and flags the alternative.

### `lib.rs` change

**None.** `workloads` and `views` are already in `lib.rs:27-28`. The new `tests/resource_table.rs` is auto-picked-up by Cargo's `tests/` discovery.

## 3. The data model — `WorkloadKind` and the per-kind `to_row`

### Existing on `main` (no change)

- `WorkloadKind` enum (`src/workloads.rs:15-50`) with 7 variants, `ALL` constant, `label()` method.
- Per-kind `*_columns() -> Vec<ColumnDef>` and `*_row(&T) -> ResourceRow` functions (L96-420).
- `name_status_columns(&[ColumnDef; N]) -> Vec<ColumnDef>` helper (L53-70) — common Name+Status prefix.
- `replicas_status(ready, desired) -> (String, StatusKind)` (L78-85) — Pods/Deployments/STS/DS/RS status.
- `pod_status`, `pod_ready`, `pod_restarts` (L135-173) — Pod-specific helpers.

### New on this PR

#### `age_cell(ts: &Option<Time>) -> Cell`

Pure helper in `src/workloads.rs`. Emits `Cell::text("2d")` for a display value plus `Cell::number` sort key (seconds since now, signed — newer sorts higher under default ascending order, so a "Sort by Age ▼" puts the youngest first). Handles `None` → `Cell::text("-")` (sort key `f64::NAN` is filtered by the comparator, see Risks §3). The "2d"/"12h" formatting uses a small pure `humanize_age(now, then) -> String` so the unit is testable.

```rust
// Skeleton — not final code.
pub fn age_cell(ts: &Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>) -> Cell {
    let Some(ts) = ts else { return Cell::text("-") };
    // Time wraps jiff::Timestamp (skill: kube-rs 4.2).
    // humanize_age returns a "2d" / "12h" / "38m" / "5s" string.
    let seconds = ts.0.as_second() as f64;  // or jiff::Timestamp::now().as_second() diff
    Cell::number(humanize_age(...), seconds)
}
```

#### `pod_health_dots(pod: &Pod) -> Cell`

Emits a `Cell::text` with a marker class — the `Cell` enum does not currently carry a class. **Two options** (skill: smallest split that holds):

- **Option A (chosen)**: add a `Cell::html(rich: &'static str, sort_text: String, sort_key: SortKey)` variant that lets the consuming view render a markup blob. The `ResourceTable::render_table_cell` (`resource_table.rs:382-401`) gets one new match arm that injects `class="health-dots"` and the dots. Cheap, single-source-of-truth.
- **Option B**: add a `Cell::component(...)` that holds a `fn() -> Element`. Compiles, runs, but invites the Dioxus-glob-import footgun into `src/workloads.rs` — forbidden by the skill. **Rejected.**

The new variant is added to `src/components/resource_table.rs:93-98` (the `Cell` enum). The 7 `*_row` functions in `src/workloads.rs` build `Cell::html("<span class=\"health-dots\">…</span>", "2/2", SortKey::Number(2.0))` only for Pods; the other kinds keep `Cell::status(...)` for the Status column.

#### `controller_of(obj: &T) -> String`

For Deployments, STS, DS, RS — pull `metadata.owner_references[0].kind + "/" + name` (e.g. `Deployment/checkout-api`). Empty for cluster-scoped or owner-less. For Pods, the `Controller` cell comes from `owner_references[0].kind + "/" + name` (e.g. `Deployment/checkout-api` — this is what the mockup shows at `openkite-console.html:201`).

A generic helper via a `Resource` bound is awkward because `Resource` does not expose a method that returns `&ObjectMeta` with a stable lifetime. The clean shape is a per-kind helper that takes the right type and returns a `String`:

```rust
pub fn controller_for_pod(pod: &Pod) -> String { /* owner_references[0] */ }
pub fn controller_for_deployment(d: &Deployment) -> String { /* owner_references[0] */ }
// etc.
```

…all of which delegate to one shared `format_controller(refs: &[OwnerReference]) -> String` private helper.

#### `node_cell(pod: &Pod) -> Cell` (Pods only)

`pod.spec.node_name` → `Cell::text(...)`. Returns `Cell::text("-")` when `node_name` is absent (Pending pods).

#### `qos_cell(pod: &Pod) -> Cell` (Pods only)

`pod.status.qos_class` (e.g. `Burstable`, `Guaranteed`, `BestEffort`). The mockup renders the class in uppercase via CSS (`assets/main.css:329` — `.qos { text-transform: uppercase; }`) — do not uppercase in Rust; the CSS handles it.

### Per-kind column plan

Concretely — current columns in `src/workloads.rs` and the OKT-33 delta:

| Kind              | Current columns                                   | OKT-33 columns                                                                                  |
|-------------------|---------------------------------------------------|-------------------------------------------------------------------------------------------------|
| Pods              | Name, Status, Ready, Restarts                     | Name, **Health, Restarts, Controller, Node, QoS, Age**, Status                                  |
| Deployments       | Name, Status, Ready, Available                    | Name, Ready, **Up-to-date, Available, Controller, Age**, Status                                 |
| StatefulSets      | Name, Status, Ready                               | Name, Ready, **Up-to-date, Age**, Status                                                        |
| DaemonSets        | Name, Status, Ready, Available                    | Name, Ready, **Desired, Current, Available, Age**, Status                                       |
| ReplicaSets       | Name, Status, Ready                               | Name, Ready, **Desired, Age**, Status                                                           |
| Jobs              | Name, Status, Completions                         | Name, **Completions, Duration, Age**, Status                                                    |
| CronJobs          | Name, Status, Schedule, Suspend                   | Name, **Schedule, Suspend, Last schedule, Age**, Status                                         |

**Removed from all kinds**: the `Status` column is kept but the table-wide `Status` cell that the existing P1 code already includes in the prefix is **kept** (it is the first thing the user scans). The `name_status_columns` helper (`src/workloads.rs:53-70`) is the only place that hardcodes the name+status prefix; the new per-kind columns append after the new middle columns. The `name_status_columns` helper signature changes from `(extra: &[ColumnDef])` to `(middle: &[ColumnDef], suffix: &[ColumnDef])` so the 7 callers can pass a middle and a tail — that is a 1-line signature change at one site, but all 7 callers must be updated. The 7 test sites in `tests/workloads.rs` get a column-count bump assertion.

### Per-kind row plan

Each `*_row()` function returns a `ResourceRow` whose `cells` vec now mirrors the column list above. The order is **exact** — `ResourceTable` does not reorder (it only sorts). The two new cells (`Age` + `Health`/`Controller`/etc.) are appended at fixed positions so the test assertions stay stable.

| Function                | cells len (old → new) | new cells (indexes)                            |
|-------------------------|-----------------------|------------------------------------------------|
| `pod_row`               | 4 → 9                 | 1=Health, 4=Controller, 5=Node, 6=QoS, 7=Age   |
| `deployment_row`        | 4 → 7                 | 3=Up-to-date, 4=Controller, 5=Age              |
| `stateful_set_row`      | 3 → 5                 | 3=Up-to-date, 4=Age                            |
| `daemon_set_row`        | 4 → 7                 | 3=Desired, 4=Current, 5=Available, 6=Age       |
| `replica_set_row`       | 3 → 5                 | 3=Desired, 4=Age                               |
| `job_row`               | 3 → 5                 | 3=Duration, 4=Age                              |
| `cron_job_row`          | 4 → 6                 | 3=Last schedule, 4=Age                         |

The `Cell` enum gains one new variant (`Cell::html`); existing variants and their `sort` semantics are unchanged. `compare_sort_keys` (`resource_table.rs:50-57`) does not change — `Cell::html` builds a normal `SortKey::Text`/`SortKey::Number`, and the comparator just uses that key.

## 4. Status mapping

### Already on `main` (no change)

- `replicas_status(ready, desired) -> (String, StatusKind)` (`src/workloads.rs:78-85`) — returns `Pending | Ready | Degraded`.
- `pod_status(pod) -> (String, StatusKind)` (L135-149) — phase-based.
- Job status (L359-367) — `Failed | Succeeded | Running | Pending`.
- CronJob status (L405-409) — `Suspended | Active`.
- `StatusKind` enum (`src/components/status_badge.rs:13-24`) — 10 variants with `class()` and `label()`. CSS class mapping already covers the semantics (L28-35).

### New on this PR

None. The status mapping is **complete** on `main`. The OKT-33 delta is **column expansion + display ordering**, not new statuses. The mockup's `Healthy/Degraded/Progressing/Suspended/Missing` vocabulary maps cleanly to the existing `StatusKind` set (the design system uses `.pill.success/.warn/.danger/.muted`):

| Mockup term      | `StatusKind`        | `pill.*` class | `status-*` class on `main` |
|------------------|---------------------|----------------|-----------------------------|
| Healthy          | `Ready` / `Running` | `.success`     | `.status-ok`                |
| Degraded         | `Degraded`          | `.danger`      | `.status-err`               |
| Progressing      | `Pending`           | `.warn`        | `.status-warn`              |
| Suspended        | `Suspended`         | `.muted`       | `.status-muted`             |
| Missing          | `Unknown`           | `.muted`       | `.status-muted`             |

The skill (`clean-code-comments` and the openkite-dev "stale module-doc" gotcha) explicitly recommends stripping OKT-N refs from comments. **Status mapping doc-comments are reformatted** to drop `(OKT-10)` or `(P1)` parentheticals that may have accumulated during the P1 cycle.

## 5. Namespace chips (multi-select)

### Current behaviour on `main`

`ResourceTable` keeps a single `Signal<Option<String>>` (L206); the chip click toggles between `Some(label)` and `None` (L308-322). The `view` filter (L216-221) only keeps rows whose `namespace == selected`. The class is `.ns-chip` (L156 of `main.css`); the mockup class is `.chip` (OKT-29 design system, `assets/main.css:221-239`).

### OKT-33 delta

- `namespace: Signal<HashSet<String>>` — empty set = "All" (no filter).
- First chip in the toolbar is a synthetic `"All"` chip with count = total rows. Click toggles between `set` empty and `set = all observed namespaces`.
- All other chips toggle a single `ns` in/out of the set.
- The chip class switches from `.ns-chip` to `.chip` (the design-system class) inside `ResourceTable` only — the topbar's existing `.ns-chip` use (`assets/main.css:155-163`) is unaffected.
- The filter helper is extracted to a free fn `pub fn namespace_filter(rows: &[ResourceRow], selected: &HashSet<String>) -> Vec<ResourceRow>` so `tests/resource_table.rs` can test it without Dioxus.

### Why HashSet, not Vec

The `ResourceTable` filter runs on every keystroke (the search field) and every `set` update. A `Vec<String>::contains` is O(n); a `HashSet<String>::contains` is O(1). With the typical 5-10 namespaces in a cluster this is academic, but a `HashSet` matches the existing `HashSet` idiom in the codebase (`crates/plugin-sdk`, OKT-46 bridge, etc.) and avoids the "do I have a duplicate?" review question in a future PR.

### Signal pattern (skill: Dioxus 0.7)

`Signal<HashSet<String>, SyncStorage>` — captures the `HashSet` into the signal, mutates in place, and reads a clone for the filter. Per the skill's "Signal::set needs &mut self" rule, the click handler does `let mut set = signal; set.write().insert(ns.clone());` and the filter does `let selected = signal.read().clone(); namespace_filter(&rows, &selected)`.

## 6. Dispatch view

### Current behaviour on `main`

`src/views/workloads.rs:80-106` — `WorkloadView` keeps a `Signal<WorkloadKind>`, renders a `.kind-tabs` row of buttons, and dispatches via a `match kind() { ... }` to one of 7 `*Table` components (PodsTable, DeploymentsTable, …, CronJobsTable) that each spin up a per-kind reflector via the `workload_table!` macro (L20-49). The reflector writes `Vec<ResourceRow>` into a per-table `Signal` and renders `<ResourceTable columns={...} rows={...} />`.

### OKT-33 delta

- **The dispatch stays as-is.** No new route, no router change.
- The 7 `*Table` components now pass `row_actions: None` to `ResourceTable` (the existing `RowActions` is `Default::default()`-able at L178-183 of `resource_table.rs`).
- The kind-tab buttons get a `class: "chip"` swap (the OKT-29 design-system class) — one-line per button, defer to OKT-43 if the design system needs a `.kind-tab` variant. **Defer to OKT-43** to keep this PR a mapping/column change; OKT-43 is the next view ticket and absorbs the visual migration.
- Each kind's table gains the same multi-select chip toolbar (via `ResourceTable`'s new `namespace` signal). No per-kind override.

### Why a single dispatch (not seven routes)

The router already has one `#[route("/workloads")]` (`src/router.rs:66-67`); adding seven sub-routes (`/workloads/pods`, `/workloads/deployments`, …) is **YAGNI** at this stage. The tab strip is the deeplink surface for now; URL-driven per-kind views become important only when JS plugins need to link to "the Deployments tab" or when the inspector (OKT-34) needs to embed a "view all pods of this controller" link. Both are downstream tickets. The plan **flags** the seven-route alternative as a follow-up (post-OKT-43) but does not ship it.

## 7. Tests

### Current on `main` (`tests/workloads.rs`)

97 lines. 4 tests, all using `k8s_openapi` builders:
- `pod_row_maps_name_status_ready_and_restarts` (L38-48) — Running pod, 2/3 ready, 4 restarts.
- `pod_row_failed_phase_maps_to_failed_status` (L50-54) — Failed phase.
- `deployment_row_ready_replicas_maps_to_ready_status` (L56-72) — Deployment 3/3.
- `deployment_row_partial_replicas_maps_to_degraded_status` (L74-90) — Deployment 1/3.
- `workload_kind_lists_seven_kinds_with_labels` (L92-97) — enum sanity.

### OKT-33 additions (in `tests/workloads.rs` + `tests/resource_table.rs`)

**`tests/workloads.rs`** — extend, do not split:

- `pod_row_emits_nine_cells_with_mockup_layout` — assert `cells.len() == 9`, `cells[1]` carries the health-dots marker, `cells[7]` is the age, `cells[8]` is the status.
- `pod_health_dots_count_ok_versus_failed` — Running with all-ok → 2 green dots; with one container `ready=false` → 1 green + 1 red.
- `pod_health_dots_empty_when_no_container_status` — Pending pod → 0 dots (or "No status" placeholder cell).
- `controller_for_pod_extracts_owner_reference` — Pod with `owner_references = [{kind: "Deployment", name: "checkout-api"}]` → `Cell::text == "Deployment/checkout-api"`.
- `controller_for_pod_with_no_owner_returns_dash` — Pod without owners → `Cell::text == "-"`.
- `age_cell_formats_seconds_minutes_hours_days` — `humanize_age` covers `0s / 30s / 5m / 3h / 2d`.
- `age_cell_returns_dash_for_none_timestamp`.
- `deployment_row_includes_up_to_date_and_controller` — assert new cells.
- `stateful_set_row_includes_up_to_date_and_age`.
- `daemon_set_row_includes_desired_current_available_age`.
- `replica_set_row_includes_desired_and_age`.
- `job_row_includes_duration_and_age` — duration derived from `status.start_time` + `status.completion_time`.
- `cron_job_row_includes_last_schedule_and_age` — `last_schedule_time` is the closest analogue (not a v1_36 guarantee, so the cell is `Cell::text` with the timestamp rendered via `humanize_age` if present, else `Cell::text("-")`).
- `workload_kind_labels_match_mockup` — pin the 7 label strings.

Total: 4 existing + ~15 new = ~19 tests in `tests/workloads.rs`.

**`tests/resource_table.rs`** (new):

- `namespace_filter_returns_all_for_empty_set` — selected = ∅ → all rows pass.
- `namespace_filter_keeps_only_selected_namespaces` — selected = {`default`} → 2/3 rows.
- `namespace_filter_keeps_rows_in_either_of_many` — selected = {`default`, `kube-system`} → 3/3 rows.
- `namespace_filter_drops_orphaned_namespaces` — selected = {`ghost`} → 0 rows.
- `namespace_filter_handles_none_namespace_rows` — cluster-scoped resources (None ns) are only kept when selected is empty (the "All" sentinel).
- `namespace_filter_dedupes_within_a_kind` — same ns appears 5 times in `rows`; output preserves all 5.

The `namespace_filter` free fn is the unit under test; no Dioxus runtime, no `Dioxus` import. Same pattern as `tests/workloads.rs`.

### View-level tests (deferred)

The view itself (the rsx tree) is harder to test. Per the openkite-dev skill, "defer to manual smoke against k3d" — the CI build gates the `cargo build --release` step which exercises the Dioxus codegen. The `tests/resource_table.rs` covers the filter logic, which is the only non-trivial runtime behaviour. The view mount (`src/views/workloads.rs:80-106`) and the `workload_table!` macro (`src/views/workloads.rs:20-49`) are simple enough that the build-time check is sufficient.

### k3d manual smoke checklist (post-CI-green)

1. `k3d cluster start` (or use the existing dev cluster).
2. `cargo run --release` — the app launches, the Workloads view loads.
3. Click each of the 7 tabs — the table populates with the cluster's resources.
4. Click 2 namespace chips — only the union shows.
5. Type into the search field — filter narrows.
6. Click a column header — sort toggles; verify numeric and text columns both.
7. Resize the window narrow — the `.table-wrap` overflow scrolls; the toolbar wraps to a new row.
8. Disconnect the cluster (kill the kubeconfig entry) — the per-kind tables show the empty state without crashing.

## 8. Implementation steps in PR-friendly chunks

Each chunk is a commit, each compiles, each ends with `cargo fmt --check` (and `cargo clippy -- -D warnings` where the local toolchain allows — link-stage build is local-impractical per the skill).

### Chunk 1 — `Cell::html` variant + `age_cell` + `humanize_age` (foundation, ~50 LoC)

- `src/components/resource_table.rs` — add `Cell::html { html: &'static str, sort_text: String, sort: SortKey }` variant; extend `compare_sort_keys` (no change — the variant's `sort` field is what the comparator already uses).
- `src/workloads.rs` — add `age_cell` and `humanize_age` (no Dioxus import).
- `tests/workloads.rs` — add `age_cell_formats_*` tests + `age_cell_returns_dash_for_none_timestamp`.

**CI gate**: `cargo fmt --check`; the new test covers the helper. No view changes yet — the test asserts `Cell::html` round-trips through the `Cell` enum's `PartialEq` derived impl.

### Chunk 2 — Pod health dots + Controller/Node/QoS cells (Pod row only, ~80 LoC)

- `src/workloads.rs` — add `pod_health_dots(pod) -> Cell`, `controller_for_pod(pod) -> Cell`, `node_cell(pod) -> Cell`, `qos_cell(pod) -> Cell`.
- Extend `pod_columns()` (L98-113) and `pod_row()` (L116-132) to the 9-cell layout per the §3 table.
- `tests/workloads.rs` — add `pod_row_emits_nine_cells_with_mockup_layout`, `pod_health_dots_*`, `controller_for_pod_*` tests. Update the existing `pod_row_*` tests to expect 9 cells.

**CI gate**: existing tests + new ones pass. The view still renders 4 cells per pod row at runtime (until Chunk 6), so no user-visible behaviour change.

### Chunk 3 — Deployment/STS/DS/RS column expansion (~120 LoC)

- `src/workloads.rs` — extend `deployment_columns()`/`deployment_row()`, `stateful_set_*`, `daemon_set_*`, `replica_set_*` to the new layouts in §3.
- Add `controller_for_deployment` / `_for_stateful_set` / `_for_daemon_set` / `_for_replica_set` and a private `format_controller(refs: &[OwnerReference]) -> String` shared helper.
- `tests/workloads.rs` — add per-kind row tests.

**CI gate**: existing + new tests pass. The Deployment/STS/DS/RS rows in the live view update at runtime but the other 4 kinds stay as-is until Chunks 4-5.

### Chunk 4 — Job + CronJob column expansion (~60 LoC)

- `src/workloads.rs` — extend `job_columns()`/`job_row()` to add Duration and Age; extend `cron_job_columns()`/`cron_job_row()` to add Last schedule and Age.
- Job duration: `status.completion_time - status.start_time` formatted as `Xs`/`Xm`/`Xh` (uses the same `humanize_age` helper as the age cell, with a sign-flip for `start > now` edge cases).
- CronJob last schedule: `status.last_schedule_time` → `Cell::text` rendered via `humanize_age` (or `Cell::text("-")`).
- `tests/workloads.rs` — add per-kind row tests; pin the `Cell::text` content for the new cells.

**CI gate**: existing + new tests pass.

### Chunk 5 — `name_status_columns` signature change + view-side acceptance

- `src/workloads.rs` — change `name_status_columns(extra)` to `name_status_columns(middle, suffix)` (L53-70). Update all 7 callers.
- `src/views/workloads.rs` — no change (the 7 `*Table` components call `*_columns()` which now returns the new shape; the `ResourceTable` consumes the columns verbatim).
- `tests/workloads.rs` — update `pod_row_*` and `deployment_row_*` cell-count assertions if not already done in Chunks 2-4.

**CI gate**: every `*_columns()` call site compiles; every `*_row()` cell-count test passes.

### Chunk 6 — Multi-select namespace chips in `ResourceTable` (~70 LoC)

- `src/components/resource_table.rs` — change `namespace: Signal<Option<String>>` (L206) to `namespace: Signal<HashSet<String>>`. Update the `view` filter (L216-221) to call a new free fn `namespace_filter`. Update the chip rendering (L308-322) to multi-select, prepend the "All" sentinel chip, and switch the class to `.chip`.
- Extract `pub fn namespace_filter(rows: &[ResourceRow], selected: &HashSet<String>) -> Vec<ResourceRow>` to the top of the module.
- `tests/resource_table.rs` (new) — 6 tests per §7.

**CI gate**: existing `tests/resource_table.rs` (none yet — file is new) + `tests/workloads.rs` + the rest of the suite stays green. The view behaviour changes here — the user can now select multiple namespaces.

### Chunk 7 — Final pass: `cargo fmt`, contract test, doc-comment cleanup

- `cargo fmt --all` to settle any macro whitespace the `#[component]` annotators left behind.
- Strip stale OKT-N refs from module doc-comments in `src/workloads.rs` (per the openkite-dev "stale module-doc after merge" gotcha and the `clean-code-comments` skill's "drop ticket refs from comments" rule).
- `git diff` review of every `*_columns` + `*_row` pair, the new `tests/resource_table.rs`, and the `ResourceTable` `namespace` signal change.
- Push → `gh pr create --body-file .hermes/plans/okt33-workload-views-PR-description-draft.md` → `gh pr checks <N> --watch --interval 15` (bg + notify_on_complete).

### Acceptance criteria (mapped from the OKT-33 ticket)

| Ticket requirement                                                              | Where it lands                                  |
|---------------------------------------------------------------------------------|-------------------------------------------------|
| 7 workload views: Pods, Deployments, STS, DS, RS, Jobs, CronJobs                 | Existing `*Table` components in `src/views/workloads.rs` (untouched) |
| Per-kind columns (mockup-aligned)                                               | Chunks 2-5                                      |
| Status from workload row mapping (OKT-10)                                       | Already on `main`; re-exported via `*_row()`    |
| Plane ticket → PR body markdown URL                                             | `PR-description-draft.md`                      |
| CI green                                                                        | Chunk 7 — final pass                            |
| Tests added                                                                     | Chunks 1, 2, 3, 4, 6                           |

## 9. Risks and gotchas

### 1. Dioxus rsx! let-binding (HARD, OKT-31 took 3 CI cycles)

The skill documents the `rsx!` macro's narrow parser — `let` with method-call / `as`-cast / nested-call RHS fails. OKT-33's views are mostly **unchanged** (the `workload_table!` macro at `src/views/workloads.rs:20-49` and the dispatch `match` at L95-103). The only view-side change is in `ResourceTable`:

- The chip loop (L254-256) currently does `for (label, active) in chips.into_iter()`. With multi-select, the third element is `selected: bool` (a set membership), not a computed `active` boolean. **Precompute** the chip tuples outside `rsx!`:
  ```rust
  // Precompute outside rsx! — skill rule "no let in rsx! with method-call RHS".
  let chips: Vec<(String, bool)> = all_namespaces
      .iter()
      .map(|ns| (ns.clone(), namespace().contains(ns)))
      .collect();
  ```
- The chip click closure: `onclick: move |_| { let mut s = namespace; if s.read().contains(&label) { s.write().remove(&label); } else { s.write().insert(label.clone()); } }`. The label is captured by move; the read-guard borrow dies before the closure body executes, so `label.clone()` inside the closure is mandatory (skill: "FnMut onclick closure + body interpolation of the same captured ns").
- **Escape valve (per skill)**: if clippy fails twice on the same `ResourceTable` region, stop patching; re-read the file with post-fmt line numbers, identify the systemic issue (likely the `namespace: Signal<HashSet<String>>` change rewrote the chip handler's borrow graph), and rewrite the chip block as one coherent fix.

### 2. `Cell::html` and the rsx! parse

The `Cell::html` variant carries a `&'static str` for the markup. The view (`ResourceTable::render_table_cell`, `resource_table.rs:382-401`) does a `match` over `cell.status` and renders via `StatusBadge` or `<span>{cell.text}</span>`. Adding a third arm that calls `rsx! { div { class: "table-cell", {cell.html} } }` is fine — but **the macro must parse `{cell.html}` as a single expression**, not a method call. The clean shape is:

```rust
Some(kind) => rsx! { div { class: "table-cell", StatusBadge { status: kind } } },
None => rsx! { div { class: "table-cell", span { "{cell.text}" } } },
// New:
```

Actually, the `html` field is best surfaced as a **rendered Dioxus element** (an `Element`), not a `&'static str`. That way the per-kind row mapper in `src/workloads.rs` does NOT need a Dioxus import (which the skill forbids) — instead, the `Cell::html` variant carries a `Cell::html { sort: SortKey, label: String, render: fn(&Cell) -> Element }`. The 7 row mappers pass a thin function pointer that builds the dot row.

This stays out of `src/workloads.rs`'s imports (the function pointers are `fn` types, no `dioxus::prelude` needed in the row mapper — the `Element` type alias from `dioxus_core` is the only `dioxus` dep, which is fine because `kube::runtime` already pulls in `dioxus_core` transitively through… actually, no, `kube` does not pull `dioxus_core`. The function-pointer shape still requires the `Element` type to be in scope, so the cell-builder functions live in `src/components/resource_table.rs` and `src/workloads.rs` calls them as free fns.

Final shape (revised plan):

- `Cell::html { sort: SortKey, label: String, render: fn() -> Element }` — lives in `src/components/resource_table.rs`.
- The Pod row in `src/workloads.rs` calls `pod_health_dots_cell(pod)` which returns `Cell::html { sort: SortKey::Number(2.0), label: "2/2".into(), render: pod_health_dots_render }`. The function pointer is `fn() -> Element` and is built inline in the same file as a `pub fn` that has `use dioxus::prelude::*;` in scope.
- `src/workloads.rs` itself does NOT import `dioxus::prelude::*;` — the type is just `Cell` from `crate::components::resource_table`.

This is the same split the OKT-29 design system uses (`src/design/tokens.rs` is pure constants; `assets/main.css` is the consumer).

### 3. Age sort key edge case (`NaN` filter)

`age_cell` returns `Cell::number("2d", seconds_since_creation)`. Pods with `creation_timestamp = None` should sort last. `f64::NAN` is filtered by the comparator (`compare_sort_keys` already orders numbers before text; NaN breaks `total_cmp`'s invariants). **Solution**: use `Option<f64>` as the numeric sort key, with a private "missing" sentinel. Simplest: return `Cell::text("-")` with a `SortKey::Text("\u{ffff}".into())` (sentinel sorts after all real text). Or extend `SortKey` with a `Missing` variant — the cleanest fix but a bigger refactor.

**Decision**: use the sentinel `SortKey::Text("\u{ffff}".into())` for missing age. One line, no `SortKey` change, documented in a `///` comment.

### 4. kube-rs Api::namespaced vs Api::all

The current `workload_table!` macro (L20-49) uses `Api::<$ty>::all(client)` (L30). All 7 kinds in this PR remain cluster-wide (no per-namespace scoping at the kube level — the per-namespace filtering happens in the view via the multi-select chip). The `Api::all` vs `Api::namespaced` distinction is **not relevant** to OKT-33; the `T::Resource<Scope = NamespaceResourceScope>` constraint (per the skill) is satisfied by all 7 kinds today. **No change.**

### 5. k8s-openapi v1_36 optionality differences

The skill flags three v1_36 specifics — verified before this plan landed:

- **`CronJob.spec` is a bare `CronJobSpec`** (not `Option`) — `src/workloads.rs:403` already deref's it directly (`cj.spec.schedule.clone()`). No change.
- **`ContainerState` has only `running`/`waiting`/`terminated`** — `src/pod.rs:32-52` already reads only those three. No change.
- **`metadata.creation_timestamp` is `Option<Time>`** in v1_36 (every workload kind) — `age_cell` handles `None` via the sentinel. No change.

### 6. Reflector leak on context switch

`workload_table!` spawns a `tokio::spawn(drive_reflector(...))` on every effect re-run. The current code does not `abort()` the prior task on `use_effect` re-entry — if the user switches the kube context, the old reflector continues to publish into the same signal. **This is a pre-existing bug** on `main`; it is **not** an OKT-33 concern but is worth a `// TODO` comment in `src/views/workloads.rs:26-39` to flag the follow-up. Tracked in a separate ticket (not OKT-33).

### 7. Existing `RowActions` wiring on `main`

`RowActions` (`resource_table.rs:178-183`) is declared but **not yet wired** by the workload views (the workload `*Table` components pass `row_actions: None` implicitly). OKT-43 (CRUD UI) wires the handlers. **No OKT-33 change.**

### 8. `tests/resource_table.rs` — the `Cargo.toml` test layout

The existing `Cargo.toml` test infrastructure (lints/clippy/test gates in `.github/workflows/lint-test.yml`) picks up any `tests/*.rs` automatically. No `Cargo.toml` change. The skill's "Foundation tests must NOT import modules from sibling (un-merged) PR branches" gotcha does not apply — `tests/resource_table.rs` imports only `openkite::components::resource_table` (already on `main`).

## 10. Skill rules applied (per `openkite-dev` SKILL.md)

- **Foundation-first workflow**: pure logic (`Cell::html`, `age_cell`, `controller_*`, `namespace_filter`) in its own chunks before the view consumes them. View chunks compile but render the old layout until the column/row chunks land.
- **CI-as-compiler loop**: push → `gh pr checks --watch --interval 15` (bg + notify) → fix → push → repeat. Each chunk ends at a green `cargo fmt --check`.
- **PR description must follow the template verbatim**: see the `PR-description-draft.md` for the literal `### Relevant Plane Tickets:` plural heading and all six checkboxes.
- **Workitem UUID verified via `mcp__plane__workitem` action=list**: `1aa27b65-488c-48a0-98b3-668b5fa876a3` (sequence_id=33, `phase-2` label, Backlog state). Confirmed in the PR body as a markdown URL.
- **Split pure logic from the Dioxus view**: `src/workloads.rs` stays free of `dioxus::prelude::*;`. The `Cell::html` variant's `render: fn() -> Element` is built in `src/components/resource_table.rs` (which has the glob import).
- **No `let` with method-call RHS in `rsx!`**: precompute `chips: Vec<(String, bool)>` outside the loop, as per the OKT-31 escape valve.
- **Comment hygiene**: drop OKT-N refs from `//!` doc-comments. Drop forward-looking language ("the interactive remainder", "not yet wired") from existing module docs in `src/workloads.rs` if any survived the P1 cycle.
- **No deps fabricated**: `k8s-openapi` 0.28, `kube` 4, `dioxus` 0.7, `jiff` 0.2, `serde-saphyr` 0.0.29, `opaline` 0.4, `notify` 8, `tempfile` 3 — all read from `Cargo.toml:18-49`. No new dependencies in OKT-33.

## 11. PR description (separate file)

See `okt33-workload-views-PR-description-draft.md` in the same directory. The draft follows `.github/pull_request_template.md` verbatim (verified at PR template `## 📚 Description` / `## 🔍 Types of Changes` / `## ✅ Checklist` headings). The workitem hyperlink is the verified `1aa27b65-488c-48a0-98b3-668b5fa876a3` UUID. Before `gh pr create --body-file`, run the skill's recommended `diff <(grep -E '^### |^## ' .github/pull_request_template.md) <(grep -E '^### |^## ' .hermes/plans/okt33-workload-views-PR-description-draft.md)` to catch any template drift between plan-time and PR-time.

## 12. Open follow-ups (not in this PR)

| Item | Why deferred | Where it lands |
|------|--------------|----------------|
| `.status-badge` → `.pill` migration | `ResourceTable::render_table_cell` (`resource_table.rs:382-401`) renders `StatusBadge`; the new design-system class is `.pill` (OKT-29). Single-source-of-truth rename needs a follow-up that touches every view consumer. | OKT-43 (CRUD UI) |
| Per-kind 7-route deeplink (`/workloads/pods`, …) | Tab strip is the deeplink surface for now. JS plugins and OKT-34 inspector links will surface the need. | Post-OKT-43 |
| Per-row actions (`on_delete`, `on_edit`, `on_scale`) | `RowActions` struct already exists (`resource_table.rs:178-183`); handlers are stubbed. | OKT-43 (CRUD UI) |
| Pod detail (inspector with container list) | `src/pod.rs:58-83` already maps `Pod → Vec<ContainerInfo>`. Inspector + click handler. | OKT-34 |
| Per-cell live watch (status dot pulses) | Different cadence than the current reflector+signal. | OKT-40/41 |
| Abort prior reflector on context switch | Pre-existing leak; out of scope. | A follow-up UI ticket |
| Pod `controller` cell renders the `Kind` with an icon | The mockup's `.resource-name .icon` is a per-kind SVG. The icon sprite is OKT-47 territory (per the OKT-29 plan, skill §"Icon sprite"). | OKT-47 or follow-up |
