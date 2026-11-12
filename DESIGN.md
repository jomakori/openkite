---
version: alpha
name: OpenKite — Liquid Frost Glass
description: >-
  Zed's tight precision on Apple-style translucent depth: a cool light canvas,
  frosted surfaces over it, and exactly one opaque dark surface — the terminal
  anchor. Every value here is the OpenDesign reference project's own token,
  adopted verbatim and declared by the console stylesheet as the CSS custom
  property named in ## Provenance.
colors:
  bg: "#f5f7fa"
  surface: "rgba(255, 255, 255, 0.85)"
  surface-solid: "rgba(255, 255, 255, 0.97)"
  border: "#e2e4e9"
  fg: "#262c34"
  muted: "#6e7581"
  subtle: "#999fa8"
  primary: "#4d8ce8"
  accent: "#4d8ce8"
  progress: "#4d8ce8"
  success: "#4d9a5e"
  warn: "#c4841d"
  danger: "#e05252"
  violet: "#8b5cf6"
  brand: "#0d9488"
  argo: "#ef7b4d"
  on-accent: "#070e16"
  terminal-bg: "#1e2024"
  terminal-fg: "#abb2bf"
  log-info: "#98c379"
  log-method: "#61afef"
  log-error: "#e06c75"
typography:
  h1:
    fontFamily: IBM Plex Sans
    fontSize: 32px
    fontWeight: 600
    lineHeight: 1.08
    letterSpacing: "-0.02em"
  body:
    fontFamily: IBM Plex Sans
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.5
  eyebrow:
    fontFamily: IBM Plex Sans
    fontSize: 10px
    fontWeight: 600
    letterSpacing: "0.09em"
  control:
    fontFamily: IBM Plex Sans
    fontSize: 12px
    fontWeight: 500
    letterSpacing: "0.01em"
  table:
    fontFamily: IBM Plex Mono
    fontSize: 12px
    fontWeight: 400
rounded:
  sm: 6px
  md: 8px
  pill: 999px
spacing:
  xs: 6px
  sm: 8px
  md: 12px
  lg: 20px
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.sm}"
    padding: 16px
    height: 44px
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "#ffffff"
    rounded: "{rounded.sm}"
    padding: 16px
    height: 44px
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.sm}"
    padding: 16px
    height: 44px
  chip:
    backgroundColor: "{colors.surface-solid}"
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    padding: 14px
    height: 44px
  chip-active:
    backgroundColor: "{colors.fg}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    height: 44px
  search-field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.sm}"
    height: 44px
  panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    padding: 16px
  resource-table:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    typography: "{typography.table}"
  log-panel:
    backgroundColor: "{colors.terminal-bg}"
    textColor: "{colors.terminal-fg}"
    rounded: "{rounded.md}"
    typography: "{typography.table}"
  inspector:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    width: 420px
  toast:
    backgroundColor: "{colors.fg}"
    textColor: "#ffffff"
    rounded: "{rounded.md}"
    width: 340px
  pill:
    backgroundColor: "{colors.surface-solid}"
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    padding: 10px
  pill-success:
    backgroundColor: "color-mix(in srgb, {colors.success} 11%, white)"
    textColor: "color-mix(in srgb, {colors.success} 62%, {colors.fg})"
    rounded: "{rounded.pill}"
  pill-warn:
    backgroundColor: "color-mix(in srgb, {colors.warn} 11%, white)"
    textColor: "color-mix(in srgb, {colors.warn} 62%, {colors.fg})"
    rounded: "{rounded.pill}"
  pill-danger:
    backgroundColor: "color-mix(in srgb, {colors.danger} 11%, white)"
    textColor: "color-mix(in srgb, {colors.danger} 62%, {colors.fg})"
    rounded: "{rounded.pill}"
---

## Overview

OpenKite's console is one visual language across two hosts — the Dioxus desktop
webview and the browser host — and it is defined by exactly two artifacts:

- the vendored reference stylesheet,
  `docs/reference/opendesign-console/openkite-console.css`, whose `:root` block
  is the palette below.
- this file — the token contract the console is graded against.

This file is a contract, not a description of shipped code: the console crate
carries no stylesheet yet, so every rule below states what the next
`openkite-ui` stylesheet must declare — the same custom-property names, in one
stylesheet, injected once per host.

