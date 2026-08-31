# OKT-32 — Resource table component (P2 UI wrapper)

**Ticket:** [OKT-32 — Resource table component — virtualized, sort, filter](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/387fc7b8-226c-4793-a31d-c4aa6da23f42)
**Branch:** `feat/okt32-resource-table` (from `origin/main`)
**PR title:** `feat(openkite): resource table component (P2 UI)`
**Workitem UUID (PR body hyperlink):** `387fc7b8-226c-4793-a31d-c4aa6da23f42`
**Status (start):** Ticket is **In Progress** (move to Done on merge).

## 1. Scope

### What ships in this PR (P2 visual wrapper on top of P1 foundation)

The P1 work is already in the repo. This PR re-skins the existing virtualized table to use the Liquid Frost Glass design system primitives (OKT-29, merged at `bd67b14`) and adds the P2 affordances the ticket text calls out: visible **sort indicators** in the column headers, the **`.search-field` filter input**, **`.pill` + `.health-dots`** status rendering, and a stubbed **row-click callback** the OKT-34 pod-detail slide-over will wire in.

Concretely, in `src/components/resource_table.rs`:

- `render_table_cell` (`src/components/resource_table.rs:382-402`) now renders a status cell as the new `.pill` markup (and an inline `.health-dots` row when `Cell` carries container/instance health), not the legacy `.status-badge` (`src/components/status_badge.rs:56-60`).
- `header_cell` (`src/components/resource_table.rs:276-305`) renders a visible **sort arrow** (▲/▼) tied to the active `(column, direction)` state and uses the design-system typography/spacing tokens for the header row.
- The `<input class="table-filter" …>` (currently `src/components/resource_table.rs:248-253`) is wrapped in the design-system `.search-field` (a flex container with the 44px touch target from `assets/main.css:242-261`).
- The view is mounted inside the `.table-wrap` + `.panel` (the **opaque-card-free** container from `assets/main.css:174-183` and `:307-313`) so the table sits on a frost surface, not the page background.
- A new **`on_row_click: EventHandler<ResourceRow>`** prop is added to `ResourceTable`. Wiring to the OKT-34 slide-over is deferred — for now it logs to `tracing::info!` and stays in the same callback signature the pod-detail view will expect.
- The `namespace_chip` filter (current `src/components/resource_table.rs:308-322`) keeps the existing `.ns-chip` rule for now — `.chip` is the design-system successor and will replace it in a follow-up that touches the whole sidebar/topbar (out of scope: touching sidebar/topbar breaks parallel view tickets).

### What is explicitly deferred (consumed by dependent tickets)

- **Row click → pod detail slide-over.** The `on_row_click: EventHandler<ResourceRow>` callback is wired but its consumer is a `tracing::info!` log call. OKT-34 (Pod detail slide-over) replaces the log with the `Inspector` open call. This split keeps the table independent of the inspector, which is itself a pure-CSS primitive from OKT-29 (`assets/main.css:362-396`).
- **Per-kind column customisation** (Pods get a Health column with `.health-dots`, Deployments get Ready/Available, Jobs get Completions, etc., per mockup L189-260). Today every kind gets the same `[name, status, ready, restarts, …]` set from `src/workloads.rs:98-112`. **OKT-33** widens the column model so each workload kind supplies its own `ColumnDef` set + per-cell render hooks. OKT-32 only adds the **generic visual surface** (`.table-wrap`, `.panel`, sort indicators, `.pill` status, `.search-field`).
- **Status drilldown** (clicking a status cell → "show me all Failed pods"). This is OKT-35+ (per Phase-2 ticket map in the openkite-dev skill §Project phases).
- **Mutations** (delete/edit/scale) — `RowActions` already exists (`src/components/resource_table.rs:178-183`) but its consumer is OKT-43 (CRUD UI). The `on_row_click` callback co-exists with the existing `RowActions` API; they target different cells.
- **Argocd / per-plugin table themes** — OKT-47. The host table uses opaline + the design-system tokens; a plugin-specific accent (e.g. `--argo` for the ArgoCD row) is a follow-up.
- **Dioxus component wrappers for design-system primitives** (`<Panel>`, `<Button>`, `<Toast>`, `<Inspector>`) — already deferred by OKT-29 (the design system is **CSS + Rust constants only**, no `#[component]`s). OKT-32 follows the same foundation-first posture: it changes the table's render output, it does not introduce a new `#[component]` library.

