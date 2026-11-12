# Design surfaces

What the console must render, surface by surface, and which visual classes and tokens each one
requires. This is a contract for the console stylesheet that does not exist yet, not a description of
shipped code: v1's stylesheet and its tests were retired with the rest of the v1 product tree.

## Sources of record

- `docs/reference/opendesign-console/openkite-console.css` and its sibling sheets
  (`openkite-charts.css`, `openkite-workloads.css`) — the prototype, vendored byte-identical to the
  live in-cluster design, holding every class name below.
- `DESIGN.md` — the token set, its reference values and the contrast floor.
- The rebuild spec — which surfaces are in scope now, which are deferred, and the parity gate.

Line numbers are deliberately absent. The reference is a live artifact that changes; selectors and
class names are the stable part. Where a class below is not present in the vendored sheets it is
named as a requirement, and the console must introduce it.

## Coverage

| # | Surface | Required classes | Tokens | Status |
|---:|---|---|---|---|
| 1 | App shell + sidebar | .app .sidebar .brand .brand-mark .brand-word .cluster-btn .nav .nav-title .nav-item .nav-item .nav-badge .sidebar-footer .status-line | surface-solid, border, muted, accent, argo, brand, success | required |
| 2 | Topbar + breadcrumbs | .main .topbar .menu-toggle .breadcrumbs .topbar-actions .icon-btn .avatar | surface, border, subtle, fg, accent, shadow-hover | required |
| 3 | Workloads inventory | .view .page-head .eyebrow .page-sub .page-actions .toolbar .chip-row .chip .search-field .panel .resource-table .table-wrap .resource-name .health-dots .dot .pill .panel-footer .pager | bg, surface, surface-solid, border, fg, muted, subtle, accent, success, warn, danger, violet, r-pill, r-md, r-sm | required |
| 4 | Namespace bar (strip) | .chip-row .chip .chip.active .search-field | surface-solid, border, fg, accent, r-pill | required |
| 5 | Log panel / dock | .log-panel .log-header .log-handle .log-body .log-line .log-time .log-level .log-method .log-msg .log-paused .log-title .log-pod .log-actions | terminal-bg, terminal-fg, log-info, log-method, log-error, shadow-terminal, r-md | required |
| 6 | Argo CD application cards | .app-grid .app-card .card-status .source-icon .tag-row .tag .card-meta .card-swipe-actions | surface, border, success, warn, danger, progress, argo, violet, shadow-rest, shadow-hover | reference-only — the v2 Argo surface is an application grid with sync state, not swipe cards |
| 7 | Mobile bottom navigation | .bottom-nav .bottom-tabs .bottom-tab | surface-solid, border, muted, fg, accent, argo | deferred — touch and mobile parity, spec §3 row 16 |
| 8 | Resource detail side pane | .inspector .inspector-header .inspector-title .inspector-eyebrow .inspector-tabs .tab-btn .tab-btn.active .inspector-body .kv-list .kv-row .inspector-actions | surface, border, fg, muted, subtle, accent, shadow-hover, r-md | required |
| 9 | Toast (cross-cutting) | .toast .toast.show | fg, shadow-hover, r-md | required |
| 10 | Pull-to-refresh indicator | .pull-indicator .pull-indicator.show | surface-solid, border, fg, shadow-hover, r-pill | required |
| 11 | Cluster overview: capacity meters + event feed | .stat-tile .overview-grid .overview-head .overview-meta .meter-track .meter-fill .event-list | surface, border, muted, accent, success, shadow-rest, r-md | required |
| 12 | Charts: line/area, sparklines, gauges, trend cells | .chart-card .chart-svg .spark .gauge .trend-cell | accent, success, warn, danger, border, muted | required |
| 13 | Metric ranking + metric detail pane | .metric-rank .metric-row .metric-detail | surface-solid, border, muted, accent, fg | required |
| 14 | Search sheet and command palette | .palette .palette-input .palette-row .palette-scope | surface-solid, border, shadow-terminal, muted, accent | required |
| 15 | Menu popup, row / column / namespace / crumb menus | .menu-pop .menu-item .row-menu .column-picker .column-picker-panel | surface-solid, border, shadow-terminal, fg, muted | required |
| 16 | Bulk-select bar | .bulk-bar .bulk-bar.active .bulk-bar-count .bulk-bar-reason .bulk-bar-actions .bulk-bar .btn .table-select .table-select-head .row-check .table-row.checked | surface-solid, border, fg, muted, accent, danger, shadow-rest, r-md, r-pill, r-sm | required |
| 17 | Cluster switcher + account menu | .switcher .switcher-row .account-pop | surface-solid, border, fg, muted, success, shadow-hover | deferred — multi-cluster lands in v1.1, spec §3 row 17 |
| 18 | Nav category pull-down + tree layout | .nav-section .nav-category .nav-tree .nav-tree-item | surface-solid, border, fg, muted, accent, r-sm | required |
| 19 | Terminal pane in the dock | .term .term-line .term-prompt | terminal-bg, terminal-fg, log-info, log-method, log-error, r-md | required |
| 20 | Settings + kubeconfig dialog | .modal .modal-head .modal-body .modal-actions | surface-solid, border, shadow-terminal, fg, muted | required |

## Deltas from the reference worth keeping

**Sidebar frost.** The reference frosts `.sidebar` and keeps section titles on `muted`. The console
keeps that, and keeps the healthy-cluster dot on `subtle` rather than the reference's `success`, so a
healthy cluster does not read as an alert.

**Pull-to-refresh.** The reference ships the indicator as static markup. The console needs it live:
`.pull-indicator` with `.show` driving the reveal, no inline styles.

**Namespace bar.** The strip, the search circle and the clear affordance are new design, not reference
carry-over. Namespace chips must filter in place without a route change.

**Bulk-select.** The bar, the count and the danger button follow the reference. The tri-state header
checkbox, shift-range selection and the empty-selection reason line are console additions.

**Inspector tabs.** The reference's tab vocabulary is kept; tab content is per-kind and therefore
specified by the kind catalogue rather than here.

## Not in this document

Command palette scope chips, drag-and-drop column reordering, and the log-dock tab strip evolve past
the reference. They are specified by their own tickets, and this file only fixes the classes they must
reuse (`.palette`, `.column-picker`, `.log-header`).

## Status vocabulary

- **required** — in scope for the v2 console; the classes must exist and match the token set.
- **reference-only** — kept in the reference for completeness; the v2 surface is a different shape.
- **deferred** — out of the first release, named with the spec row that defers it.
