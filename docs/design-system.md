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
| `.app-grid` + `.app-card` | Application card grid and frosted card shell |
| `.card-swipe-actions` + `.card-action` | Card action column, revealed by the kebab or a swipe (44px buttons) |
| `.card-main` + `.card-status` | Card face (`translateX(-112px)` when `.swiped`) and its edge tone |
| `.card-menu-btn` | Card kebab (44px, always rendered — never hover-only) |
| `.card-body` + `.card-title-row` + `.app-name` + `.app-sub` | Card content block |
| `.source-icon` + `.card-footer` + `.badges` + `.card-meta` | Card source glyph, footer, status pills and age |

## Namespace bar

One narrow line, never a section. The route toolbar's namespace selector is a
single `.ns-bar` row whose middle is the `.chip-row`:

| Class                | Purpose                                                              |
|----------------------|----------------------------------------------------------------------|
| `.ns-bar`            | The one-line row: search circle, scrolling strip, × reset            |
| `.chip-row`          | The strip — `flex-wrap: nowrap`, `overflow-x: auto`, `scroll-snap-type: x mandatory`, scrollbar hidden (`scrollbar-width: none` plus `.chip-row::-webkit-scrollbar{display:none}`), each chip a `scroll-snap-align: start` target that `scroll-snap-stop: always` holds so a flick never rests between two chips |
| `.chip` + `.chip.active` | A namespace chip (44px min height) and its selected state        |
| `.chip-mark`         | The check on a selected chip                                         |
| `.ns-search`         | The 44px search circle; opens the chip-list filter                   |
| `.ns-filter`         | The inline field the circle shows — narrows the *list* only; on a coarse pointer it takes the full width **above** the strip |
| `.ns-reset`          | The 44px × circle; rendered only while a selection exists, after the strip so it is reachable without scrolling |

The selection is **one** selection for the whole console, held in
`openkite_ui::runtime::NAMESPACE_SELECTION` (empty = "all namespaces"). Every
data surface reads it through `namespace_bar::selection_matches`; `ResourceTable`
scopes its rows with it and owns no namespace chips of its own. The search field
narrows the *chip list* (`visible_namespaces(options, query)`) and never the
selection.

## Touch paths

Touch has no hover and no double-click, so every desktop-only affordance names
its substitute. Layout adapts under `@media (pointer: coarse)`; the hover
reward is opt-in under `@media (hover: hover) and (pointer: fine)`. No state is
reachable only by hover, and a hover rule may never hide something a touch
screen needs.

| Affordance | Pointer path | Touch path |
|------------|--------------|------------|
| Hover reward (lift, border, shadow) | `:hover` | not applied — the gate leaves the resting state visible |
| `title=` tooltip | the native tooltip | the same state is already in a label or attribute; nothing depends on the tooltip |
| Temporary dock tab (`.dock-tab[data-temporary]`) | double-click promotes it | long-press → **Keep tab** + a confirming toast (`openkite_ui::touch::resolve_press`, `keep_tab_toast`) |
| Dock tab manager (`.dock-menu`) | dropdown above the bar | bottom sheet (`position: fixed; bottom: 0`) whose rows meet the 44px floor |
| Namespace search (`.ns-search`) | tap opens the inline field | the same tap; the field spans the full width **above** the strip |
| Chip strip (`.chip-row`) | scroll, snapped | `scroll-snap-type: x mandatory` + `scroll-snap-stop: always` — a chip never rests half-visible |
| Dock resize (`.dock-resize`) | `pointerdown`/`move`/`up` drag | the same pointer drag, on a 22px grab strip (`touch-action: none`) |
| Side pane drawer toggle (`.menu-toggle` → `.sidebar.open`) | click opens the drawer | the same tap; `.sidebar-backdrop.show` is the tap-to-dismiss scrim |
| Side pane (`.inspector`) | a second click replaces the pane in place | the same tap; ≤767px the pane is a full-width sheet dismissed by the scrim |
| Side pane resize (`.inspector-resize`) | `pointerdown` drag on the pane's left edge | the same pointer drag (`touch-action: none`); ≤767px the handle is not rendered |
| Card actions (`.card-menu-btn` kebab) | click opens `.card-swipe-actions` | the same tap; the column is always rendered, never hover-revealed |
| Card swipe (`.card-main`) | n/a — a pointer drag scrolls the grid | a horizontal drag past `SWIPE_THRESHOLD_PX` (44px) reveals the column; `resolve_swipe` counts only a horizontal drag, and `resolve_tap` swallows the click that trails it, so a swipe never navigates |

The long-press rule is one pure function: movement past
`PRESS_MOVE_TOLERANCE_PX` latches a scroll, so a finger that drifts and settles
back can still never fire the long-press. The dock's adoption of it (and of the
`.dock-*` sheet rules) rides OKT-172's `DockView`; this ticket ships the rule,
the sheet stylesheet and the documented path.

## Deferred to dependent tickets

- **ArgoCD-specific primitives** (`.app-grid`, `.app-card`, `.card-status`,
  `.source-icon`, `.card-meta`, `.card-swipe-actions`) — OKT-47 (ArgoCD plugin)
  *consumes* the card primitives core ships with OKT-169; it adds none of its
  own.
- **Mobile bottom-nav** (`.bottom-nav`, `.bottom-tab`) and pull-to-refresh
  (`.pull-indicator` is wired in `crates/openkite-ui/src/components/shell.rs:384`
  with a `data-pull` state; the touch gesture tickets own its behaviour) —
  the mobile surface tickets (T9/T11/T16).
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

`crates/openkite-ui/tests/card_actions.rs` mounts `AppCard` headlessly and pins
the card's touch contract: the swipe column offers exactly the actions the
kebab's `aria-controls` names, `resolve_swipe`/`resolve_tap` make a swipe unable
to navigate, both the kebab and the action buttons clear the 44px floor in the
stylesheet, and every card `:hover` rule sits behind a `(hover: hover)` guard.

The test runs on every CI push with no kube/JS dependencies. Token **values**
are pinned separately by `DESIGN.md` and graded against this stylesheet — see
its `## Regeneration` section for the `design.md` commands.