### Acceptance criteria (mapped from ticket)

| # | Criterion (from ticket) | How met |
|---|---|---|
| 1 | "Generic virtualized resource table component" | `ResourceTable` (`src/components/resource_table.rs:195-273`) is generic over `Vec<ResourceRow>` — already virtualized (windowed via `visible_range` at `src/components/resource_table.rs:80-90`); OKT-32 only re-skins |
| 2 | "wired to ResourceState<T> reflectors" | The Workloads view at `src/views/workloads.rs:23-48` already uses `use_signal_sync` + `drive_reflector` to push rows; the new component is a drop-in |
| 3 | "sortable columns" | `sort_by_key` (`src/components/resource_table.rs:61-72`) + per-column `toggle_sort` (`:186-192`) already work; OKT-32 adds the visible ▲/▼ indicator in `header_cell` |
| 4 | "filter input" | `matches_query` (`:75-78`) is already there; OKT-32 wraps the input in the design-system `.search-field` container |
| 5 | "status pills" | Switch the existing `StatusBadge` (`src/components/status_badge.rs:56-60`) to the design-system `.pill` + `.health-dots` markup using the OKT-29 primitives |
| 6 | "row click → pod detail" | New `on_row_click: EventHandler<ResourceRow>` prop; OKT-34 wires it to the Inspector; the prop is plumbed now so OKT-34 is a one-line change |

## 2. File structure

The whole change lives inside the **existing** `src/components/resource_table.rs` plus one new integration test file. No new module, no `lib.rs` change, no new dependency.

```
src/
├── components/
│   ├── mod.rs                       # Unchanged (`pub mod resource_table;` at L3)
│   └── resource_table.rs            # EDIT. Re-skin to design-system primitives.
│                                    # Add `on_row_click: EventHandler<ResourceRow>`
│                                    # prop to `ResourceTable`. Switch
│                                    # `render_table_cell` from `.status-badge`
│                                    # to `.pill` + `.health-dots`. Wrap the
│                                    # filter `<input>` in `.search-field`.
│                                    # Wrap the table root in `.panel >
│                                    # .table-wrap > .resource-table`. Keep
│                                    # all P1 logic (`sort_by_key`, `visible_range`,
│                                    # `matches_query`) byte-for-byte.
├── status_badge.rs                  # Unchanged. `StatusKind` enum + mapping
│                                    # stay — only the rendering moves to the
│                                    # table cell.
└── views/workloads.rs               # Unchanged. Mounts `ResourceTable` already.

tests/
└── resource_table.rs                # NEW. ~60 lines. Asserts:
                                     # - sort_by_key / visible_range /
                                     #   matches_query / Cell constructors
                                     #   still work (re-export the existing
                                     #   unit-test surface for cross-PR
                                     #   visibility — same pattern as
                                     #   tests/workloads.rs:1-97).
                                     # - The CSS contract still pins the
                                     #   `.pill` / `.health-dots` / `.search-field`
                                     #   classes on `ResourceTable` (per
                                     #   `tests/design.rs:41-64`).

assets/main.css                      # NO edit. The design-system tokens and
                                     # primitive classes from OKT-29 (merged)
                                     # are reused as-is: `.panel` (L174-183),
                                     # `.pill` (L263-298), `.health-dots`
                                     # (L300-305), `.search-field` (L241-261),
                                     # `.table-wrap` (L307-313), `.resource-name`
                                     # (L313-320), `.dot` + `.ok`/`.warn`/`.err`
                                     # (L301-305). The contract test
                                     # (`tests/design.rs`) already pins them.
```

### Why no new module (and why no new components/resource_table/ subdir)

- The P1 table is already a single file (`src/components/resource_table.rs:608` lines). Splitting P2 into a directory would create `mod.rs` + `view.rs` + `logic.rs` for a 50-line edit. **Ponytail principle**: a subdirectory is the right call only when the file gets unwieldy; today it doesn't.
- `src/components/{mod,status_badge,theme_selector,resource_table}.rs` is the established pattern (verified by `src/components/mod.rs:1-5`).
- The skill §Dioxus 0.7 patterns rule "Split pure logic (no dioxus import) from the Dioxus view (glob import) into separate modules" is already satisfied: `sort_by_key`, `compare_sort_keys`, `matches_query`, `visible_range`, `RowActions`, `Cell`, `ResourceRow`, `ColumnDef`, `TableStatus` (all in the same file) are pure-logic; the `#[component]`s are the Dioxus side. No new split is needed.