The palette is **not invented here**. It is the OpenDesign reference project's
own token set, adopted verbatim; every colour below is the same value the
reference declares, resolved from `oklch()` to sRGB hex. `## Provenance` records
each token's reference custom property alongside the hex the console must
declare.

Posture, in four rules:

- **Translucent everywhere except the terminal.** Frosted surfaces are a
  translucent fill plus `backdrop-filter: blur(16–40px)`. The only opaque dark
  surface is `.log-panel` (`{colors.terminal-bg}`).
- **One blue accent per view.** `{colors.accent}` is the only interaction
  colour. Status colours (`success` / `warn` / `danger`) are semantic and never
  decorative.
- **Argo owns orange, the kite mark owns teal.** `{colors.argo}` fills the Argo
  CD plugin zone and nothing else; `{colors.brand}` is the kite mark, full stop.
- **44px touch floor.** Every button, chip, search field, nav item and log
  action is at least 44×44px.

## Colors

Semantic names, not colour names — `success` is what a healthy pod is, not that
its green is `#4d9a5e`.

- **Canvas — `bg` / `surface` / `surface-solid`.** `bg` is the page. `surface`
  (`rgba(255,255,255,0.85)`) is a frosted panel over it; `surface-solid`
  (`0.97`) is the frosted-but-almost-solid fill for chips, pills and controls,
  where text sits directly on the fill and needs the extra opacity.
- **Ink — `fg` / `muted` / `subtle`.** `fg` is body, headings and inverted fills
  (active chip, toast). `muted` is secondary ink (table meta, chips, log pods);
  `subtle` is placeholders, disabled ink and the eyebrow.
- **Interaction — `primary` / `accent` / `on-accent`.** `primary` is the
  design.md-conventional alias of `accent`; both are `#4d8ce8`. `on-accent` is
  the near-black ink for text on a bright fill (kite mark, brand chips).
- **Status — `success` / `warn` / `danger` / `progress`.** `progress` shares the
  accent hue deliberately: a progressing resource is an "in-flight" state, not a
  warning. Nothing else may use `progress`.
- **Domain — `brand` / `argo` / `violet`.** `brand` (`#0d9488`) is the kite
  mark; `argo` (`#ef7b4d`) is the Argo CD plugin accent zone; `violet`
  (`#8b5cf6`) is the controller tag in the resource table.
- **Terminal — `terminal-bg` / `terminal-fg` / `log-info` / `log-method` /
  `log-error`.** The one opaque surface. `log-info` colours the pod icon and
  info-level text, `log-method` the HTTP method / path tags, `log-error` the
  error-level lines. They intentionally equal the xterm ANSI green / blue / red
  so the dock and a live terminal agree.

The `--term-*` / `--term-bright-*` ANSI set (16 values, declared on `:root`
beside the palette above) is an OpenKite extension the reference does not carry —
xterm needs a full ANSI palette. design.md has no ANSI-palette concept, so it is
outside the `colors:` set above; it is pinned instead by those `--term-*` custom
properties in the console stylesheet, which the terminal surface reads and no
other surface may reference.

## Typography

Two families, both OFL-1.1, shipped as vendored asset files by the console crate
and declared by `@font-face` in its stylesheet (`--font-sans` / `--font-mono` in
the reference `:root`):

- **IBM Plex Sans** (400/500/600/700) for all UI text.
- **IBM Plex Mono** (400/500/600) for anything a cluster emitted or that must
  align in columns: log lines, resource names, pod names, counts.

Rules that come from the reference, not from taste:

- `h1` is `clamp(24px, 2vw, 32px)` at weight 600 with `-0.02em` tracking — the
  clamp is what keeps the page head from shouting on a 1280px window; `32px` is
  the resolved desktop value recorded above.
- UI type is small and dense on purpose: 13px body, 12px controls and table,
  10–10.5px for uppercase eyebrows and pills. The uppercase eyebrow always
  carries `0.09em` tracking; controls carry `0.01–0.02em`.

## Layout

- **Density.** Table and log text is 12px mono; panels use 16px body padding
  (`.panel`), route chrome uses 16–20px gaps between blocks.
