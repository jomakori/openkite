<!--
  Paste verbatim into `gh pr create --body-file .hermes/plans/okt33-workload-views-PR-description-draft.md`
  or via `gh pr edit <N> --body-file …`. Follows `.github/pull_request_template.md` (verified
  heading text: `## 📚 Description`, `### Relevant Plane Tickets:`, `## 🔍 Types of Changes`,
  `## ✅ Checklist`). Pre-PR drift check:
    diff <(grep -E '^### |^## ' .github/pull_request_template.md) \
         <(grep -E '^### |^## ' .hermes/plans/okt33-workload-views-PR-description-draft.md)
  If the template heading text drifts between plan-time and PR-time, patch this draft first
  per the openkite-dev skill §Ticket↔PR rule.
-->

## 📚 Description

This PR lands the **P2 UI layer for the seven workload kinds** (Pods, Deployments, StatefulSets, DaemonSets, ReplicaSets, Jobs, CronJobs) on top of the P1 OKT-10 row mappings, the OKT-32 generic `ResourceTable`, and the OKT-29 Liquid Frost Glass design system. It aligns the per-kind column layouts to the `openkite-console.html` mockup, expands the namespace chip filter from single-select to multi-select, and adds the per-kind Controller / Node / QoS / Health / Age cells the mockup prescribes.

The dispatch view (`src/router.rs:352-355` → `src/views/workloads.rs:80-106`), the per-kind reflectors (`workload_table!` macro at `src/views/workloads.rs:20-49`), the `WorkloadKind` enum (`src/workloads.rs:15-50`), the seven `*_row`/`*_columns` functions, and the `StatusKind` mapping (`src/components/status_badge.rs`) are all already on `main` from the P1 OKT-10 cycle. OKT-33 builds the design-system application and mockup-aligned column expansion on top of that foundation.

### Column expansion (mockup-aligned)

The existing P1 columns were a slim `Name | Status | Ready | Restarts/Available/Completions/Schedule/Suspend` set. The mockup (`openkite-console.html:192`) prescribes `Name | Namespace | Health | Restarts | Controller | Node | QoS | Age | Status` for Pods and an equivalent richness for the other kinds. Concretely:

- **Pods** (4 → 9 cols): add `Health` (`.health-dots` from OKT-29), `Controller` (`Deployment/foo` from `owner_references`), `Node`, `QoS`, `Age` (`2d` / `12h`).
- **Deployments** (4 → 7 cols): add `Up-to-date`, `Controller`, `Age`.
- **StatefulSets** (3 → 5 cols): add `Up-to-date`, `Age`.
- **DaemonSets** (4 → 7 cols): add `Desired`, `Current`, `Available`, `Age`.
- **ReplicaSets** (3 → 5 cols): add `Desired`, `Age`.
- **Jobs** (3 → 5 cols): add `Duration`, `Age`.
- **CronJobs** (4 → 6 cols): add `Last schedule`, `Age`. (`spec` stays non-`Option` per k8s-openapi v1_36 — the existing `src/workloads.rs:403-404` deref is correct.)

The `Namespace` column is intentionally **removed from the table body** — the mockup surfaces namespace via the chip filter (`openkite-console.html:174-180`), not as a separate column.

### Multi-select namespace chips

`ResourceTable` (`src/components/resource_table.rs:206`) held a single `Signal<Option<String>>` for namespace selection. OKT-33 replaces it with `Signal<HashSet<String>, SyncStorage>` and prepends an "All" sentinel chip. The chip class switches from the ad-hoc `.ns-chip` (still used by the topbar at `assets/main.css:155-163`) to the OKT-29 `.chip` class (`assets/main.css:221-239`). The pure filter logic is extracted to `pub fn namespace_filter(rows: &[ResourceRow], selected: &HashSet<String>) -> Vec<ResourceRow>` so `tests/resource_table.rs` can cover it without a Dioxus runtime.

### `Cell::html` variant

