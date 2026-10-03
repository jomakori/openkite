# OpenKite Design System — Liquid Frost Glass

The design system is the **token contract + primitive CSS layer** that every
console surface consumes without re-deciding the visual language.

It lives in four places, and each one has exactly one job:

| Artifact | Owns |
|---|---|
| **`DESIGN.md`** (repo root) | The token **values** and their provenance. Authoritative — nothing else restates a value. |
| **`docs/design-surfaces.md`** | Which surface consumes which tokens, graded against the reference. |
| `crates/openkite-ui/assets/main.css` | The shipped stylesheet, embedded as `openkite_ui::MAIN_CSS` (`crates/openkite-ui/src/lib.rs:29`) and injected by each host. |
| `crates/openkite-ui/src/design/{mod.rs,tokens.rs}` | Typed Rust view of the surface-treatment strings (blur radii, panel radii, shadow strings) so Dioxus `style:` attributes reference them by name. |

The values themselves are **not duplicated here** — see `DESIGN.md`. This page
covers the shape of the system and what the contract test enforces.

## Posture

- **Translucent everywhere except the terminal.** Frosted surfaces use
  `backdrop-filter: blur(16-40px)` + a translucent fill over the opaline-mapped
  colors. The only opaque surface is `.log-panel` (terminal anchor) — brand
  posture.
- **One blue accent per view.** Primary actions use `var(--accent)`; no other
  rule writes blue.
- **6px controls, 8px panels, pill for chips.** Everything snaps to the
  small/medium/pill scale.
- **44px touch targets.** Every button, chip, search field, and nav item
  respects a 44px minimum.
- **Argo owns orange, kite mark owns teal.** `--argo` and `--brand` are the only
  absolute hex values added; every other primitive routes through opaline tokens.

## Token contract

Colors are supplied by the opaline theme engine, mapped onto the reference's
variable names in `crates/openkite-ui/src/theme_opaline.rs:19-41` and declared
in the order pinned by `crates/openkite-ui/src/theme.rs:20-50` — `--bg`,
`--surface`, `--surface-solid`, `--border`, `--fg`, `--muted`, `--subtle`,
`--accent`, `--progress`, `--success`, `--warn`, `--danger`, `--violet`, plus
the `--term-*` / `--term-bright-*` ANSI set. The `--bg-0` / `--fg-0` / `--bg-1`
aliases are gone (they were the pre-T3 contract; see OKT-174).

The design-system `:root` block adds the 16 properties the opaline theme does
not carry, and declares each exactly once:

| Group | Properties |
|---|---|
| Terminal anchor | `--terminal-bg`, `--terminal-fg` |
| Brand | `--brand` (kite teal), `--argo` (ArgoCD orange), `--on-accent` (text on fills) |
| Log semantics | `--log-info`, `--log-method`, `--log-error` |
| Fonts | `--font-sans`, `--font-mono` (IBM Plex + system stack) |
| Elevation | `--shadow-rest`, `--shadow-hover`, `--shadow-terminal` |
| Radii | `--r-sm` (6px), `--r-md` (8px), `--r-pill` (999px) |

## Primitive classes

The table below is the design-system vocabulary; the authoritative, test-pinned
list is `REQUIRED_CLASSES` in `crates/openkite-ui/tests/design.rs`.

