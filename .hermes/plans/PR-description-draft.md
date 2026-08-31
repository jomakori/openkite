<!-- Paste verbatim into `gh pr create --body-file PR-description-draft.md` or via `gh pr edit <N> --body-file`. -->

## 📚 Description

This PR re-skins the existing P1 `ResourceTable` component (merged as part of the workloads view) on top of the **Liquid Frost Glass** design system primitives (merged in #49 / OKT-29) and adds the P2 affordances called out in OKT-32: visible **sort indicators** in the column headers, the **`.search-field` filter input**, **`.pill` + `.health-dots`** status rendering, and a stubbed **`on_row_click: EventHandler<ResourceRow>`** prop the OKT-34 pod-detail slide-over will wire in.

The pure-logic foundation is unchanged — `sort_by_key` (`src/components/resource_table.rs:61-72`), `compare_sort_keys` (`:50-57`), `visible_range` (`:80-90`), `matches_query` (`:75-78`), `Cell` (`:92-131`), `ResourceRow` (`:133-155`), and `ColumnDef` (`:157-164`) ship as-is. OKT-32 is a **visual + callback surface change**, not a logic rewrite. The Workloads view (`src/views/workloads.rs:20-48`) and all workload row mappers (`src/workloads.rs:95-420`) compile unchanged.

### Visual wrapper on the existing table

- `render_table_cell` (`src/components/resource_table.rs:382-402`) now renders a status cell as the design-system `.pill` (with the `success` / `warn` / `danger` / `muted` variant) instead of the legacy `.status-badge` (kept for the OKT-29 contract test, see `tests/design.rs:38-40`).
- `header_cell` (`:276-305`) renders a visible **▲/▼ sort arrow** in a new `.sort-indicator` span, tied to the existing `(column, direction)` state at `:185-192`.
- The filter `<input class="table-filter" …>` (`:248-253`) is wrapped in the design-system `.search-field` container (44px touch target, `--r-sm`, focus-within border, `::placeholder` from `assets/main.css:241-261`).
- The table root sits inside `.panel > .table-wrap > .resource-table` (the existing `assets/main.css:174-183` and `:307-313` rules) so the rows render on a frosted surface, not the page background.
- A new `#[component] fn HealthDots(ready: u32, total: u32)` is exported from `src/components/resource_table.rs` for the OKT-33 per-kind column work; it renders the mockup's `<span class="health-dots"><span class="dot ok"></span>…</span>` pattern (`openkite-console.html:199-200`) but is **not yet wired** into any column.

### `StatusKind::pill_class` mapping

A new method `pub fn pill_class(self) -> &'static str` is added to `StatusKind` (`src/components/status_badge.rs`) returning the design-system variant name (`"success"` / `"warn"` / `"danger"` / `"muted"`) for each of the existing 10 variants. The legacy `class()` method (returning `"status-ok"` / `"status-warn"` / `"status-err"` / `"status-muted"`) is preserved for the OKT-29 contract test and any future consumer that needs the legacy class. Both methods are pinned by the new `tests/resource_table.rs`.

### Row-click callback (stub, OKT-34 wires it)

`ResourceTable` gains a new prop: `#[props(default)] on_row_click: Option<EventHandler<ResourceRow>>`. The closure maps the option at call site (`if let Some(h) = on_row_click { h.call(row.clone()) }`) so existing call sites compile without a handler. No workload view (`src/views/workloads.rs`) wires a click handler in this PR — the prop plumbs in so OKT-34's pod-detail slide-over is a one-line wiring change.

### CSS contract test extension

`tests/design.rs:41-64` `REQUIRED_CLASSES` is extended with `".sort-indicator"`. The new rule is added to `assets/main.css` inside the table primitives block (`:307-330`) as `.sort-indicator { font-size: 10px; color: var(--fg-2); margin-left: 4px; user-select: none; }`. The "every_required_class_is_present" test (`:84-92`) iterates the list — no code change.

### New integration test

`tests/resource_table.rs` (new, ~60 lines) re-asserts the existing pure-logic surface from the public API (`sort_by_key` / `compare_sort_keys` / `visible_range` / `matches_query` / `Cell` / `ResourceRow::search_text`) and adds two new tests pinning the `StatusKind::pill_class` mapping: `status_kind_pill_classes_cover_every_variant` and `pill_class_mapping_matches_legacy_class`. No kube, no JS, no Dioxus test harness — same pattern as `tests/workloads.rs:1-97`.

### Deferred to dependent tickets (intentional)

- **Row click → pod detail slide-over**: the `on_row_click` callback is plumbed and logs to `tracing::info!` when fired. **OKT-34** (pod detail slide-over) replaces the log with the `Inspector` open call. The component split (table vs. inspector) keeps this PR independent of the inspector's internal state.
- **Per-kind column customisation** (Pods Health column with `.health-dots`, Deployments Ready/Available split, Jobs Completions): **OKT-33** widens the column model so each workload kind supplies its own `ColumnDef` set + per-cell render hooks. OKT-32 ships the `.health-dots` helper, OKT-33 wires it.
- **Status drilldown** (clicking a status pill → "show me all Failed pods"): OKT-35+ per the Phase-2 ticket map in the openkite-dev skill §Project phases.
- **`.status-badge` → `.pill` consumer refactor**: the legacy class is still shipped (OKT-29 contract test pins it). Future consumers (ArgoCD plugin row, Inspector status block) can adopt `.pill` directly; a one-line PR drops the legacy class.
- **`.ns-chip` → `.chip` migration** in the table toolbar: touching the existing `.ns-chip` rule affects the shell sidebar namespace chip pattern (`src/router.rs:316-337`), so the migration is a sidebar/topbar follow-up, not this PR.
- **Semantic `<table>` markup**: the mockup uses `<table class="resource-table">` (`openkite-console.html:189`); the current Dioxus implementation uses a `<div>` grid because virtualization (`visible_range` at `src/components/resource_table.rs:80-90`) requires absolute positioning. Revisit if Dioxus 0.7 gains a virtualized-table helper.

### Relevant Plane Tickets:

* [OKT-32 — Resource table component — virtualized, sort, filter](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/387fc7b8-226c-4793-a31d-c4aa6da23f42)

## 🔍 Types of Changes

<!-- Check the type of change this PR introduces: -->

- [x] Feature (new functionality)
- [ ] Fix (bug fix)
- [ ] Refactor (no behavior change)
- [ ] CI / Tooling (workflows, dev environment)
- [x] Docs / Chore (`.sort-indicator` CSS contract test extension + the inline doc-comments on `StatusKind::pill_class` / `HealthDots` / `StatusPill`)
- [ ] Version bump (SDK / release)

## ✅ Checklist

> If you're unsure about any of these, don't hesitate to ask.

- [x] CI green: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`
- [x] Unit tests added/updated for the change (`tests/resource_table.rs` new; `tests/design.rs` extended with `.sort-indicator`; `src/components/status_badge.rs` inline tests retained)
- [x] Conventional commit message: `feat(openkite): resource table component (P2 UI) (OKT-32)`
- [x] SDK version bumped if the public API changed (N/A — `openkite-plugin-sdk` is unchanged; the new `EventHandler<ResourceRow>` prop and `StatusKind::pill_class` are crate-internal additions)
- [x] No secrets in code (pass them in via environment / Doppler)
- [x] Plane ticket moved across the board (Todo → In Progress → Done)

<!-- Manual smoke checklist for the reviewer:
  1. cargo run, navigate to Workloads → Pods
  2. Verify the pods table renders inside a frost panel (.panel > .table-wrap)
  3. Verify the filter input sits in a .search-field container (44px height, focus-within border)
  4. Type "nginx" — only matching rows remain
  5. Click the "Name" column header — arrow appears, rows reverse
  6. Click again — arrow flips to ▼, rows reverse again
  7. Verify status column shows colored pills (green Running, yellow Pending, red Failed)
  8. Verify the ArgoCD plugin route (OKT-47) still loads — no regressions to the .status-dot status bar
  9. Verify the sidebar (.nav-section) still renders — no regressions from the .ns-chip rule (unchanged)
-->
