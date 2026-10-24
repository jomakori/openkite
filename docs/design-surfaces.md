# OpenKite Console — Surface Contract

The checkable contract between the **OpenDesign reference** and the **Phase-3
surfaces**. `DESIGN.md` pins the tokens; this file pins which surface consumes
which tokens and which reference lines each surface is graded against.

Two anchors are used throughout:

- **ref** — the vendored reference artifact at
  `docs/reference/opendesign-console/openkite-console.{html,css}`. Line refs are
  into that copy.
- **repo** — the shipped stylesheet `crates/openkite-ui/assets/main.css`,
  whose class vocabulary is asserted by `crates/openkite-ui/tests/design.rs`.

A surface is **done** when the classes it lists exist in the repo stylesheet,
its values match the tokens in `DESIGN.md`, and its states (below) render.

## Coverage table

| # | Surface | ref (html → css) | repo classes | Primary tokens | Ticket | Status |
|---|---|---|---|---|---|---|
| 1 | App shell + sidebar | `:56`, `:57-127` → `:74-205` | `.app` `.sidebar` `.brand` `.brand-mark` `.brand-word` `.cluster-btn` `.nav` `.nav-title` `.nav-item` `.nav-item .nav-badge` `.sidebar-footer` `.status-line` | `surface-solid`, `border`, `muted`, `accent`, `argo`, `brand`, `success` | T4 | classes shipped |
| 2 | Topbar + breadcrumbs | `:129-155` → `:206-265` | `.main` `.topbar` `.menu-toggle` `.breadcrumbs` `.topbar-actions` `.icon-btn` `.avatar` | `surface`, `border`, `subtle`, `fg`, `accent`, `shadow-hover` | T4 | classes shipped |
| 3 | Workloads inventory | `:156-286` → `:266-490` | `.view` `.page-head` `.eyebrow` `.page-sub` `.page-actions` `.toolbar` `.chip-row` `.chip` `.search-field` `.panel` `.resource-table` `.table-wrap` `.resource-name` `.health-dots` `.dot` `.pill` `.panel-footer` `.pager` | `bg`, `surface`, `surface-solid`, `border`, `fg`, `muted`, `subtle`, `accent`, `success`, `warn`, `danger`, `violet`, `r-pill`, `r-md`, `r-sm` | T5, T10 | classes shipped |
| 4 | Namespace bar (strip) | `:174-186` (chips only) | `.chip-row` `.chip` `.chip.active` `.search-field` | `surface-solid`, `border`, `fg`, `accent`, `r-pill` | T14, T16 | **partial** — strip/search-circle/× are new design |
| 5 | Log panel / dock | `:287-315` → `:491-570` | `.log-panel` `.log-header` `.log-handle` `.log-body` `.log-line` `.log-time` `.log-level` `.log-method` `.log-msg` `.log-paused` — `.log-title` `.log-pod` `.log-actions` are ref-only | `terminal-bg`, `terminal-fg`, `log-info`, `log-method`, `log-error`, `shadow-terminal`, `r-md` | T6, T11, T15 | classes shipped |
| 6 | Argo CD cards | `:316-522` → `:571-695` | *(not shipped)* `.app-grid` `.app-card` `.card-status` `.source-icon` `.tag-row` `.tag` `.card-meta` `.card-swipe-actions` | `surface`, `border`, `success`, `warn`, `danger`, `progress`, `argo`, `violet`, `shadow-rest`, `shadow-hover` | T7 (out of scope — OKA) | reference-only |
| 7 | Mobile bottom nav | `:523-539` → `:696-726` | *(not shipped)* `.bottom-nav` `.bottom-tabs` `.bottom-tab` | `surface-solid`, `border`, `muted`, `fg`, `accent`, `argo` | T9, T16 | reference-only |
| 8 | Resource detail side pane | `:540-569` → `:746-808` | `.inspector` `.inspector-header` `.inspector-title` `.inspector-eyebrow` `.inspector-tabs` `.tab-btn` `.tab-btn.active` `.inspector-body` `.kv-list` `.kv-row` `.inspector-actions` | `surface`, `border`, `fg`, `muted`, `subtle`, `accent`, `shadow-hover`, `r-md` | T18 | classes shipped |
| 9 | Toast (cross-cutting) | `:570` → `:837-855` | `.toast` `.toast.show` | `fg`, `shadow-hover`, `r-md` | all write surfaces | classes shipped |
| 10 | Pull-to-refresh indicator | *(JS-injected)* → `:808-827` | `.pull-indicator` | `surface-solid`, `border`, `fg`, `shadow-hover`, `r-pill` | T9, T11 | classes shipped |

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

## Cross-cutting rules

- **States.** Every data surface must render **empty, loading and error**, not
  only the happy path. The reference only draws the happy path; the token set
  covers the rest (`muted`/`subtle` for empty and loading, `danger` for error).
- **Touch floor.** 44px minimum on every interactive target — `.btn`, `.chip`,
  `.search-field`, `.nav-item`, `.icon-btn`, `.log-actions .icon-btn`. T12 and
  T16 audit this; no action may be reachable only by hover or keyboard.
- **Reduced motion.** The reference ships a `prefers-reduced-motion: reduce`
  block (`openkite-console.css:967-969`); any transition added by a Phase-3
  surface must respect it.
- **Theme flow.** Every surface treatment is token-driven, so an opaline theme
  switch flows through without a surface re-declaring a colour. A literal hex in
  a surface rule is a contract violation.

## Out of parity scope

Per the Phase-3 spec, the reference's **primary** surfaces are the workloads
inventory, the log panel and Argo CD. **Not depicted, therefore not graded
against the reference:** the command palette, the cluster switcher, CRUD modals,
the code editor, the terminal view, and the secrets value-masking controls. They
are graded against `DESIGN.md` only.

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
ref openkite-console.css sha256 11aed3d3f58af7ed2eb6a340fb1399fd3520e2924905f8bdc85bdea01a6adbb1
ref openkite-console.html sha256 7f3e2931f56fad821f41b1e82116b9ef69c289cdbc2f5f24b03517015c9ed1e4

# Reference selectors the app does not ship, and why.
ref-only .cell-value the mockup's td wrapper that right-aligns values in the mobile card layout; the repo table renders .table-cell directly
ref-only .node the mockup's Node-column value text (fg); the repo inventory has no Node column and wraps every value in .table-cell
ref-only .svg-defs the mockup's inline icon-sprite container; the repo embeds icons per component
ref-only .paused only ever a modifier on .log-panel, see .log-panel.paused below
ref-only .log-title mockup log-header chrome; the repo's dock owns its own header (.dock-tab/.dock-menu)
ref-only .log-pod mockup log-header chrome (pod name); the dock header carries the target instead
ref-only .log-actions mockup log-header chrome (icon cluster); the repo's dock toolbar carries those actions
ref-only .log-panel.open the mockup's mobile drawer transform; the repo log panel is a dock surface and is never translated off-canvas
ref-only .log-panel.paused the repo expresses pause by rendering .log-paused (components/logs.rs) rather than styling a base+state pair
ref-only .bottom-tab.argo.active the Argo destination tab is plugin-owned (OKA, surface 6 out of scope); the repo ships .bottom-tab.active
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