- **Rhythm.** `spacing.xs` (6px) is the gap inside a button or pill,
  `spacing.sm` (8px) the gap inside a chip row or table toolbar, `spacing.md`
  (12px) the search-field and field-input padding, `spacing.lg` (20px) the page
  head gap / route gutter.
- **The sidebar is fixed.** It never collapses to icons at desktop width; below
  the tablet breakpoint it becomes an off-canvas drawer behind
  `.sidebar-backdrop`, with `.bottom-nav` carrying the mobile tabs. That mobile
  posture is the reference's, not a second design — the reference's
  `@media (max-width: 1024px)` and `@media (max-width: 767px)` blocks move
  `.sidebar` off-canvas and reveal `.bottom-nav`. It is deferred along with the
  rest of the touch surfaces (`docs/architecture.md` § 6), so it is a contract
  for that work, not a v1 surface.

## Elevation & Depth

Three states, no more. Depth is what tells you a surface can be interacted with.

- `--shadow-rest` — a panel, a table wrap or a button at rest.
- `--shadow-hover` — a hovered control, the inspector, or the toast.
- `--shadow-terminal` — the log dock, the one surface that reads as sunk into
  the page rather than lifted off it.

Blur is a token too, and it is *layered*, not decorative: 40px for branded frost
(theme rows), 20px for shell chrome (sidebar, topbar), 24px for the inspector,
18px for content panels, 16px for Argo cards. It is a five-value set; a sixth
blur radius is a sixth elevation state by another name, and the blur set is
closed for the same reason the elevation set is.

## Shapes

- `--r-sm` (6px) — controls: buttons, chips, search fields, inputs, log meta.
- `--r-md` (8px) — surfaces: panels, table wrap, cards, inspector, toast.
- `--r-pill` (999px) — pills and namespace chips. The reference writes the
  `999px` literal at `.chip` / `.pill`; `--r-pill` is the repo's formalisation
  of it, and is the value the contract test pins.

Nothing else gets a radius. A radius outside this set is a defect, not a style.

## Components

Each entry names the selectors the console stylesheet must define, unless marked
*(reference-only)*; the reference anchor for the same shape is the vendored
`openkite-console.css`. Values come from `colors:` / `rounded:` / `spacing:`
above, never from a new literal.

- **Buttons** — `.btn` is the base: 44px minimum height, `r-sm`, `surface` fill,
  `border` edge, 13px label. `.btn-primary` is the single high-emphasis action on
  a view: `accent` fill, white text; hover mixes the fill toward `fg`
  (`color-mix(in srgb, var(--accent) 90%, var(--fg))`). `.btn-secondary` is the
  `surface` fill with a `border` edge.
- **Chips** — `.chip` for namespace and filter toggles: pill radius, 44px,
  `surface-solid` fill at rest, `fg` fill + white text when active
  (`.chip.active`). The chip is the namespace surface's only control shape.
- **Search field** — `.search-field`: 44px, `surface` fill, `sm` radius, 12px
  inline padding, `accent` border on `:focus-within`, `subtle` placeholder.
- **Pills** — `.pill` plus the `.success` / `.warn` / `.danger` / `.muted`
  variants. A 6px `::before` dot inherits `currentColor`, so the dot and the
  label always agree. Variants are derived from the status tokens with
  `color-mix(in srgb, <token> 11%, white)` fill, `24%` border and
  `62%`-toward-`fg` text — do not re-pick tint percentages per view.
- **Panels** — `.panel`: frosted content surface, `surface` fill, `border`, `md`
  radius, `shadow-rest`, 18px blur, 16px padding.
- **Resource table** — `.table-wrap` / `.resource-table`, mono 12px;
  `.table-header` is uppercase 10px sans with `0.075em` tracking;
  `.resource-name` is mono with a 14px `subtle` icon; `.health-dots` and
  `.dot.ok` / `.dot.warn` / `.dot.err` carry inline state.
- **Log panel** — `.log-panel`, the terminal anchor: `terminal-bg` fill,
  `terminal-fg` text, mono 12px, `md` radius, `shadow-terminal`, 268px max
  height, 44px header actions.
