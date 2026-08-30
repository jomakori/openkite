# OpenKite Design System — Liquid Frost Glass

The design system is the **token + primitive CSS layer** that every Phase-2
view ticket (OKT-42 Settings, OKT-43 CRUD, OKT-47 ArgoCD plugin, future
views) consumes without re-deciding the visual language.

It lives in two places:

- `assets/main.css` — the single existing asset, already injected by
  `src/lib.rs:107` via `include_str!` into the webview's `<head>`.
- `src/design/{mod.rs, tokens.rs}` — typed Rust view of the
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
| Fonts       | `--font-sans`, `--font-mono` (IBM Plex + system stack)                    |
| Elevation   | `--shadow-rest`, `--shadow-hover`, `--shadow-terminal`                    |
| Radii       | `--r-sm` (6px), `--r-md` (8px), `--r-pill` (999px)                        |

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
- **Explicit `@font-face` for IBM Plex** — OKT-42 (Settings UI) or a
  follow-up visual polish ticket.

## Verification

`tests/design.rs` reads `assets/main.css` via `include_str!` and asserts
every required custom property and primitive class is present, plus a
"exactly 12 new properties" guard against accidental re-declaration of
an opaline-mapped var. The test runs on every CI push with no kube/JS
dependencies.
