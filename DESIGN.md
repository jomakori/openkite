---
version: alpha
name: OpenKite — Liquid Frost Glass
description: >-
  Zed's tight precision on Apple-style translucent depth: a cool light canvas,
  frosted surfaces over it, and exactly one opaque dark surface — the terminal
  anchor. Every value here is the shipped value from
  crates/openkite-ui/assets/main.css; the palette is the OpenDesign reference
  project's own token set, adopted verbatim (see ## Provenance).
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

- `crates/openkite-ui/assets/main.css` — the shipped stylesheet
  (`openkite_ui::MAIN_CSS`), injected by the host into the webview `<head>`.
- this file — the token contract those surfaces are graded against.

The palette is **not invented here**. It is the OpenDesign reference project's
own token set, adopted verbatim; every colour below is the same value the
reference declares, resolved from `oklch()` to sRGB hex. `## Provenance` records
the two file:line anchors (repo and reference) for each token, and
`crates/openkite-ui/tests/design.rs` pins the shipped stylesheet to the same
name set at CI time.

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

The `--term-*` / `--term-bright-*` ANSI set (16 values, `main.css:18-33`) is an
OpenKite extension the reference does not carry — xterm needs a full ANSI
palette. design.md has no ANSI-palette concept, so it is outside the `colors:`
set above and is pinned instead by `CSS_VARS` in
`crates/openkite-ui/src/theme.rs:20-50`.

## Typography

Two families, both vendored (OFL-1.1, `crates/openkite-ui/assets/fonts/`,
`@font-face` at `main.css:57-111`):

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
  posture is the reference's, not a second design — see the reference's
  `@media` blocks at `openkite-console.css:858-967`.

## Elevation & Depth

Three states, no more. Depth is what tells you a surface can be interacted with.

- `--shadow-rest` — a panel, a table wrap or a button at rest.
- `--shadow-hover` — a hovered control, the inspector, or the toast.
- `--shadow-terminal` — the log dock, the one surface that reads as sunk into
  the page rather than lifted off it.

Blur is a token too, and it is *layered*, not decorative: 40px for branded frost
(theme rows), 20px for shell chrome (sidebar, topbar), 24px for the inspector,
18px for content panels, 16px for Argo cards. The typed view lives in
`crates/openkite-ui/src/design/tokens.rs` (`BLUR_FROST`, `BLUR_TOPBAR`,
`BLUR_PANEL`).

## Shapes

- `--r-sm` (6px) — controls: buttons, chips, search fields, inputs, log meta.
- `--r-md` (8px) — surfaces: panels, table wrap, cards, inspector, toast.
- `--r-pill` (999px) — pills and namespace chips. The reference writes the
  `999px` literal at `.chip` / `.pill`; `--r-pill` is the repo's formalisation
  of it, and is the value the contract test pins.

Nothing else gets a radius. A radius outside this set is a defect, not a style.

## Components

All of the following ship in `crates/openkite-ui/assets/main.css` today, unless
marked *(reference-only)*.

- **Buttons** (`main.css:569-602`) — `.btn` is the base: 44px minimum height,
  `r-sm`, `surface` fill, `border` edge, 13px label. `.btn-primary` is the
  single high-emphasis action on a view: `accent` fill, white text; hover mixes
  the fill toward `fg` (`color-mix(in srgb, var(--accent) 90%, var(--fg))`).
  `.btn-secondary` is the `surface` fill with a `border` edge.
- **Chips** (`main.css:604-624`) — namespace / filter toggles: pill radius, 44px,
  `surface-solid` fill at rest, `fg` fill + white text when active. The chip is
  the namespace surface's only control shape.
- **Search field** (`main.css:625-644`) — 44px, `surface` fill, `sm` radius,
  12px inline padding, `accent` border on `:focus-within`, `subtle` placeholder.
- **Pills** (`main.css:659-693`) — `.pill` + `.success` / `.warn` / `.danger` /
  `.muted`. A 6px `::before` dot inherits `currentColor`, so the dot and the
  label always agree. Variants are derived from the status tokens with
  `color-mix(in srgb, <token> 11%, white)` fill, `24%` border and `62%`-toward-`fg`
  text — do not re-pick tint percentages per view.