- **Inspector** — `.inspector`: slide-over from the right, `min(420px, 100%)`,
  `surface` fill + 24px blur, `shadow-hover`, `.kv-list` key/value rows,
  `.inspector-actions` footer.
- **Toast** — `.toast`: bottom-anchored at 80px, 220–340px wide, `fg` fill with
  white text, `md` radius, `shadow-hover`. It reads as a system message, which is
  why it is the only light surface that inverts to `fg`.
- **Argo card / card status / bottom nav** *(reference-only — outside this
  contract)* — reference `openkite-console.css` `.app-grid`, `.app-card`,
  `.card-status` with `synced` / `outofsync` / `degraded` / `progressing`,
  `.card-swipe-actions`, `.tag`, `.source-icon`, and `.bottom-nav` /
  `.bottom-tab`. If they land, the status band and swipe tray must consume the
  tokens above, not new values.

Where this contract and the reference differ, this contract is normative — it is
what the console ships. The known deltas are deliberate: `.btn` padding (16px vs
14px) and label size (13px vs 12px), the search field as a flex wrapper rather
than an absolutely-positioned icon, `color-mix(in srgb, …)` with `fg`/`border`
mixes rather than the reference's `oklab`-with-black/white, and the
inspector/toast fills noted above.

## Accessibility
## Accessibility

The contract is audited, not assumed. `npx -y @google/design.md lint DESIGN.md`
reports **0 errors** and exactly one real finding, which is recorded here rather
than silently "fixed", because the fix is a stylesheet change owned by the
consuming surface tickets:

