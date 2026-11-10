# OpenKite Console — Surface Contract

The checkable contract between the **OpenDesign reference** and the **Phase-3
surfaces**. `DESIGN.md` pins the tokens; this file pins which surface consumes
which tokens and which reference lines each surface is graded against.

Two anchors are used throughout:

- **ref** — the vendored reference artifact at
  `docs/reference/opendesign-console/` (`openkite-console.{html,css}` plus
  `openkite-charts.css` and `openkite-workloads.css` where a surface lives
  there). Line refs are into that copy.
- **repo** — the shipped stylesheet `crates/openkite-ui/assets/main.css`,
  whose class vocabulary is asserted by `crates/openkite-ui/tests/design.rs`.

A surface is **done** when the classes it lists exist in the repo stylesheet,
its values match the tokens in `DESIGN.md`, and its states (below) render.

## Coverage table

| # | Surface | ref (html → css) | repo classes | Primary tokens | Ticket | Status |
|---|---|---|---|---|---|---|
| 1 | App shell + sidebar | `:79`, `:80-113` → `:74-205` | `.app` `.sidebar` `.brand` `.brand-mark` `.brand-word` `.cluster-btn` `.nav` `.nav-title` `.nav-item` `.nav-item .nav-badge` `.sidebar-footer` `.status-line` | `surface-solid`, `border`, `muted`, `accent`, `argo`, `brand`, `success` | T4 | classes shipped |
| 2 | Topbar + breadcrumbs | `:118-141` → `:206-265` | `.main` `.topbar` `.menu-toggle` `.breadcrumbs` `.topbar-actions` `.icon-btn` `.avatar` | `surface`, `border`, `subtle`, `fg`, `accent`, `shadow-hover` | T4 | classes shipped |
| 3 | Workloads inventory | `:163-213` → `:266-490` | `.view` `.page-head` `.eyebrow` `.page-sub` `.page-actions` `.toolbar` `.chip-row` `.chip` `.search-field` `.panel` `.resource-table` `.table-wrap` `.resource-name` `.health-dots` `.dot` `.pill` `.panel-footer` `.pager` | `bg`, `surface`, `surface-solid`, `border`, `fg`, `muted`, `subtle`, `accent`, `success`, `warn`, `danger`, `violet`, `r-pill`, `r-md`, `r-sm` | T5, T10 | classes shipped |
| 4 | Namespace bar (strip) | `:181` (chips only) | `.chip-row` `.chip` `.chip.active` `.search-field` | `surface-solid`, `border`, `fg`, `accent`, `r-pill` | T14, T16 | **partial** — strip/search-circle/× are new design |
| 5 | Log panel / dock | `:250-265` → `:491-511`, `:942-997` | `.log-panel` `.log-header` `.log-handle` `.log-body` `.log-line` `.log-time` `.log-level` `.log-method` `.log-msg` `.log-paused` — `.log-title` `.log-pod` `.log-actions` are ref-only | `terminal-bg`, `terminal-fg`, `log-info`, `log-method`, `log-error`, `shadow-terminal`, `r-md` | T6, T11, T15 | classes shipped |
| 6 | Argo CD cards | `:215-229` → `:512-636` | *(not shipped)* `.app-grid` `.app-card` `.card-status` `.source-icon` `.tag-row` `.tag` `.card-meta` `.card-swipe-actions` | `surface`, `border`, `success`, `warn`, `danger`, `progress`, `argo`, `violet`, `shadow-rest`, `shadow-hover` | T7 (out of scope — OKA) | reference-only |
| 7 | Mobile bottom nav | `:232-244` → `:637-667` | *(not shipped)* `.bottom-nav` `.bottom-tabs` `.bottom-tab` | `surface-solid`, `border`, `muted`, `fg`, `accent`, `argo` | T9, T16 | reference-only |
| 8 | Resource detail side pane | `:268-279` → `:677-748` | `.inspector` `.inspector-header` `.inspector-title` `.inspector-eyebrow` `.inspector-tabs` `.tab-btn` `.tab-btn.active` `.inspector-body` `.kv-list` `.kv-row` `.inspector-actions` | `surface`, `border`, `fg`, `muted`, `subtle`, `accent`, `shadow-hover`, `r-md` | T18 | classes shipped |
| 9 | Toast (cross-cutting) | `:317` → `:778-796` | `.toast` `.toast.show` | `fg`, `shadow-hover`, `r-md` | all write surfaces | classes shipped |
| 10 | Pull-to-refresh indicator | `:146` → `:749-768` | `.pull-indicator` | `surface-solid`, `border`, `fg`, `shadow-hover`, `r-pill` | T9, T11 | classes shipped |
| 11 | Cluster overview: capacity meters + event feed | `:148-161` → `:917-939`, `charts.css:64-100` | *(not shipped)* `.stat-tile` `.overview-grid` `.overview-head` `.overview-meta` | `surface`, `border`, `muted`, `accent`, `success`, `shadow-rest`, `r-md` | — | **not shipped** — the `/` route renders `.stat-tile` counts only; no `.meter-track`/`.meter-fill`/`.event-list` rule exists in `main.css` |
| 12 | Charts: line/area, sparklines, gauges, trend cells | `:160` → `charts.css:1-61` | *(not shipped)* — no chart component | `accent`, `success`, `warn`, `danger`, `border`, `muted` | — | **not shipped** — no `.chart-card`/`.chart-svg`/`.spark`/`.gauge`/`.trend-cell` rule in `main.css`; `routes.rs` computes numeric tiles only |
| 13 | Metric ranking + clickable-chart detail pane | `:160` → `charts.css:564-588`, `:622-638` | *(not shipped)* | `surface-solid`, `border`, `muted`, `accent`, `fg` | — | **not shipped** — depends on the chart system above; no ranked-metric surface or metric detail pane in the repo |
| 14 | Pull-down search sheet (`#searchSheet`) | `:303-314` → `charts.css:377-423` | `.palette` (Ctrl+P overlay) | `surface-solid`, `border`, `shadow-terminal`, `muted`, `accent` | — | **unknown** — the repo ships a command palette overlay (`.palette`), but whether it satisfies the sheet's scope chips and pull-down affordance is not decidable from `routes.rs` |
| 15 | Menu popup + row / column / namespace / crumb menus | `:144`, `:281-290` → `workloads.css:6-23`, `:49-57` | `.column-picker` `.column-picker-panel` | `surface-solid`, `border`, `shadow-terminal`, `fg`, `muted` | — | **unknown** — the repo has `.column-picker`/`.column-picker-panel` but no `.menu-pop`/`.row-menu` vocabulary, so overlay parity is undecided |
| 16 | Bulk-select bar | `:186-190` → `workloads.css:42-43`, `:39` | `.bulk-bar` `.bulk-bar.active` `.bulk-bar-count` `.bulk-bar-reason` `.bulk-bar-actions` `.bulk-bar .btn` `.table-select` `.table-select-head` `.row-check` `.table-row.checked` | `surface-solid`, `border`, `fg`, `muted`, `accent`, `danger`, `shadow-rest`, `r-md`, `r-pill`, `r-sm` | P3-6 | classes shipped — the bar, the count and the danger button follow the reference; the tri-state header checkbox, shift-range and the empty-selection reason line are repo deltas |
| 17 | Cluster switcher + account / profile menu | `:86-90`, `:102-112`, `:287-288` → `charts.css:200-221` | `.switcher` (Ctrl+Tab overlay) | `surface-solid`, `border`, `fg`, `muted`, `success`, `shadow-hover` | — | **unknown** — the repo ships a Ctrl+Tab cluster switcher overlay (`.switcher`); the sidebar cluster button and account popover pairing has no equivalent in `main.css` |
| 18 | Nav category pull-down + tree / tree-grid layout | `:92-99` → `charts.css:306-361` | *(not shipped)* — flat `.nav-section` model | `surface-solid`, `border`, `fg`, `muted`, `accent`, `r-sm` | — | **not shipped** — `shell.rs` models a flat section list; no tree layout or category-filter state exists |
| 19 | Terminal pane in the dock | `:250-265` → `:984-991` | `.term` / `components/terminal.rs` | `terminal-bg`, `terminal-fg`, `log-info`, `log-method`, `log-error`, `r-md` | — | **unknown** — a terminal surface exists in the repo, but `routes.rs` has no terminal route and the mockup drives it from dock tabs; parity is not decidable from the docs alone |
| 20 | Settings + kubeconfig dialog | `:291-300` → `charts.css:223-242` | `.modal` / `components/crud_modal.rs` (CRUD + confirm only) | `surface-solid`, `border`, `shadow-terminal`, `fg`, `muted` | — | **unknown** — the repo ships CRUD/confirm modals but no settings or kubeconfig dialog branch |