### Why no new Cargo deps

- `dioxus = "0.7"` (`Cargo.toml:19`) is the only UI dep; `EventHandler` is already in `dioxus::prelude::*`.
- The new `EventHandler<ResourceRow>` prop reuses the existing `EventHandler` import in `src/components/resource_table.rs:15` (`use dioxus::prelude::*;`).
- The `.pill` / `.health-dots` / `.search-field` classes are pure CSS (`assets/main.css:241-305`); no Rust side.

## 3. Token / CSS contract

OKT-32 is **consume-only** — it uses the OKT-29 contract verbatim. No new tokens, no new classes, no `src/design/tokens.rs` changes. The contract test (`tests/design.rs:73-121`) already pins:

- 12 new custom properties (`:23-35`): `--brand`, `--argo`, `--on-accent`, `--font-sans`, `--font-mono`, `--shadow-rest`, `--shadow-hover`, `--shadow-terminal`, `--r-sm`, `--r-md`, `--r-pill`.
- 22 primitive classes (`:41-64`): `.panel`, `.btn`, `.btn-primary`, `.btn-secondary`, `.chip`, `.search-field`, `.pill`, `.pill.success`, `.pill.warn`, `.pill.danger`, `.table-wrap`, `.resource-name`, `.log-panel`, `.log-line`, `.inspector`, `.kv-list`, `.toast`, `.nav-section`, `.dot`, `.dot.ok`, `.dot.warn`, `.dot.err`.

This PR **adds nothing to that contract**. The new test `tests/resource_table.rs` reads the same `STYLESHEET` constant the design contract pins (re-uses `include_str!("../assets/main.css")`) and asserts the table render path references the right class names — but at the **Rust source level** (string match on `src/components/resource_table.rs` body), not the rendered DOM. Reason: the existing `tests/design.rs` already does the CSS contract; this PR's test exists to catch a regression where someone deletes the `.pill` markup from the table cell renderer and the test would only see the missing CSS if they also deleted the class rule.

### Class usage in the new render path

| Element | New class | Existing class | Source line (post-PR) |
|---|---|---|---|
| Table root | `.panel` | (none) | `src/components/resource_table.rs` `rsx!` in `ResourceTable` |
| Table-wrap div | `.table-wrap` | (none) | same |
| Table grid | `.resource-table` | `.resource-table` | same |
| Filter input container | `.search-field` | (none) | new wrapper around `input` at `:248-253` |
| Filter input | (unchanged) | `input` (untouched) | `:252-253` |
| Header row | `.table-header` | `.table-header` | `:258` |
| Header cell | `.table-cell` | `.table-cell` | `:288-289` |
| Sort indicator | `.sort-indicator` | (none) | new `<span class="sort-indicator">` at `:298-301` |
| Status cell (replaces `StatusBadge`) | `.pill` + variant | `.status-badge` | `render_table_cell` at `:382-402` |
| Health dots inline (future use) | `.health-dots` + `.dot` + `.ok`/`.warn`/`.err` | (none) | new helper component for Pod-row `health` cell |
| Body row | `.table-row` | `.table-row` | `:369` |
| Namespace chip | `.ns-chip` | `.ns-chip` | unchanged (`:312-313`); `.chip` migration is a sidebar/topbar follow-up |

The `StatusKind` enum (`src/components/status_badge.rs:13-24`) is **kept** — its `class()` method (`status_badge.rs:28-35`) is reused to derive the `.pill.success`/`.pill.warn`/`.pill.danger` variant. The `StatusBadge` component (`status_badge.rs:55-60`) is **not deleted**; it's deprecated by the cell renderer switch but the contract test (`tests/design.rs:38-40`) explicitly pins the `.status-badge` class as a transitional alias. Removing it is a separate cleanup after consumers (if any) migrate.

## 4. Component shape

### `ResourceTable` — current P1 signature (kept)

From `src/components/resource_table.rs:195-203`:

```rust
#[component]
pub fn ResourceTable(
    columns: Vec<ColumnDef>,
    rows: Vec<ResourceRow>,
    #[props(default)] status: TableStatus,
    #[props(default)] empty_message: Option<String>,
    #[props(default)] row_actions: Option<RowActions>,
    #[props(default = 600.0)] height: f64,
) -> Element
```

### `ResourceTable` — P2 signature (add one prop)