| Class | Purpose |
|---|---|
| `.panel` | Frosted content surface (18px blur, translucent fill) |
| `.btn` + `.btn-primary` + `.btn-secondary` + `.btn-danger` | Buttons (44px touch, hover escalation) |
| `.chip` + `.chip.active` | Pill toggles (namespace chips, filter chips) |
| `.search-field` | Inline-icon search input (44px) |
| `.pill` + `.success` / `.warn` / `.danger` / `.muted` | Status badges (6px dot + label) |
| `.table-wrap`, `.resource-table`, `.table-header` | Tabular content + horizontal-scroll wrapper |
| `.resource-name` (+ `.icon`), `.health-dots` + `.dot` | Icon + mono-font name cell; inline-cell semantic dots |
| `.log-panel` (+ `.log-header`/`.log-handle`/`.log-body`/`.log-line`/`.log-time`/`.log-level`/`.log-method`/`.log-msg`/`.log-paused`) | Opaque terminal anchor (`var(--terminal-bg)`) |
| `.inspector` (+ `.inspector-header`/`.inspector-title`/`.inspector-eyebrow`/`.inspector-body`/`.inspector-actions`) + `.kv-list`/`.kv-row` | Slide-over panel (420px, right-anchored) |
| `.toast` + `.toast.show` | Bottom-anchored notification (340px max-width) |
| `.page-head`/`.eyebrow`/`.page-sub`/`.page-actions`, `.toolbar`/`.chip-row`, `.panel-footer`/`.pager` | Route chrome (heading row, filter toolbar, footer) |
| `.nav-section` + `.nav-item` (+ `.nav-badge`) | Sidebar navigation |
| `.modal-backdrop`/`.modal` (+ 7 children), `.field-input`/`.field-label`/`.field-helper`/`.field-error`, `.editor-textarea` | CRUD/edit surfaces |
| `.value-mask`/`.value-masked`/`.value-revealed`/`.value-actions`/`.reveal-btn` | Secret value masking |

## Namespace bar

One narrow line, never a section. The route toolbar's namespace selector is a
single `.ns-bar` row whose middle is the `.chip-row`:

| Class                | Purpose                                                              |
|----------------------|----------------------------------------------------------------------|
| `.ns-bar`            | The one-line row: search circle, scrolling strip, × reset            |
| `.chip-row`          | The strip — `flex-wrap: nowrap`, `overflow-x: auto`, `scroll-snap-type: x proximity`, scrollbar hidden (`scrollbar-width: none` plus `.chip-row::-webkit-scrollbar{display:none}`), each chip a `scroll-snap-align: start` target |
| `.chip` + `.chip.active` | A namespace chip (44px min height) and its selected state        |
| `.chip-mark`         | The check on a selected chip                                         |
| `.ns-search`         | The 44px search circle; opens the inline chip-list filter            |
| `.ns-filter`         | The inline field the circle opens — narrows the *list* only          |
| `.ns-reset`          | The 44px × circle; rendered only while a selection exists, after the strip so it is reachable without scrolling |

The selection is **one** selection for the whole console, held in
`openkite_ui::runtime::NAMESPACE_SELECTION` (empty = "all namespaces"). Every
data surface reads it through `namespace_bar::selection_matches`; `ResourceTable`
scopes its rows with it and owns no namespace chips of its own. The search field
narrows the *chip list* (`visible_namespaces(options, query)`) and never the
selection.

## Deferred to dependent tickets

- **ArgoCD-specific primitives** (`.app-grid`, `.app-card`, `.card-status`,
  `.source-icon`, `.tag`, `.card-meta`, `.card-swipe-actions`) — OKT-47
  (ArgoCD plugin), the first consumer.
- **Mobile bottom-nav** (`.bottom-nav`, `.bottom-tab`), pull-to-refresh
  (`.pull-indicator` is wired in `crates/openkite-ui/src/components/shell.rs:384`
  with a `data-pull` state; the touch gesture tickets own its behaviour),
  and card swipe — the mobile surface tickets (T9/T11/T12/T16).
- **Icon sprite** (32 inline `<symbol>` SVGs from the reference mockup) — OKT-47
  alongside the first consumer.

*Already landed, previously listed here:* the `@font-face` faces for IBM Plex
(vendored, OFL-1.1, `crates/openkite-ui/assets/fonts/`), the shim/filter
components, and the reference token names (`--bg`/`--surface`/…, OKT-174).

## Verification

`crates/openkite-ui/tests/design.rs` reads `crates/openkite-ui/assets/main.css`
through `openkite_ui::MAIN_CSS` and asserts:

- every property in `REQUIRED_PROPERTIES` is declared exactly once;
- every class in `REQUIRED_CLASSES` is present;
- no `PRE_EXISTING_PROPERTIES` entry (the opaline-mapped colors) is re-declared
  by the design-system block.

The test runs on every CI push with no kube/JS dependencies. Token **values**
are pinned separately by `DESIGN.md` and graded against this stylesheet — see
its `## Regeneration` section for the `design.md` commands.