## Per-surface notes

### 1. App shell + sidebar

Reference sidebar is a frosted 252px column; the repo keeps the width and the
frost (`.sidebar`, `main.css:145-157`). Section titles carry the nav contract:
`.nav-title` is uppercase 10px `muted`; `.nav-section.argo .nav-title` and the
active Argo icon switch to `argo` — this is the **only** place `argo` may
appear, so a reviewer can grep for it. `.status-line` is the cluster-health
anchor; the repo keeps it `subtle` (`main.css:261-269`) where the reference used
`success` (`:205`) — a deliberate repo delta, so a "healthy cluster" dot is not
a second green competing with the health column.

### 3. Workloads inventory

The eight-column table is the densest token consumer. Column semantics are
fixed: `.resource-name` (mono + 14px `subtle` icon), `.namespace` (`muted`),
`.controller` (`violet`, 12.5px), `.restarts` (mono; `.warn` → `warn` weight
600), `.qos` (11px uppercase `subtle`). `.health-dots` is the inline status cell
and must use `.dot.ok|.warn|.err` — not ad-hoc colours.

### 4. Namespace bar

The reference only draws chips inline in the toolbar. T14's strip (one line,
never wraps), the search circle and the × reset are **new design**, so they are
graded against `DESIGN.md` tokens, not against a reference pixel. Constraints
that are non-negotiable because they come from the token set: chips are
`r-pill`, 44px, `surface-solid` at rest and `fg` + white when active.