```rust
#[component]
pub fn ResourceTable(
    columns: Vec<ColumnDef>,
    rows: Vec<ResourceRow>,
    #[props(default)] status: TableStatus,
    #[props(default)] empty_message: Option<String>,
    #[props(default)] row_actions: Option<RowActions>,
    #[props(default)] on_row_click: Option<EventHandler<ResourceRow>>,
    #[props(default = 600.0)] height: f64,
) -> Element
```

- `EventHandler<ResourceRow>` (not `EventHandler<String>`) so the OKT-34 slide-over receives the full row (id + namespace + cells) without a second lookup pass.
- `Option<…>` (not required) so existing call sites (`src/views/workloads.rs:42-46` and the macro at `:20-48`) compile unchanged. Workload views in OKT-32 do **not** wire a click handler; the prop plumbs in for OKT-34.
- `EventHandler` is `Copy` (per skill §Dioxus 0.7 patterns, "EventHandler/Callback is Copy … don't .clone()") → pass it bare in rsx, never `.clone()` it.

### New internal `StatusPill` helper

A small `#[component] fn StatusPill(kind: StatusKind)` lives in `src/components/resource_table.rs` and replaces the existing inline `StatusBadge` call at `:390`. The helper maps:

| `StatusKind` | Class | Label (from `StatusKind::label()` at `status_badge.rs:38-51`) |
|---|---|---|
| `Running` / `Ready` / `Succeeded` | `.pill.success` | "Running" / "Ready" / "Succeeded" |
| `Pending` / `OutOfSync` | `.pill.warn` | "Pending" / "OutOfSync" |
| `Failed` / `CrashLoop` / `Degraded` | `.pill.danger` | "Failed" / "CrashLoopBackOff" / "Degraded" |
| `Unknown` / `Suspended` | `.pill.muted` | "Unknown" / "Suspended" |