- **Panels** (`main.css:557-566`) — frosted content surface: `surface` fill,
  `border`, `md` radius, `shadow-rest`, 18px blur, 16px padding.
- **Resource table** (`main.css:703-759`) — mono 12px; `.table-header` is
  uppercase 10px sans with `0.075em` tracking; `.resource-name` is mono with a
  14px `subtle` icon; `.health-dots` / `.dot.ok|.warn|.err` carry inline state.
- **Log panel** (`main.css:770-800`) — the terminal anchor: `terminal-bg` fill,
  `terminal-fg` text, mono 12px, `md` radius, `shadow-terminal`, 268px max
  height, 44px header actions.
- **Inspector** (`main.css:813-872`) — slide-over from the right,
  `min(420px, 100%)`, `surface` fill + 24px blur, `shadow-hover`, `.kv-list`
  key/value rows, `.inspector-actions` footer.
- **Toast** (`main.css:874-890`) — bottom-anchored at 80px, 220–340px wide,
  `fg` fill with white text, `md` radius, `shadow-hover`. It reads as a system
  message, which is why it is the only light surface that inverts to `fg`.
- **Argo card / card status / bottom nav** *(reference-only — not yet shipped;
  landing with OKT-47 and the mobile surface ticket)* — reference
  `openkite-console.css:571-695` (`.app-grid`, `.app-card`, `.card-status` with
  `synced`/`outofsync`/`degraded`/`progressing`, `.card-swipe-actions`,
  `.tag`, `.source-icon`) and `:696-726` (`.bottom-nav`, `.bottom-tab`). When
  they land, the status band and swipe tray must consume the tokens above, not
  new values.

Where the repo and the reference differ, the repo is normative — it is what
ships. The known deltas are deliberate: `.btn` padding (16px vs 14px) and label
size (13px vs 12px), the search field as a flex wrapper rather than an
absolutely-positioned icon, `color-mix(in srgb, …)` with `fg`/`border` mixes
rather than the reference's `oklab`-with-black/white, and the inspector/toast
fills noted above.

## Accessibility

The contract is audited, not assumed. `npx -y @google/design.md lint DESIGN.md`
reports **0 errors** and exactly one real finding, which is recorded here rather
than silently "fixed", because the fix is a stylesheet change owned by the
consuming surface tickets:

- **`button-primary` label contrast is 3.37:1** — `#ffffff` on `{colors.accent}`
  `#4d8ce8`, below the WCAG AA 4.5:1 floor for normal text (the label is 13px,
  `main.css:588-592`). It passes AA for large text only. Candidate fixes, in
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

Every value above resolves to a shipped file. `repo` is the value the crate
actually renders; `reference` is the OpenDesign source token, vendored in-repo
at `docs/reference/opendesign-console/`. The `oklch()` spelling is shown beside
the resolved hex so the derivation is checkable — e.g.
`oklch(0.642 0.1531 257.9)` = `#4d8ce8`.