### 5. Log panel / dock

The only opaque surface (`terminal-bg`). Level colouring is part of the token
contract, not the log component's choice: `.log-level` → `log-info`,
`.log-level.warn` → `warn`, `.log-level.error` → `log-error`,
`.log-method` → `log-method`. T15's dock is a new arrangement of this panel;
it must not re-declare the colours.

### 6. Argo CD cards

Depicted but explicitly out of scope (OKA, T7). Recorded here so that when it
lands it consumes the tokens above rather than inventing a card palette — the
`.card-status` band maps `synced`/`progressing`/`degraded`/`outofsync` to
`success`/`progress`/`danger`/`warn`.

### 8. Resource detail side pane

T18 requires the pane to open in the *same position for every kind* and to
replace rather than stack. The reference's `.inspector` is the shape
(`min(420px,100%)`, right edge, slide-over) and `.kv-list` the row primitive;
depth comes from `shadow-hover` in the repo (the reference used
`shadow-terminal` — the repo value is normative).

### 10. Pull-to-refresh indicator

The mockup now ships the indicator as static markup (`openkite-console.html:146`)
styled by `.pull-indicator` / `.pull-indicator.show` (`:749-768`). It is no
longer JS-injected, so the old `*(JS-injected)*` ref is replaced by the DOM
anchor; the gesture itself is still driven from `openkite-console.js`.

## Cross-cutting rules

- **States.** Every data surface must render **empty, loading and error**, not
  only the happy path. The reference only draws the happy path; the token set
  covers the rest (`muted`/`subtle` for empty and loading, `danger` for error).