The existing `Cell` enum (`src/components/resource_table.rs:93-98`) carries `text | status | sort`. The Pod Health cell needs to render multiple `.dot.ok`/`.dot.err` elements (one per container). The clean fix is a new `Cell::html { sort, label, render: fn() -> Element }` variant — the function pointer keeps `src/workloads.rs` free of `dioxus::prelude::*;` (per the openkite-dev skill's "split pure logic from view" rule). The variant's `sort` field is what the existing `compare_sort_keys` comparator already consumes — no comparator change.

### Age cell

A small pure helper `age_cell(ts: &Option<Time>) -> Cell` in `src/workloads.rs`. `humanize_age(now, then) -> String` formats `0s / 30s / 5m / 3h / 2d`. The numeric sort key is seconds since creation (so a "Sort by Age ▼" puts the youngest first). `creation_timestamp = None` returns `Cell::text("-")` with a `SortKey::Text("\u{ffff}".into())` sentinel that sorts after all real text — `compare_sort_keys` is unchanged.

### Test coverage

- `tests/workloads.rs` (extended in place): from 4 tests to ~19. New coverage for the 5 non-Pod/Deployment kinds, the new age/health/controller cells, the `humanize_age` formatting, and the OKT-33 cell-count assertions.
- `tests/resource_table.rs` (new file, 6 tests): multi-select chip filter — `all / one / many / none / "All" sentinel / cluster-scoped None-namespace` matrix.

### Design-system primitive application

OKT-29's `.chip` (44px min-height, OKT-29 design system, `assets/main.css:221-239`) replaces the ad-hoc `.ns-chip` in the table toolbar. The `.pill` status badges (OKT-29, `assets/main.css:264-298`) are the future target for the existing `StatusBadge` component — the rename is **deferred to OKT-43** per the openkite-dev skill's "primitive class names overlap with existing components" gotcha. OKT-33 leaves `StatusBadge` rendering `<span class="status-badge">` and only changes the column layout and chip class.

### Deferred to dependent tickets (intentional)

- **Per-row action buttons** (`RowActions { on_delete, on_edit, on_scale }` already declared at `resource_table.rs:178-183`) → OKT-43 (CRUD UI). OKT-33 passes `row_actions: None`.
- **Pod detail inspector** with the container list from `src/pod.rs:58-83` → OKT-34. The mockup's `.inspector` slide-over (`assets/main.css:362-396`) is already shipped by the design system; OKT-34 wires the click handler + content.
- **Per-cell live watch** (status dot pulses, restart-counter interpolation) → OKT-40/41.
- **`.status-badge` → `.pill` migration** → OKT-43.
- **ArgoCD app views, services, ingress, configmaps, secrets, YAML/log/terminal/exec** → their respective OKT-N tickets.
- **Per-kind route deeplinks** (`/workloads/pods`, …) → post-OKT-43. The tab strip is the deeplink surface for now.

### Relevant Plane Tickets:

* [OKT-33 — Workload views (Pods → CronJobs)](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/1aa27b65-488c-48a0-98b3-668b5fa876a3)

## 🔍 Types of Changes

- [x] Feature (new functionality)
- [ ] Fix (bug fix)
- [ ] Refactor (no behavior change)
- [ ] CI / Tooling (workflows, dev environment)
- [ ] Docs / Chore
- [ ] Version bump (SDK / release)

## ✅ Checklist

- [ ] CI green: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`
- [ ] Unit tests added/updated for the change (`tests/workloads.rs` extended; `tests/resource_table.rs` new)
- [ ] Conventional commit message: `feat(openkite): workload views (P2 UI) (OKT-33)`
- [ ] SDK version bumped if the public API changed (N/A — no SDK public-API change; `openkite-plugin-sdk` is untouched)
- [ ] No secrets in code (pass them in via environment / Doppler)
- [ ] Plane ticket moved across the board (Backlog → In Progress → Done)