| Token | repo — `crates/openkite-ui/assets/main.css` | reference — `docs/reference/opendesign-console/openkite-console.css` |
|---|---|---|
| `bg` | `:3` `#f5f7fa` | `:2` `oklch(0.9755 0.0045 258.3)` |
| `surface` | `:4` `rgba(255,255,255,.85)` | `:3` `oklch(1 0 0 / 0.85)` |
| `surface-solid` | `:5` `rgba(255,255,255,.97)` | `:4` `oklch(1 0 0 / 0.97)` |
| `border` | `:6` `#e2e4e9` | `:8` `oklch(0.9188 0.0071 268.5)` |
| `fg` | `:7` `#262c34` | `:5` `oklch(0.29 0.018 258)` |
| `muted` | `:8` `#6e7581` | `:6` `oklch(0.56 0.02 262)` |
| `subtle` | `:9` `#999fa8` | `:7` `oklch(0.70 0.015 262)` |
| `accent` / `primary` | `:10` `#4d8ce8` | `:9` `oklch(0.6420 0.1531 257.9)` |
| `progress` | `:11` `#4d8ce8` | `:10` `oklch(0.6420 0.1531 257.9)` |
| `success` | `:12` `#4d9a5e` | `:11` `oklch(0.6212 0.1182 149.1)` |
| `warn` | `:13` `#c4841d` | `:12` `oklch(0.6629 0.1331 72.6)` |
| `danger` | `:14` `#e05252` | `:13` `oklch(0.6294 0.1776 23.7)` |
| `violet` | `:15` `#8b5cf6` | `:14` `oklch(0.6056 0.2189 292.7)` |
| `terminal-bg` | `:16` `#1e2024` | `:18` `oklch(0.2431 0.0082 264.4)` |
| `terminal-fg` | `:17` `#abb2bf` | `:19` `oklch(0.7621 0.0202 263.0)` |
| `brand` | `:38` `#0d9488` | `:16` `oklch(0.6002 0.1038 184.7)` |
| `argo` | `:39` `#ef7b4d` | `:15` `oklch(0.7069 0.1554 41.6)` |
| `on-accent` | `:40` `#070e16` | `:17` `oklch(0.16 0.02 250)` |
| `log-info` | `:41` `#98c379` | `:20` `oklch(0.7683 0.1103 133.0)` |
| `log-method` | `:42` `#61afef` | `:21` `oklch(0.7304 0.1213 245.3)` |
| `log-error` | `:43` `#e06c75` | `:22` `oklch(0.6709 0.1448 17.0)` |
| `font-sans` | `:44` | `:23` (identical stack) |
| `font-mono` | `:45` | `:24` (identical stack) |
| `shadow-rest` | `:46` | `:25` (identical) |
| `shadow-hover` | `:47` | `:26` (identical) |
| `shadow-terminal` | `:48` | `:27` (identical) |
| `r-sm` | `:49` `6px` | `:28` `6px` |
| `r-md` | `:50` `8px` | `:29` `8px` |
| `r-pill` | `:51` `999px` | `.chip` `:344`, `.pill` `:458` — the `999px` literal |

Component geometry comes from the shipped rules cited in `## Components`;
the reference originals are `h1` `:275-280`, `.view` padding `:266`, `.btn`
`:300-309`, `.chip` `:341-350`, `.search-field input` `:372-384`, `.panel`
`:385-393`, `.resource-table` `:395-427`, `.pill` `:453-469`, `.log-panel`
`:491-498`, `.app-card` `:577-588`, `.inspector` `:746-760`, `.toast`
`:837-855`. Full surface-level mapping: `docs/design-surfaces.md`.

## Regeneration

The reference is vendored, so this file can be re-derived and re-checked
offline:

```bash
# 1. lint the token contract (structure, token refs, WCAG contrast)
npx -y @google/design.md lint DESIGN.md

# 2. grade the contract against the stylesheet the crate actually ships:
#    every --color-* line below must equal the matching :root value in
#    crates/openkite-ui/assets/main.css (as of this commit: 21/21 colour
#    vars resolve — 19 byte-identical, 2 equal modulo the exporter's
#    rgba -> #rrggbbaa alpha normalisation; --color-primary is the
#    accent alias, and the four --spacing-* tokens are literal gaps, not
#    vars; the 3 --rounded-* tokens match --r-sm/--r-md/--r-pill)
npx -y @google/design.md export --format css-vars DESIGN.md

# 3. machine-readable export for the crate (not committed — DESIGN.md is
#    the source of truth; this is a build product). The CLI also offers
#    Tailwind-format exports; the repo deliberately does not adopt a
#    Tailwind layer, so they are unused here.
npx -y @google/design.md export --format dtcg         DESIGN.md > tokens.json

# 4. re-check the CSS contract layer the crate ships
cargo test -p openkite-ui --test design
```

`design.md` has no typed slot for shadows or font stacks, so the three
`--shadow-*` strings and the two `--font-*` stacks are pinned by the
`## Provenance` table and by `crates/openkite-ui/tests/design.rs` (which asserts
both), not by the YAML token block.

To re-fetch the reference project instead of using the vendored copy, see
`docs/reference/opendesign-console/README.md`.