- **Touch floor.** 44px minimum on every interactive target — `.btn`, `.chip`,
  `.search-field`, `.nav-item`, `.icon-btn`, `.log-actions .icon-btn`. T12 and
  T16 audit this; no action may be reachable only by hover or keyboard.
- **Reduced motion.** The reference ships a `prefers-reduced-motion: reduce`
  block (`openkite-console.css:890-892`); any transition added by a Phase-3
  surface must respect it.
- **Theme flow.** Every surface treatment is token-driven, so an opaline theme
  switch flows through without a surface re-declaring a colour. A literal hex in
  a surface rule is a contract violation.

## Out of parity scope

Per the Phase-3 spec, the reference's **primary** surfaces are the workloads
inventory, the log panel and Argo CD. The live mockup also depicts the search
sheet, the cluster switcher, CRUD dialogs and the terminal view, but they stay
outside the Phase-3 parity scope: **not graded against the reference pixel,**
graded against `DESIGN.md` only. The secrets value-masking controls and the code
editor's internals remain undepicted.

## Reference contract (G1)

The class vocabulary, the token set and the state selectors are **derived from
the vendored mockup**, not listed here: `crates/openkite-ui/tests/design.rs`
parses `docs/reference/opendesign-console/openkite-console.css` and asserts that
everything it defines exists in the shipped stylesheet. That only holds while
the reference bytes stay the reference a reviewer looked at, so the bytes are
pinned by SHA-256 in the block below. A re-grab that changes the mockup fails
the contract until this block is re-derived, which is the point: the failure
message hands back ready-to-paste `ref` lines, and the class/token/state
failures that follow say what moved.

`ref-only` is the only hand-written part. Each entry is a reference selector the
app deliberately does not ship, with the reason. An entry that no longer matches
anything in the mockup fails the contract too, so the list cannot rot.

