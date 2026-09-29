# OpenKite Design System — Liquid Frost Glass

The design system is the **token + primitive CSS layer** that every Phase-2
view ticket (OKT-42 Settings, OKT-43 CRUD, OKT-47 ArgoCD plugin, future
views) consumes without re-deciding the visual language.

It lives in two places:

- `crates/openkite-ui/assets/main.css` — the shell stylesheet, inlined into
  both hosts' `<head>` (`openkite_ui::MAIN_CSS`). It declares the vendored
  typefaces with `@font-face`.
- `crates/openkite-ui/src/design/{mod.rs, tokens.rs}` — typed Rust view of the
  surface-treatment strings (blur radii, panel radii, shadow strings)
  so Dioxus `style:` attributes can reference them by name.

## Posture

- **Translucent everywhere except the terminal.** Frosted surfaces use
  `backdrop-filter: blur(18-40px)` + a translucent fill over the
  existing opaline-mapped colors. The only opaque surface is the
  `.log-panel` (terminal anchor) — brand posture.
- **One blue accent per view.** Primary actions use `var(--accent)`;
  no other rule writes blue.
- **6px controls, 8px panels.** Pills and chips use the pill radius;
  everything else snaps to the small/medium scale.
- **44px touch targets.** Every button, chip, search field, and nav
  item respects a 44px minimum.
- **Argo owns orange, kite mark owns teal.** `--argo` and `--brand`
  are the only absolute hex values added; every other primitive routes
  through opaline tokens.

## Token contract

### Already shipped (opaline-mapped, do not redeclare)

From `src/theme_opaline.rs:19-41` and `src/theme.rs:18-47`:
`--bg-0`, `--bg-1`, `--bg-2`, `--border`, `--fg-0`, `--fg-1`, `--fg-2`,
`--accent`, `--green`, `--yellow`, `--red`, `--violet`, and the full
`--term-*` / `--term-bright-*` set.

### Design-system additions (12 new custom properties)

| Group       | Properties                                                                 |
|-------------|----------------------------------------------------------------------------|
| Brand       | `--brand` (kite teal), `--argo` (ArgoCD orange), `--on-accent` (text on fills) |
| Fonts       | `--font-sans`, `--font-mono` (vendored IBM Plex, then the system stack)   |
| Elevation   | `--shadow-rest`, `--shadow-hover`, `--shadow-terminal`                    |
| Radii       | `--r-sm` (6px), `--r-md` (8px), `--r-pill` (999px)                        |

## Vendored typefaces

`crates/openkite-ui/assets/fonts/` holds the faces those two tokens resolve to:
IBM Plex Sans 400/500/600/700 and IBM Plex Mono 400/500/600, IBM's Latin1 web
subsets, under SIL OFL 1.1 (`OFL.txt`, provenance and checksums in
`SOURCE.txt`). `assets/main.css` declares them with `@font-face`, so no page
asks an external font host for type.

The faces reach each host differently, because only one of them has an origin:

| Host                       | How the bytes arrive                                                                 | Cost                                                            |
|----------------------------|--------------------------------------------------------------------------------------|-----------------------------------------------------------------|
| Browser (`openkite-web`)   | `GET /assets/fonts/{file}`, the URL each `@font-face` names; `immutable`              | 136.5 KiB, fetched once per install, then cached                 |
| Desktop (wry webview)      | `openkite_ui::assets::embedded_css()` — the same stylesheet with every URL as a `data:` URI | ~182 KiB of base64 in the window head, and no request at all     |

## Primitive classes

| Class                        | Purpose                                                     |
|------------------------------|-------------------------------------------------------------|
| `.panel`                     | Frosted content surface (18px blur, translucent fill)       |
| `.btn` + `.btn-primary` + `.btn-secondary` | Buttons (44px touch, hover escalation)         |
| `.chip` + `.chip.active`     | Pill toggles (namespace chips, filter chips)                |
| `.search-field`              | Inline-icon search input (44px, 12px padding)               |
| `.pill` + semantic variants  | `.success`, `.warn`, `.danger`, `.muted` status badges      |
| `.table-wrap`                | Horizontal-scroll wrapper for tabular content               |
| `.resource-name` (+ `.icon`) | Icon + mono-font name cell                                  |
| `.log-panel` (+ 7 children)  | Opaque terminal anchor (`var(--term-bg)`)                   |
| `.inspector` (+ 5 children)  | Slide-over panel (420px, right-anchored)                    |
| `.toast` + `.toast.show`     | Bottom-anchored notification (340px max-width)              |
| `.health-dots` + `.dot`      | Inline-cell semantic dots (`.ok`, `.warn`, `.err`)          |
| `.nav-section`               | Parent wrapper for the existing `.nav-section-label`        |

## Deferred to dependent tickets

- **Dioxus `#[component]` wrappers** (`<Panel>`, `<Button>`, `<Toast>`,
  `<Inspector>`, `<LogPanel>`, `<AppCard>`) — land in the consuming
  view ticket alongside its first live use.
- **ArgoCD-specific primitives** (`.app-card`, `.card-status`,
  `.source-icon`, `.tag`, `.card-meta`, `.card-swipe-actions`) — OKT-47
  (ArgoCD JS plugin), the first consumer.
- **Mobile bottom-nav, pull-to-refresh, card swipe** — consumer view
  ticket.
- **Icon sprite** (32 inline `<symbol>` SVGs from the mockup) — OKT-47
  alongside the first consumer.

## Verification

`crates/openkite-ui/tests/design.rs` reads `openkite_ui::MAIN_CSS` and asserts
every required custom property and primitive class is present, plus a
"exactly 12 new properties" guard against accidental re-declaration of
an opaline-mapped var.

`crates/openkite-ui/tests/fonts.rs` pins the type layer: every vendored face is
declared exactly once, every declared URL is a file the route serves, the
embedded stylesheet decodes back to the file bytes, and the payload stays
inside its budget. `crates/openkite-web/tests/routes.rs` serves each face
byte-for-byte over HTTP, and `.../tests/ssr.rs` plus the `openkite-desktop`
head tests assert each host's page declares them.

All of it runs on every CI push with no kube/JS dependencies. The browser and
desktop head tests need their host crates, so the full set rides the normal
workspace jobs.