- **`button-primary` label contrast is 3.37:1** — `#ffffff` on `{colors.accent}`
  `#4d8ce8`, below the WCAG AA 4.5:1 floor for normal text (the label is
  `.btn-primary`'s 13px `control` size). It passes AA for large text only. Candidate fixes, in
  preference order: (1) darken the primary fill for the button specifically,
  (2) use `{colors.on-accent}` ink on the accent fill, (3) raise the label to
  18.66px bold. Whichever lands must come with a token value here so the next
  surface inherits the fixed pair. Tracked as a follow-up; no surface may
  introduce a *new* pair below 4.5:1 in the meantime.

The remaining lint warnings are the linter's blind spots, not defects: it cannot
follow `color-mix()` into the pill variants, and it flags tokens that are
consumed by CSS rules rather than by a `components:` entry. Semantic tokens
(`success`, `warn`, `danger`, `progress`, `brand`, `argo`, `log-*`, `violet`,
`border`, `subtle`, `bg`) fall in that bucket.

## Do's and Don'ts

- **Do** reference tokens by name; never inline a hex that is not in `colors`.
- **Do** keep one accent per view — a second blue is a defect.
- **Do** keep every interactive target at 44px, including the log header's icon
  buttons.
- **Don't** give the terminal a light variant; the opaque dark anchor is the
  brand.
- **Don't** use `argo` outside the Argo CD plugin zone, or `brand` outside the
  kite mark.
- **Don't** widen the radius set, the elevation set, or the blur set — each has
  exactly the entries listed above.

## Provenance

Every value above resolves to a vendored file. The `reference` column is the
custom property and value in
`docs/reference/opendesign-console/openkite-console.css`; the `console` column is
the value the console stylesheet must declare for the same name. The `oklch()`
spelling is kept beside the hex so the derivation is checkable — e.g.
`--accent: oklch(0.6420 0.1531 257.9)` = `#4d8ce8`.

| Token | reference — `:root` in `openkite-console.css` | console — value to declare |
|---|---|---|
| `bg` | `--bg` `oklch(0.9755 0.0045 258.3)` | `#f5f7fa` |
| `surface` | `--surface` `oklch(1 0 0 / 0.85)` | `rgba(255, 255, 255, 0.85)` |
| `surface-solid` | `--surface-solid` `oklch(1 0 0 / 0.97)` | `rgba(255, 255, 255, 0.97)` |
| `border` | `--border` `oklch(0.9188 0.0071 268.5)` | `#e2e4e9` |
| `fg` | `--fg` `oklch(0.29 0.018 258)` | `#262c34` |
| `muted` | `--muted` `oklch(0.56 0.02 262)` | `#6e7581` |
| `subtle` | `--subtle` `oklch(0.70 0.015 262)` | `#999fa8` |
| `accent` / `primary` | `--accent` `oklch(0.6420 0.1531 257.9)` | `#4d8ce8` |
| `progress` | `--progress` `oklch(0.6420 0.1531 257.9)` | `#4d8ce8` |
| `success` | `--success` `oklch(0.6212 0.1182 149.1)` | `#4d9a5e` |
| `warn` | `--warn` `oklch(0.6629 0.1331 72.6)` | `#c4841d` |
| `danger` | `--danger` `oklch(0.6294 0.1776 23.7)` | `#e05252` |
| `violet` | `--violet` `oklch(0.6056 0.2189 292.7)` | `#8b5cf6` |
| `terminal-bg` | `--terminal-bg` `oklch(0.2431 0.0082 264.4)` | `#1e2024` |
| `terminal-fg` | `--terminal-fg` `oklch(0.7621 0.0202 263.0)` | `#abb2bf` |
| `brand` | `--brand` `oklch(0.6002 0.1038 184.7)` | `#0d9488` |
| `argo` | `--argo` `oklch(0.7069 0.1554 41.6)` | `#ef7b4d` |
| `on-accent` | `--on-accent` `oklch(0.16 0.02 250)` | `#070e16` |
| `log-info` | `--log-info` `oklch(0.7683 0.1103 133.0)` | `#98c379` |
| `log-method` | `--log-method` `oklch(0.7304 0.1213 245.3)` | `#61afef` |
| `log-error` | `--log-error` `oklch(0.6709 0.1448 17.0)` | `#e06c75` |
| `font-sans` | `--font-sans` (IBM Plex Sans stack) | the same stack |
| `font-mono` | `--font-mono` (IBM Plex Mono stack) | the same stack |
| `shadow-rest` | `--shadow-rest` | the same value |
| `shadow-hover` | `--shadow-hover` | the same value |
| `shadow-terminal` | `--shadow-terminal` | the same value |
| `r-sm` | `--r-sm` `6px` | `6px` |
| `r-md` | `--r-md` `8px` | `8px` |
| `r-pill` | no `:root` property — `border-radius: 999px` on `.chip` and `.pill` | `999px` |

Component geometry comes from the reference rules named in `## Components` — `h1`,
`.view`, `.btn`, `.chip`, `.search-field input`, `.panel`, `.resource-table`,
`.pill`, `.log-body`, `.app-card`, `.inspector`, `.toast`. Each surface's mapping
to those rules, and the tokens it consumes, is `docs/design-surfaces.md`.

## Regeneration

The reference is vendored, so this file can be re-derived and re-checked
offline:

```bash
# 1. lint the token contract (structure, token refs, WCAG contrast)
npx -y @google/design.md lint DESIGN.md

# 2. grade the contract against the vendored reference `:root` block: every
#    --color-* line below must equal the matching custom property in
#    docs/reference/opendesign-console/openkite-console.css (21/21 colour vars
#    resolve — the exporter normalises rgba() to #rrggbbaa, --color-primary is
#    the accent alias, and the four --spacing-* tokens are literal gaps rather
#    than properties, so they are not comparable)
npx -y @google/design.md export --format css-vars DESIGN.md

# 3. machine-readable export for the crate (not committed — DESIGN.md is the
#    source of truth; this is a build product). The CLI also offers
#    Tailwind-format exports; the repo deliberately does not adopt a
#    Tailwind layer, so they are unused here.
npx -y @google/design.md export --format dtcg         DESIGN.md > tokens.json
```

`design.md` has no typed slot for shadows or font stacks, so the three
`--shadow-*` values and the two `--font-*` stacks are pinned by the
`## Provenance` table and by the reference `:root` block, not by the YAML token
block.

The console crate's design test is the enforcement point, and it does not exist
yet: when it lands it must pin the vendored reference by SHA-256, derive the
class and token vocabulary from `openkite-console.css`, and fail when the
shipped stylesheet is missing a name from this file or adds a value that is not
here. The pinned hashes are in `docs/design-surfaces.md` § Reference contract.

To re-fetch the reference project instead of using the vendored copy, see
`docs/reference/opendesign-console/README.md`.