```design-contract
# Pinned vendored mockup bytes (G1). Re-derive; never hand-edit the hash alone.
ref openkite-console.css sha256 cd9061b4b42fc17ed573cec0ea41af5992228edfcdd7f935568f67ed3158e04e
ref openkite-console.html sha256 612f8ca629a7365bd5642217d60c84e79b409e1587f18c78bb2e2d613c57aff4

# Reference selectors the app does not ship, and why.
ref-only .cell-value the mockup's td wrapper that right-aligns values in the mobile card layout; the repo table renders .table-cell directly
ref-only .node the mockup's Node-column value text (fg); the repo inventory has no Node column and wraps every value in .table-cell
ref-only .svg-defs the mockup's inline icon-sprite container; the repo embeds icons per component
ref-only .bottom-tab.argo.active the Argo destination tab is plugin-owned (OKA, surface 6 out of scope); the repo ships .bottom-tab.active

# Cluster overview: the mockup's capacity-meter/event-feed panel (surface 11); overview.rs renders .stat-tile counts only
ref-only .stat-grid mockup's overview stat-card grid; overview.rs renders its own .overview-grid of .stat-tile cells instead
ref-only .stat-card mockup's overview stat card; overview.rs has no card wrapper, only .stat-tile
ref-only .stat-label mockup's stat-card label row; overview.rs uses .stat-tile-label instead
ref-only .stat-value mockup's stat-card value row; overview.rs uses .stat-tile-value instead
ref-only .overview-cols mockup's two-column overview layout; overview.rs renders a single .overview-grid, no meter/event column
ref-only .panel-title mockup's meter/event panel heading; overview.rs has no equivalent panel title
ref-only .meter-list mockup's capacity-meter list; overview.rs has no meter markup
ref-only .meter-head mockup's per-meter label row; overview.rs has no meter markup
ref-only .meter-track mockup's meter background track; overview.rs has no meter markup
ref-only .meter-fill mockup's meter fill bar; overview.rs has no meter markup
ref-only .mono mockup's monospace helper used inside the meter/event rows; overview.rs has no meter or event-feed markup to apply it to
ref-only .event-list mockup's cluster-event feed list; overview.rs has no event-feed markup
ref-only .event-text mockup's event-feed row text; overview.rs has no event-feed markup
ref-only .event-age mockup's event-feed relative-age column; overview.rs has no event-feed markup

# Breadcrumb cluster switcher: not part of surface 2 (topbar + breadcrumbs); shell.rs renders .crumb items only
ref-only .crumb-cluster mockup's cluster-switch breadcrumb button; shell.rs renders .crumb/.crumb-hide items only, no cluster switcher chip
ref-only .crumb-trail mockup's wrapper around the crumb buttons; shell.rs's .breadcrumbs renders .crumb items directly, no trail wrapper
ref-only .crumb-cluster.active state pair of the unshipped .crumb-cluster above

# Dock toolbar/tabs chrome: dock.rs ships its own toolbar and tab vocabulary instead
ref-only .dock-act mockup's dock toolbar icon button; dock.rs's toolbar uses .dock-layout-btn and .dock-manager-btn instead
ref-only .dock-actions wrapper around the mockup's .dock-act buttons; dock.rs has no equivalent wrapper
ref-only .dock-note mockup's inline notice banner under the dock bar; dock.rs has no equivalent banner
ref-only .dock-open mockup's body-level class driving the push-content layout; dock.rs tracks open/closed via a data-dock attribute, not a body class
ref-only .dock-toggle mockup's dock open/close handle button; dock.rs's dock visibility is driven by its own state, not this class
ref-only .dock-toggle[aria-pressed] state pair of the unshipped .dock-toggle above
ref-only .dock-act[aria-pressed] state pair of the unshipped .dock-act above
ref-only .dock-note.show state pair of the unshipped .dock-note above
ref-only .dock.open the repo expresses the dock's open state via the data-dock="open" attribute (components/dock.rs) rather than an .open class toggle
ref-only .edge-grip mockup's mobile edge-swipe affordance for the dock; dock.rs has no equivalent gesture hint
ref-only .hl mockup's clickable log-line highlight; the repo's log viewer has no highlight-on-click interaction
ref-only .hl-mode mockup's log-highlight-mode toggle; the repo's log viewer has no highlight-on-click interaction
ref-only .opening mockup's dock-pane open transition hook; dock.rs has no equivalent animation class
ref-only .pinned mockup's pinned-tab indicator; dock.rs has no tab-pinning feature
ref-only .tab-label mockup's dock-tab label wrapper; dock.rs's tab label is .dock-tab-label, a different class
ref-only .tab-main mockup's dock-tab clickable body; dock.rs has no equivalent wrapper around its tab contents
ref-only .temp mockup's temporary-tab style hook; dock.rs expresses the same state via the [data-temporary] attribute instead

# Terminal pane (surface 19): no terminal route; terminal.rs renders xterm.js directly and does not use this markup
ref-only .term mockup's terminal container
ref-only .term-out mockup's terminal output pane
ref-only .term-prompt mockup's terminal input row
ref-only .term-ps mockup's terminal prompt-symbol span
ref-only .cmd mockup's terminal command-echo styling
```

Re-deriving after a mockup change:

1. Replace the vendored file(s) under `docs/reference/opendesign-console/`.
2. `cargo test -p openkite-ui --test design` — the hash test fails and prints
   the new `ref` lines.
3. Paste them in, re-run: the class/token/state tests now name exactly what the
   new mockup added or dropped. Ship the rules, or add `ref-only` lines with a
   reason.
4. Update `docs/reference/opendesign-console/README.md`'s hash table and say in
   the PR what moved in the mockup.

## Provenance

The reference artifact and its hashes are recorded in
`docs/reference/opendesign-console/README.md`. The class vocabulary and the
token set are enforced by `crates/openkite-ui/tests/design.rs`; the token values
themselves are pinned by `DESIGN.md`.