(The "CrashLoop" label discrepancy: `StatusKind::label()` returns "CrashLoop" (no "BackOff" suffix), which matches the existing component's text. OKT-34 may rename when the pod detail is wired; OKT-32 keeps parity.)

### New `HealthDots` helper (deferred consumer, but ships the helper)

A `#[component] fn HealthDots(ready: u32, total: u32)` renders the mockup's `health-dots` pattern (mockup L199-200, L210-211: `<span class="health-dots"><span class="dot ok"></span><span class="dot ok"></span></span>`). It iterates a `Vec<DotClass>` of length `total`, the first `ready` are `.dot.ok`, the rest are `.dot.err` (or `.dot.warn` for 1-ready-when-2).

**OKT-32 ships the helper; it does NOT wire it into any column.** Today's `pod_columns` (`src/workloads.rs:98-113`) does not include a Health column — that's OKT-33's per-kind column work. The helper is exported from `resource_table` so OKT-33 (and the eventual `health: HealthDots { ready, total }` cell variant) has a ready-to-use component.

### Row click wiring

`render_table_row` (`src/components/resource_table.rs:357-379`) adds a single `onclick: move |_| on_row_click.map(|h| h.call(row.clone()))` to the row `<div>`. The closure:

- **Clones** the `row` into the move closure (skill §Dioxus 0.7: "for move closures in onclick, clone owned values; never capture references").
- **Maps** over the `Option<EventHandler<…>>` because `EventHandler::call` is a method on the wrapped `Fn`, not on `Option<EventHandler>`. Mapping at call site is the cheapest way to no-op when the prop is `None`.
- **Does not** pass `on_row_click` as a captured `&` — it's `Copy`, so the move closure captures it by value, and the rsx body keeps a separate copy for the next row's closure (each iteration creates a fresh `move |_|`).

This pattern avoids the E0507 "use after move" trap verified on OKT-31 (skill §Dioxus 0.7: "FnMut onclick closure + body interpolation of the same captured `ns` — when `onclick: move |_| toggle(ns)` and the rsx! body also writes `\"{ns}\"`, the body consumes one `ns` and the move closure body tries to consume another → E0507"). Here: the `row` is consumed in the closure; the outer for-loop holds a `&ResourceRow` (`:348` iterates `slice.iter()`), and `render_table_row` takes `row: &ResourceRow` (`:358`) so the next iteration just borrows fresh.

### Sort indicator UI

`header_cell` (`src/components/resource_table.rs:276-305`) adds the visible arrow:

```rust
span { class: "sort-indicator", if dir == SortDirection::Ascending { "▲" } else { "▼" } }
```

…where `dir` is already computed (`:285`):

```rust
let direction = sort().and_then(|(active, dir)| (active == index).then_some(dir));
```

The `sort-indicator` class is new CSS — see §3. No `.sort-indicator` rule ships in this PR (the existing `.status-dot` style at `assets/main.css:139` is a close visual cousin but a separate concept; OKT-32 adds the rule inline in `assets/main.css` if the contract test is extended, or relies on the existing `.table-cell` text styles to render the glyph). **Decision: add a minimal `.sort-indicator` rule to `assets/main.css`** — `font-size: 10px; color: var(--fg-2); margin-left: 4px;` — and extend `tests/design.rs:41-64`'s `REQUIRED_CLASSES` list. Two-line change, contract test still green.

## 5. Implementation steps (PR-friendly chunks)

Each step ends at a green `cargo fmt --all -- --check` and (where applicable) `cargo clippy --all-targets -- -D warnings` locally; the CI workflow `lint-test.yml` (fmt → clippy → test → build) is the merge gate (skill §CI-as-compiler loop). Link-stage builds are SKIPPED locally (no `glib-2.0`); CI is the build authority.

### Chunk A — Add the `on_row_click` prop (zero behaviour change)

1. In `src/components/resource_table.rs:195-203`, add `#[props(default)] on_row_click: Option<EventHandler<ResourceRow>>,` as the 6th prop (between `row_actions` and `height`).
2. In `render_table_row` (`:357-379`), add a `let on_row_click = on_row_click;` rebind and an `onclick: move |_| if let Some(h) = on_row_click { h.call(row.clone()) }` on the row `<div>`. `Option::if let Some(…)` does not need a method-call RHS, so it survives the rsx! parser (skill §Dioxus 0.7: "No `let` with method-call / `as`-cast / nested-call RHS inside `rsx!`").
3. **Push.** CI = fmt + clippy + test (no behaviour change; the prop is `None` for every existing call site).

### Chunk B — `.sort-indicator` CSS + extend the contract

4. Append a `.sort-indicator` rule to `assets/main.css` after the `.table-header` block (around `:257` — `main.css` has no explicit `.table-header` rule today, the class is used but unstyled). Add `.sort-indicator { font-size: 10px; color: var(--fg-2); margin-left: 4px; user-select: none; }` inside the table primitives block (`:307-330`).
5. Add `".sort-indicator"` to the `REQUIRED_CLASSES` list in `tests/design.rs:41-64` and to the test for "every_required_class_is_present" (it iterates the same list — no code change).
6. **Push.** CI = contract test green (the new class is in CSS + in the required list).

### Chunk C — Visible sort indicator + search-field wrapper

7. In `header_cell` (`src/components/resource_table.rs:276-305`): replace the bare label `<span>"{label}"</span>` (`:296`) with the inline arrow. The existing conditional at `:297-302` already renders the arrow text — just add `class: "sort-indicator"` to the span and a `if column.sortable { … }` gate so non-sortable columns (none today, but `ColumnDef::sortable: bool` is already a field at `:163`) don't show a dangling arrow.
8. In `ResourceTable` (`:245-257`): wrap the `<input class="table-filter" …>` in a `<div class="search-field">…</div>` (matches the design-system markup at `assets/main.css:242-261`). Rename `table-filter` → keep it (the input is fine, just a container class on the parent). **Do not** add the inline `::placeholder` styling — the global `assets/main.css:260` rule covers it.
9. **Push.** CI = visual smoke (no behaviour change, just markup).

### Chunk D — `.pill` status rendering

10. In `src/components/resource_table.rs:382-402` (`render_table_cell`): replace the `StatusBadge` call with a new inline `StatusPill` (or a `#[component] fn StatusPill(kind: StatusKind)` declared above the `render_table_cell` body). Markup: `<span class="pill {kind.pill_class()}">{kind.label()}</span>` where `StatusKind::pill_class()` is a new method on the existing `StatusKind` enum that maps to `"success"` / `"warn"` / `"danger"` / `"muted"` (re-using the existing `class()` table at `status_badge.rs:28-35`). Add `pub fn pill_class(self) -> &'static str` to `StatusKind` in `status_badge.rs` — keeps the cell renderer thin.
11. Update `src/components/status_badge.rs:28-35`: leave `class()` as-is (returning the legacy `.status-ok`/`.status-warn`/`.status-err`/`.status-muted` for the contract test). Add `pill_class()` next to it returning `"success"` / `"warn"` / `"danger"` / `"muted"`. The legacy mapping (ok→green, warn→yellow, err→red) is unchanged; only the prefix string changes.
12. **Push.** CI = fmt + clippy + test (existing tests at `tests/workloads.rs:39-72` still pass — they only check `StatusKind` enum values, not class strings).

### Chunk E — `.panel > .table-wrap` container + HealthDots helper (no live consumer)

13. In `ResourceTable` rsx! (`:245-270`): wrap the `<div class="resource-table">` in `<div class="panel"><div class="table-wrap">…</div></div>`. Both classes exist (`assets/main.css:174-183` and `:307-313`); no new CSS.
14. Add `#[component] fn HealthDots(ready: u32, total: u32) -> Element` after `StatusBadge` (or above `TableBody` in the same file). The body: `let dots: Vec<&'static str> = (0..total).map(|i| if i < ready { "ok" } else { "err" }).collect(); rsx! { div { class: "health-dots", for cls in dots { span { class: "dot {cls}" } } } }`. The `Vec<&'static str>` allocation is the same gotcha-prevention pattern as `Vec<(String, bool)>` from the skill §Dioxus 0.7: precompute outside the for loop.
15. **Push.** CI = full gate.

### Chunk F — `tests/resource_table.rs` + manual smoke checklist

16. New file `tests/resource_table.rs` (~60 lines). Asserts:
    - `sort_by_key` (text + number, both directions) — re-asserts the inline `#[cfg(test)] mod tests` at `src/components/resource_table.rs:440-607` from a public-API angle.
    - `compare_sort_keys` orders numbers-before-text and is case-insensitive.
    - `visible_range` is bounded for 10k rows and empty at zero.
    - `matches_query` is case-insensitive and blank-match-everything.
    - `Cell::{text, status, number}` set the right `sort` key.
    - `ResourceRow::search_text` includes namespace and every cell text.
    - A new test `status_kind_pill_classes_cover_every_variant` that calls `StatusKind::pill_class()` on all 10 variants and asserts each returns a string in `["success", "warn", "danger", "muted"]` (catches a new variant added to the enum but not mapped).
    - A new test `pill_class_mapping_matches_legacy_class` that asserts `pill_class()` and `class()` for the same variant are semantically equivalent (ok→success, warn→warn, err→danger, muted→muted). Catches the 2026-08-30-style "the prefix is right but the colour is wrong" drift.
17. **Push.** CI = full gate.
18. Manual smoke checklist (added to the PR description, not code):
    - `cargo run` and navigate to Workloads → Pods. The pods table renders inside a frost panel with a visible filter input (the new `.search-field`).
    - Type `nginx` in the filter — only the matching row remains.
    - Click the "Name" column header — arrow appears, rows reverse.
    - Click again — arrow flips to ▼, rows reverse.
    - The status column shows colored pills (green for Running, yellow for Pending, red for Failed).
    - Open the ArgoCD plugin route (OKT-47 follow-up); the sidebar/status bar still render (no regressions from the `src/router.rs` `StatusFooter` color routing).

## 6. Test plan

### Unit / integration tests (no kube, no JS)

- `src/components/resource_table.rs` inline `#[cfg(test)] mod tests` (`src/components/resource_table.rs:440-607`) — **kept** verbatim. Pin `sort_by_key`, `compare_sort_keys`, `visible_range`, `matches_query`, `Cell`, `ResourceRow::search_text` from the public API.
- `src/components/status_badge.rs` inline `#[cfg(test)] mod tests` (`src/components/status_badge.rs:62-92`) — **kept** + extended with `pill_class_*` cases.
- `tests/resource_table.rs` (new) — cross-PR visibility of the same logic, plus the new `StatusKind::pill_class` contract.
- `tests/design.rs` (existing, `tests/design.rs:73-173`) — extended with `.sort-indicator` in `REQUIRED_CLASSES`. The new class rule is asserted on every push.

### What is NOT in unit tests (and where it lives instead)

- **Visual smoke (frost panel + sort arrow + pill colour)**: CI does not run a browser. First verification on a dev machine via `cargo run`. The PR description has a "manual smoke" section.
- **Dioxus component rendering in isolation**: out of scope — `ResourceTable` is tested through the public `sort_by_key` / `Cell` / `ResourceRow` API. The Dioxus side is glue.
- **k8s / kube interactions**: none — the change is pure-CSS + enum mapping.

## 7. Risks and gotchas

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Dioxus rsx! let-binding gotcha** (skill §Dioxus 0.7: "No `let` with method-call / `as`-cast / nested-call RHS inside `rsx!`") | High | All `let` bindings inside `rsx!` use precomputed `Vec<T>` or `Option<T>` (where `T` is a plain local). The new `HealthDots` rsx! uses `for cls in dots { … }` where `dots: Vec<&'static str>` is computed before the macro (matches the PR #48 fix pattern). The row-click closure's `if let Some(h) = on_row_click { h.call(row.clone()) }` is a plain `if let`, not a `let`, so it survives the parser. |
| **Signal ownership in move closures** (skill §Dioxus 0.7) | Low | `on_row_click: EventHandler<ResourceRow>` is `Copy` (per skill §Dioxus 0.7: "EventHandler/Callback is Copy → don't .clone() it"). Pass bare in rsx. The row body closure captures by value via `row.clone()`; the outer for-loop holds `&ResourceRow` and re-borrows per iteration. |
| **GlobalSignal vs use_signal_sync for cross-thread use** | None | The change touches an existing component (`ResourceTable`) that does not spawn tasks. The Workloads view at `src/views/workloads.rs:23-48` already uses `use_signal_sync` for the rows signal; OKT-32 reads them the same way. |
| **clippy `unnecessary_sort_by` under `-D warnings`** (skill §Gotchas) | None | No new `.sort_by` in this PR; the existing `sort_by_key` at `src/components/resource_table.rs:65-71` uses `sort_by` not `sort_by_key` with `Reverse`, but it's a custom compare not a bare key + direction (so the lint doesn't fire). |
| **`StatusBadge` legacy contract test** | Low | `tests/design.rs:38-40` pins the legacy `.status-badge` class. The new cell renderer emits `.pill`, so the legacy class is still shipped (in the CSS) but no longer used by the table. The contract test passes; the future consumer refactor can drop `.status-badge` cleanly. |
| **OKT-47 JS plugin regression** | Low | The router (`src/router.rs:316-337`) and `status_dot_color` pipeline are unchanged. The new `.pill` / `.health-dots` / `.search-field` classes are additive; JS plugins that use their own CSS keep working. |
| **Mockup visual drift** | Low | The mockup (`openkite-console.html:189-260`) uses `<table class="resource-table">` (semantic `<table>`); the current Dioxus implementation uses a `<div>` grid for virtualization. OKT-32 keeps the `<div>` grid (virtualization requires it) and only changes the **class names** on the existing divs. A future ticket can swap to semantic `<table>` if Dioxus 0.7 gains a virtualized-table helper. |
| **Cargo dep drift** | None | `Cargo.toml:18-41` is unchanged. |
| **Doc comments with `(OKT-N)` refs** (user directive 2026-08-30) | Low | This PR adds `// no comment narrating OKT-N` — only the commit trailer carries the ticket ref. Module-level `//!` in `src/components/resource_table.rs:1-6` is reworded to drop the OKT-N tone if any is added. |
| **Hot-reload of the CSS file** | None | `assets/main.css` is loaded once via `include_str!` in `src/lib.rs:101`; the webview does not hot-reload. Style changes need a rebuild. |
| **Cargo fmt regression on multi-region edits** (skill §Gotchas: "Hermes `patch` fuzzy matching corrupts files on multi-region edits") | Medium | Use `write_file` for the whole `src/components/resource_table.rs` rewrite in chunk E (instead of `patch`) — the file is 608 lines and the change touches ≥4 disjoint regions. Verify with `cargo fmt --all -- --check` before every push. |
| **Dropped push event → "no checks reported"** (skill §Gotchas) | Low | First-push run hits the OKT-31-era race; verify with `gh run list --branch feat/okt32-resource-table --limit 3` before any empty-commit re-trigger. |
| **Branch predates merged PRs** | Low | `git switch -c feat/okt32-resource-table origin/main` (NOT off a feature branch). The OKT-29 plan's stale `git switch main` line is ignored — OKT-31/#43/#45 are already merged, `origin/main` is the right base. |

## 8. PR description

The PR body draft lives at `.hermes/plans/PR-description-draft.md` next to this plan, pre-filled per `.github/pull_request_template.md` (L1–37). The verified workitem UUID `387fc7b8-226c-4793-a31d-c4aa6da23f42` is used as a markdown URL. The template's `### Relevant Plane Tickets:` heading (plural) is preserved verbatim (per skill §Ticket↔PR rule, the 2026-08-30 verification).

## 9. openkite-dev skill rules applied

Pulled from the `openkite-dev` skill, checked against this plan:

- **One ticket at a time.** This plan covers only OKT-32. No cross-ticket scope creep. OKT-33 (per-kind columns), OKT-34 (pod detail), OKT-35+ (drilldown) are explicitly deferred and listed in §1.
- **Branch from `origin/main`, not a feature branch.** `git switch -c feat/okt32-resource-table origin/main`. The OKT-29 plan's stale "from `feat/okt31-shell-complete`" line is ignored — OKT-31 already merged.
- **Conventional commit `feat(openkite): resource table component (P2 UI) (OKT-32)`.** The `(OKT-N)` suffix is in the commit trailer only — NOT in code comments (user directive 2026-08-30).
- **No `OKT-N` in source comments.** Module doc-comments describe what the table *is*, not which ticket built it. The new `StatusPill` / `HealthDots` doc-comments are ticket-ref-free.
- **Plane ticket hyperlink uses markdown URL, not bare text.** `[OKT-32 — …](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/387fc7b8-226c-4793-a31d-c4aa6da23f42)`.
- **PR body follows `.github/pull_request_template.md`.** All 6 type checkboxes and all 6 checklist items present. The template's `### Relevant Plane Tickets:` (plural) is used verbatim.
- **CI-as-compiler loop** (skill §CI-as-compiler loop): `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` locally before every push. CI is the build/test gate. No link-stage builds locally.
- **Comments-as-context, not change-history.** Doc-comments on every new `pub` item (`StatusPill`, `HealthDots`, `StatusKind::pill_class`) describe what the function returns, not when it was added.
- **Foundation tests don't import sibling un-merged modules.** `tests/resource_table.rs` uses only the public API (`openkite::components::resource_table::*`); it does NOT import `openkite::design` (the OKT-29 module is already merged, but the principle still holds: tests should be self-contained).
- **EventHandler passed bare, not `.clone()`d** (skill §Dioxus 0.7: "EventHandler/Callback is Copy → don't .clone() it"). `on_row_click: Option<EventHandler<ResourceRow>>` in the prop list, then the `move |_|` closure captures it by value.
- **`read()` returns a guard, not an owned value** (skill §Dioxus 0.7). The new `HealthDots` `for cls in dots { … }` iterates a `Vec<&'static str>` computed BEFORE the rsx!; the `for (label, active) in chips.into_iter() { … }` in the existing `ResourceTable` rsx! (`src/components/resource_table.rs:254-256`) is the same pattern, no change.
- **`use_memo` takes zero-arg closures** (skill §Dioxus 0.7). Not used in this PR — the new renderers skip memos entirely per the pattern (skill §Dioxus 0.7: "For computed lists skip use_memo entirely: plain locals before rsx!").
- **`src/lib.rs` alphabetical `pub mod` placement** (skill §Foundation-first workflow). No new `pub mod` line in this PR — the module already exists.
- **Doc-comment every `pub` function** (skill §Code conventions). New `StatusPill`, `HealthDots`, and `StatusKind::pill_class` each get a `///` doc.

## Branch + commit plan

```bash
# Pre-flight
git switch main
git pull --ff-only origin main
git switch -c feat/okt32-resource-table

# After each chunk's push (A → F), CI gates per the loop in skill §CI-as-compiler
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "feat(openkite): resource table component (P2 UI) (OKT-32)" \
           -m "Re-skin the virtualized table on the Liquid Frost Glass primitives; ..."
git push -u origin feat/okt32-resource-table

# After all chunks are pushed (or, more likely, accumulate in one PR):
gh pr create --base main \
             --title "feat(openkite): resource table component (P2 UI)" \
             --body-file .hermes/plans/PR-description-draft.md

# CI poll
gh pr checks <N> --watch --interval 15

# On green
gh pr merge <N> --squash --delete-branch
# Move ticket In Progress → Done, post the PR URL as a comment.
```

The PR is small enough to ship as one squash commit if the chunks are pushed in series; the chunks A–F exist so the implementer can recover from a CI failure (skill §CI failure loop escape valve: "if clippy fails twice in a row on the same component … STOP incremental patching") and so a reviewer can rebase interactively if a chunk needs to be split. If CI goes green on the first push, the chunks are squash-merged into one commit; if a chunk needs rework, the implementer can `git rebase -i` to re-order or fix the offending commit before merge.
