# OKT-29 — Liquid Frost Glass Design System

**Ticket:** [OKT-29 — Design system — Liquid Frost Glass primitives](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/a61bdbd7-2ad3-46b3-b7e8-fa9cad654060)
**Branch:** `feat/okt29-design-system` (from `origin/main`)
**PR title:** `feat(openkite): liquid frost glass design system`
**Workitem UUID (PR body hyperlink):** `a61bdbd7-2ad3-46b3-b7e8-fa9cad654060`
**Status (start):** Ticket just moved to **In Progress**.

## 1. Scope

### What ships in this PR (foundation-first, pure-logic)

A **token + primitive CSS layer** that every other Phase-2 view ticket (OKT-42 Settings, OKT-43 CRUD, OKT-47 ArgoCD plugin, future views) can consume without re-deciding the visual language. The `run()` bootstrap in `src/lib.rs:107` already injects `assets/main.css` via `include_str!` into the webview's `<head>`; the design system extends that one file in place.

Concretely:

- The **token contract** that the design system *adds on top of* the existing opaline-mapped `--bg-*/--fg-*/--accent/--green/--yellow/--red/--violet/--border/--term-*` vars from `src/theme.rs:18-47` — every new token here either references an existing opaline-mapped var, or is a brand-spec constant (teal/Argo-orange, shadows, radii, fonts, blur) that opaline does not carry.
- The **frost/glass primitive CSS** that translates those tokens into reusable surface treatments (`backdrop-filter: blur(40px)` + `color-mix` translucency, mirroring the `theme-row` recipe at `assets/main.css:94-97`).
- A small **Rust token registry** (`src/design/mod.rs`) so the Dioxus side can reference token names by string (e.g. `design::tokens::BLUR_FROST` → `"40px"`) without scattering raw `40px` literals through the codebase. No Dioxus component wrappers in this PR — those land in the view ticket that actually consumes them.
- A **CSS-contract integration test** that asserts the shipped CSS contains every required class and every required custom property, so a regression in either layer fails CI.

### What is explicitly deferred (consumed by dependent tickets)

- **Interactive Dioxus component wrappers** for each primitive (`<Panel>`, `<Button variant="primary">`, `<Toast>`, `<Inspector>`, `<LogPanel>`, `<AppCard>`): the OKT-29 design system ships CSS classes; the consuming view ticket wraps the class with a `#[component]`. This matches the foundation-first pattern (skill §Foundation-first workflow) — shell-rs shipped pure logic in OKT-31 #43, view wrappers landed in #45.
- **Mobile / tablet / `@media (max-width:1024px)` and `(max-width:767px)` rules**: shipped in this PR as a single, well-organised `/* responsive */` block at the bottom of `main.css` (mirrors mockup lines 858–963), but no responsive JS behaviours — those depend on view-level touch handling (card swipe, pull-to-refresh) and belong to the consuming view ticket.
- **Argo-CD-specific primitives** (`.app-card`, `.card-status`, `.card-swipe-actions`, `.source-icon` helm/kustomize, `.tag`, `.card-meta`): deferred to OKT-47 (ArgoCD JS plugin), which is their first consumer. The CSS *base* (`.app-card` surface, `--argo` token) can ship here as a neutral base; the colored card-status stripes, swipe affordance, source-icon glyphs are plugin content.
- **Icon sprite** (`#i-kite #i-cluster #i-pods ...` from mockup `openkite-console.html`/`:0` per reference notes): the mockup has 32 inline `<symbol>` SVG icons. The plugin API and shell already support plugin-supplied icons (`openkite_plugin_sdk::SidebarEntry::icon` — see `tests/shell.rs:14-18`); a single shared `openkite-icon-sprite` CSS module is out of scope for OKT-29 and can land in OKT-47 alongside the first consumer.
- **Font file shipping** (IBM Plex Sans/Mono `@font-face` rules): the mockup CSS declares `--font-sans: "IBM Plex Sans", -apple-system, ...` and `--font-mono: "IBM Plex Mono", ui-monospace, ...` (`openkite-console.css:23-24`); the existing `assets/main.css:38` already uses the same `IBM Plex Sans` stack. Defer explicit `@font-face` + font file asset to OKT-42 (Settings UI) or a follow-up visual polish ticket — the system stack is good enough for the first phase-2 view.

## 2. File structure

The design system lives in **two places**: CSS in `assets/main.css` (the single existing asset) and a small Rust token registry in `src/design/mod.rs`. The CSS-only path is the foundation; the Rust module is a name-and-string index so non-class contexts (the `style:` attribute on a Dioxus element, an inline `background:` in `router.rs`) can use a typed constant instead of a raw literal.

### Layout

```
assets/
└── main.css                        # EXTEND in place. Add :root tokens + primitives + responsive.
                                    # No new files in assets/. The 142-line file becomes ~360 lines.

src/
├── lib.rs                          # No change — `include_str!("../assets/main.css")` at L107 already
                                    # injects the whole file into the webview head.
├── design/
│   ├── mod.rs                      # NEW. `pub mod tokens;` + module-level docs.
│   │                               # Re-exports `tokens::CSS_VARS` (Rust-side view of the contract).
│   └── tokens.rs                   # NEW. `pub const` strings for every token NAME used in CSS,
│                                   # plus typed string constants for the primitive blur/radii/shadow
│                                   # literals that Dioxus code might pass via `style:`.
└── components/
    └── mod.rs                      # No change. The view-level component wrappers (Button/Panel/...)
                                    # land in dependent tickets; this PR does NOT add new components.

tests/
├── design.rs                       # NEW. Contract test: loads main.css, asserts every required
│                                   # custom property and primitive class is present. Catches
│                                   # "I forgot to add the rule" regressions.
└── ...                             # Unchanged. No sibling-module imports (per skill §Gotchas:
                                    # "Foundation tests must NOT import modules from sibling
                                    # (un-merged) PR branches"). Test reads the CSS file directly
                                    # with `include_str!` so it works even when `lib.rs` doesn't
                                    # re-export the new module until the PR lands.
```

### Why a new `src/design/` module (not a `src/design_system.rs` flat file)

- The skill (§Foundation-first workflow) recommends **alphabetical `pub mod` placement in `lib.rs`** so concurrent PRs don't collide. The current `lib.rs:3-28` already lists 24 modules; adding `pub mod design;` between `crud` and `fuzzy` (lines 8–9) is a 1-line diff that no other in-flight foundation PR touches.
- A submodule `design::tokens` keeps the CSS/Rust split explicit: `design::tokens::BLUR_FROST` reads as a design-system constant; a flat `pub const BLUR_FROST: &str = "40px";` at crate root would not.
- No Dioxus code lives in `src/design/` (per skill §Dioxus 0.7 patterns: "Split pure logic (no dioxus import) from the Dioxus view (glob import) into separate modules"). This keeps the foundation free of the `rsx! let-binding` / `dioxus::prelude::*` glob hazards.

### `lib.rs` change (1 line, alphabetical position)

```rust
pub mod crud;
pub mod design;          // <-- new, between `crud` and `fuzzy`
pub mod fuzzy;
```

No other `lib.rs` line is touched.

## 3. Token inventory

### Already shipped (opaline-mapped, do NOT redeclare)

From `src/theme_opaline.rs:19-41` (the MAPPING table) and `src/theme.rs:18-47` (CSS_VARS): `--bg-0`, `--bg-1`, `--bg-2`, `--border`, `--fg-0`, `--fg-1`, `--fg-2`, `--accent`, `--green`, `--yellow`, `--red`, `--violet`, `--term-*` (all 16). These are **in scope but not redefined** — the design system references them.

### New tokens (design-system adds)

From `brand-spec.md:7-28` and `openkite-console.css:1-30`. Each new var either names a brand-spec constant, or is a `color-mix` recipe over an existing opaline var (so theme switches flow through automatically).

```css
:root {
  /* Brand-specific accents that opaline does not carry.
     See brand-spec.md L23-24. --brand is the kite teal mark;
     --argo is the per-plugin accent zone for the ArgoCD section. */
  --brand:   #0d9488;   /* kite mark teal */
  --argo:    #ef7b4d;   /* Argo CD orange */
  --on-accent: #1c2530; /* text on bright fills (slightly lighter than --fg-0 for hover/primary text) */

  /* Font stacks (mockup L23-24). IBM Plex Sans/Mono is the brand,
     the system stack is the fallback. */
  --font-sans: "IBM Plex Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace;

  /* Elevation (mockup L25-27). Three tiers — rest/hover/terminal — used by
     .panel, .btn, .log-panel, .inspector. rgba(0,0,0,*) is the only
     absolute color allowed in the design system; everything else routes
     through theme vars. */
  --shadow-rest:     0 1px 3px rgba(0,0,0,0.04), 0 4px 12px rgba(0,0,0,0.06);
  --shadow-hover:    0 4px 16px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.06);
  --shadow-terminal: 0 8px 32px rgba(0,0,0,0.12);

  /* Radii (mockup L28-29 + brand-spec.md L37 "6px controls, 8px panels"). */
  --r-sm: 6px;   /* buttons, chips, search fields, log-line meta */
  --r-md: 8px;   /* panels, table-wrap, cards, inspector, toast */
  --r-pill: 999px; /* chips, pills, namespace toggles */
}
```

**No color is duplicated**: the only new hex values are `--brand` (teal), `--argo` (orange), `--on-accent`. All other primitives reference existing opaline vars through `color-mix(in srgb, var(--bg-1) 85%, transparent)` (the recipe that the existing `theme-row` rule at `assets/main.css:94-97` and `.status` at `assets/main.css:113-116` already use).

### Spacing scale (derived, no new vars)

Spacing is consumed as raw px in the existing shell (`padding: 16px`, `padding: 12px`, `gap: 6px`, `gap: 10px`). The design system **adds no spacing vars** — instead, the new primitives document their padding/gap choice in the class comment so the visual rhythm stays consistent:

| Use | Value | Where |
|---|---|---|
| Touch target minimum | 44px | every `button`, `input`, `.nav-item` (mockup L51, L154, L301, L342) |
| Page padding | 20px (desktop), 16px (tablet), 12px (mobile) | `.view`, `.topbar` (mockup L222, L892) |
| Stack gap (small) | 6px | `.ns-chips` (existing `assets/main.css:134`), `.badges` (mockup L685) |
| Stack gap (medium) | 8–10px | `.panel`, `.inspector-header` (mockup L502) |
| Stack gap (large) | 14–16px | `.page-head` ↔ `.toolbar` (mockup L274, L331) |
| Frost blur (panels) | 18px | `.panel` (mockup L390) |
| Frost blur (topbar/sidebar) | 20–24px | `.topbar`, `.sidebar` (mockup L226, L89) |
| Frost blur (theme cards / status) | 40px | `.theme-row`, `.status` (existing) — establishes the "frost" name |

The 40px value is the **branded "frost" number** (brand-spec.md "translucent fill + backdrop-filter everywhere except terminal"); smaller blur (18–24px) is the **content-surface variant** (panels and sidebars that sit *over* the frosted topbar).

## 4. Primitive components

Class names taken directly from the mockup HTML/CSS (`openkite-console.css` line refs) and the existing `assets/main.css`. **The 12 below ship in this PR**; the remaining classes from the mockup that are pure-plugin content (`.app-card`, `.card-status`, `.source-icon`, `.tag`, `.card-meta`, `.card-swipe-actions`, `.bottom-nav`, `.bottom-tab`, `.sidebar-backdrop`, `.pull-indicator`, `.spinner`) are **deferred to OKT-47** (ArgoCD plugin) or the consuming view ticket.

| # | Class | Mockup line | Dioxus wrapper in this PR? | Notes |
|---|---|---|---|---|
| 1 | `.panel` | 385–393 | No | Frosted surface: `background: var(--bg-1)`, `backdrop-filter: blur(18px)`, `box-shadow: var(--shadow-rest)`, `border: 1px solid var(--border)`, `border-radius: var(--r-md)`. The workhorse container. |
| 2 | `.btn` / `.btn-primary` / `.btn-secondary` | 300–326 | No | 44px min-height, `--r-sm`, primary = `background: var(--accent); color: white`, secondary = `background: var(--bg-1); border: 1px solid var(--border)`. :hover escalates to `--shadow-hover`, :active does `translateY(1px)`. |
| 3 | `.chip` / `.chip.active` | 341–357 | No | 44px min-height, `--r-pill`. Active state inverts to `var(--fg-0) bg + white text` (the brand-spec "one blue accent per view" → secondary surfaces use near-ink inversion). |
| 4 | `.search-field` (+ child `input`) | 358–384 | No | 44px min-height, 12px left padding for the inline icon. `::placeholder` uses `var(--fg-2)`. |
| 5 | `.pill` + `.success`/`.warn`/`.danger`/`.muted` | 453–469 | No | 10.5px font, `--r-pill`, semantic color via `color-mix(in srgb, var(--green|--yellow|--red) NN%, white)` (11% bg, 24% border, 62% text) — pattern from mockup L467–469. **Note**: the existing `components/status_badge.rs:55-60` renders `<span class="status-badge {class}">` with a different class name. The contract test asserts both class name sets ship. (Renaming `status-badge` → `pill` is out of scope — that's a consumer refactor.) |
| 6 | `.table-wrap` | 394 | No | `overflow-x: auto` — horizontal scroll wrapper for any tabular content (mockup L394). Distinct from the existing `components/resource_table.rs` virtualization divs. |
| 7 | `.resource-name` (+ child `.icon`) | 428–435 | No | The "icon + name" cell used by every resource row (mockup L428). 14px icon, `--font-mono` text. |
| 8 | `.log-panel` + child `.log-header`, `.log-handle`, `.log-body`, `.log-line`, `.log-time`, `.log-level` + `.warn`/`.error`, `.log-method`, `.log-msg` | 491–570 | No | **The terminal anchor surface** — the only opaque surface in the design system (per brand-spec.md L38 "never opaque light cards except the terminal"). Background = `var(--term-bg)`, foreground = `var(--term-fg)`, font = `--font-mono`. 268px max-height, scrollable. |
| 9 | `.inspector` + `.inspector-header` + `.inspector-body` + `.kv-list` + `.kv-row dt`/`dd` + `.inspector-actions` | 746–807 | No | Slide-over pattern: `position: fixed; right: 0; transform: translateX(100%); .open → translateX(0)`. `width: min(420px, 100%)`. `.kv-row` is a `grid-template-columns: 130px 1fr` two-column key-value table. |
| 10 | `.toast` | 837–855 | No | Bottom-anchored notification: `position: fixed; bottom: 80px; left: 50%`, `var(--fg-0) bg + white text`, 340px max-width, `.show` toggles opacity. The mockup JS shows a toast for 2.2s (mockup JS L22). |
| 11 | `.health-dots` + `.dot` (`.ok`/`.warn`/`.err`) | 437–447 | No | 8×8px semantic dots used in the pod-table Health column. The status_bar_model already routes plugin-supplied colors through `shell::status_dot_color` (test in `router.rs:457-475`) — the design-system `.dot` is the CSS-side equivalent for inline-cell dots. |
| 12 | `.nav-section` + `.nav-section-label` (rename of existing `.nav-section-label` in `assets/main.css:79-85`) | mockup L143–151 | No | Wrapper class for a labelled section. The existing rule ships as `.nav-section-label` (singular). Mockup uses the parent `.nav-section` wrapper. **Add the parent**; leave the existing child class untouched to avoid breaking the current sidebar (`router.rs:301`). |

### What is NOT in this PR (and why)

- `.app-card` / `.card-status` / `.source-icon` / `.card-swipe-actions` / `.tag` / `.card-meta` / `.card-menu-btn` / `.card-footer` / `.badges` / `.card-title-row` / `.app-name` / `.app-sub` / `.app-grid` — all **ArgoCD-specific** (mockup L571–694). They ship in OKT-47 (ArgoCD JS plugin) so the design system stays a plugin-agnostic surface vocabulary.
- `.bottom-nav` + `.bottom-tab` — mobile bottom tabs. Not relevant to the desktop-first Phase-2 build; revisit when Phase-3 web/Mobile lands.
- `.sidebar-backdrop`, `.pull-indicator`, `.spinner` — single-use interaction components; land in the view ticket that needs them.
- `.breadcrumbs`, `.cluster-btn`, `.avatar`, `.icon-btn` — mockup uses these in the topbar/sidebar/cluster button. The current shell implements a simpler `TopBar` (`router.rs:241-278`). A redesign of the topbar is OKT-31's interactive scope, not OKT-29's.

## 5. Implementation steps (PR-friendly chunks)

Each step ends at a green `cargo fmt --all -- --check` + clean working tree, so pushes are never in a half-parsed state. CI gates `fmt`, `clippy --all-targets -- -D warnings`, `test`, `build` (workflow `lint-test.yml:27-88`).

### Chunk A — Token contract only (smallest possible first push)

1. Create `src/design/{mod.rs, tokens.rs}` with the `BLUR_FROST`, `BLUR_PANEL`, `BLUR_TOPBAR`, `R_SM`, `R_MD`, `R_PILL`, `SHADOW_REST`, `SHADOW_HOVER`, `SHADOW_TERMINAL` Rust constants (string values from §3). `pub mod design;` in `lib.rs` (alphabetical: between `crud` line 7 and `fuzzy` line 8).
2. Append the new `:root` block (`--brand`, `--argo`, `--on-accent`, `--font-sans`, `--font-mono`, `--shadow-*`, `--r-*`, `--r-pill`) to `assets/main.css` after the existing `:root` block (current file ends at line 33).
3. **Push.** CI = fmt + clippy (clean: no Dioxus code, no `rsx!` hazards).

### Chunk B — Frost surface recipe + `.panel`

4. Add a comment-delimited section header in `assets/main.css`: `/* Frost primitives (OKT-29) — surfaces built on opaline tokens. */`.
5. Add `.panel` rule (translucent `--bg-1` + `backdrop-filter: blur(18px)` + `--shadow-rest` + `var(--r-md)`).
6. **Push.** Same gate.

### Chunk C — Buttons + chips + search field + pills

7. Add `.btn`, `.btn-primary`, `.btn-secondary`, `.chip`, `.chip.active`, `.search-field` (with inner `input` + `::placeholder`), `.pill` + 3 semantic variants, `.health-dots` + `.dot` (+ `.ok`/`.warn`/`.err`).
8. **Push.**

### Chunk D — Table primitives

9. Add `.table-wrap`, `.resource-name` (with `.icon` child), `.namespace`, `.restarts` + `.restarts.warn`, `.controller`, `.qos`.
10. **Push.**

### Chunk E — Log panel (terminal anchor)

11. Add `.log-panel` (opaque `--term-bg`), `.log-header`, `.log-handle`, `.log-body`, `.log-line`, `.log-time`, `.log-level` + `.warn`/`.error`, `.log-method`, `.log-msg`, `.log-paused`. Inside this block, reference `var(--term-*)` and `var(--term-bright-*)` from the opaline mapping (`src/theme_opaline.rs:33-40`, the terminal entries).
12. **Push.**

### Chunk F — Inspector (slide-over)

13. Add `.inspector` (`.open` toggles slide), `.inspector-header`, `.inspector-body`, `.inspector-eyebrow`, `.kv-list`, `.kv-row dt`/`.dd`, `.inspector-actions`.
14. **Push.**

### Chunk G — Toast + nav-section wrapper

15. Add `.toast` + `.toast.show` (mockup L837-855), and `.nav-section` (the parent class for the existing `.nav-section-label` from `assets/main.css:79-85`).
16. **Push.**

### Chunk H — CSS contract test

17. Create `tests/design.rs` with a test that:
    - `include_str!("../assets/main.css")`-loads the file.
    - Asserts every required custom property is declared (`--brand`, `--argo`, `--on-accent`, `--font-sans`, `--font-mono`, `--shadow-rest`, `--shadow-hover`, `--shadow-terminal`, `--r-sm`, `--r-md`, `--r-pill`).
    - Asserts every shipped class is present (`.panel`, `.btn`, `.btn-primary`, `.btn-secondary`, `.chip`, `.search-field`, `.pill`, `.pill.success`, `.pill.warn`, `.pill.danger`, `.table-wrap`, `.resource-name`, `.log-panel`, `.log-line`, `.inspector`, `.kv-list`, `.toast`, `.nav-section`, `.dot`, `.dot.ok`, `.dot.warn`, `.dot.err`).
    - Asserts the file declares exactly the 12 new `--*` properties (catches accidental duplication of an existing opaline var name).
18. Add a Rust-side test that asserts `src/design/tokens.rs` exports the same blur/radii/shadow strings as the CSS file — guards against drift (e.g. someone updates one but not the other).
19. **Push.** CI now has the contract.

### Chunk I — Responsive block + doc

20. Append the `@media (max-width:1024px) and (min-width:768px)` and `@media (max-width:767px)` blocks at the bottom of `main.css` (mirrors mockup L858-963), scoped to the **already-shipped** primitives only (no `.app-card`, `.bottom-nav` — those are deferred). Add `@media (prefers-reduced-motion: reduce)` to honour the OS-level accessibility setting (mockup L967-968).
21. Add a `docs/design-system.md` (2-screen markdown) summarising the token contract, the primitive list, and the brand posture. Matches the `docs/` pattern (`docs/theming.md` exists at 2,490 B).
22. **Push.** Final CI run.

## 6. Test plan

### Unit / integration tests (no kube, no Dioxus, no JS)

- `tests/design.rs` (new, ~80 lines) — CSS contract test as described in §5 chunk H. Reads the file via `include_str!` so it works from a fresh CI clone.
- `src/design/tokens.rs` `#[cfg(test)] mod tests` — pin each constant to its expected string. Catches accidental literal drift between Rust and CSS.

### What is NOT in unit tests (and where it lives instead)

- **Visual smoke**: CI does not run a browser. The first visual verification happens on a dev machine via `cargo run` (the asset handler in `src/router.rs:81-86` serves `main.css` on the webview's first paint). The PR description includes a "manual smoke" checklist for the reviewer to click through.
- **Dioxus component rendering**: out of scope for this PR (no `#[component]` ships). The first component wrappers render in the consuming view ticket.
- **k8s / kube interactions**: none — the design system is pure CSS + a constant table. Mirrors the foundation-first pattern (skill §Foundation-first workflow).

### What the contract test actually catches

- Missing class rule (someone deletes `.panel` from `main.css`).
- Token renamed in CSS but not in the Rust constant table, or vice versa.
- Accidental re-declaration of an opaline-mapped variable (the "exactly 12 new properties" check).
- File accidentally broken by a partial push (the `include_str!` will fail at compile time).

## 7. Acceptance criteria (mapped from ticket)

| # | Criterion (from ticket) | How met |
|---|---|---|
| 1 | Frost glass surfaces use `backdrop-filter: blur(18-40px)` + translucent fill | `.panel` (18px), `.log-panel` (opaque — explicit exception), topbar/sidebar (20-24px via existing rules) |
| 2 | One blue accent per view | `.btn-primary` uses `var(--accent)`, nothing else writes blue |
| 3 | 6px controls, 8px panels | `--r-sm: 6px` (buttons/chips/search), `--r-md: 8px` (panel/table-wrap/inspector) |
| 4 | 44px touch targets | `.btn`/`button`, `.chip`, `.search-field input`, `.nav-item` all `min-height: 44px` |
| 5 | Translucent everywhere except terminal | `.panel` is translucent, `.log-panel` is opaque (`var(--term-bg)`); brand-spec.md posture honoured |
| 6 | Opaline tokens not redefined | New `:root` block adds 12 vars; existing `--bg-*/--fg-*/--accent/--green/--yellow/--red/--violet/--border/--term-*` are untouched |
| 7 | Argo owns orange, kite mark owns teal | `--argo: #ef7b4d`, `--brand: #0d9488`; only `.nav-section.argo` (deferred to OKT-47) and brand mark (deferred) consume them |
| 8 | IBM Plex Sans/Mono stacks | `--font-sans` / `--font-mono` declared with system fallbacks; existing `body` font-rule at `assets/main.css:38` updated to `var(--font-sans)` |
| 9 | 13px base, 12.5–14px UI type | Documented in `docs/design-system.md`; the new primitives use `font-size: 12px` (chips, search, btn, log) and `12.5px` (nav-item per mockup L158) |
| 10 | Consumable by every other P2 ticket | `assets/main.css` already injected by `src/lib.rs:107`; no consumer change needed |

## 8. Risks and gotchas

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Dioxus rsx! let-binding gotcha** (skill §Dioxus 0.7) | Low | This PR ships **no `rsx!` macros** — the only Rust added is constants in `src/design/tokens.rs`. Component wrappers land in the consuming view ticket. |
| **Drift between CSS and Rust token table** | Low | The contract test in §6 asserts both match. |
| **Existing sidebar/topbar regressions** | Medium | The new `--font-sans` and `--font-mono` vars share names with the existing `font-family` declarations in `assets/main.css:38, 132, 136, 235`. Replace inline strings with `var(--font-sans)` / `var(--font-mono)` so the new vars actually take effect. The existing `.status` and `.topbar` already use the same `color-mix` recipe (`assets/main.css:113-116, 126-127`) — they integrate naturally. |
| **Backdrop-filter cost on low-end GPUs** | Low | Brand-spec.md posture is unconditional. `prefers-reduced-motion` media query lands in chunk I as a hygiene measure. If a future ticket finds a perf issue, the `backdrop-filter` line is a single CSS property to drop. |
| **Opaline theme with no `text.primary` token (e.g. a future user theme)** | Low | Already handled — `src/theme_opaline.rs:45-54` has a 3-tier fallback (token → palette → neutral default). New tokens (`--brand`, `--argo`, `--on-accent`) are CSS constants, not theme-derived, so they're immune. |
| **Accidental re-declaration of an opaline var in the new `:root` block** | Low | The contract test's "exactly 12 new properties" check pinpoints this. |
| **CI flake from `cargo build --workspace` pulling webkit-gtk** | Low | Unchanged from prior PRs. The lint workflow already installs the system deps (`lint-test.yml:47`). |
| **clippy `unnecessary_sort_by` style warnings** | None | No `.sort_by` in this PR. The clippy gate is on `cargo clippy --workspace --all-targets -- -D warnings` (`lint-test.yml:50`). |
| **Auth-gated opendesign mockup** | None | Local grab at `/opt/data/opendesign-grabs/openkite/` (per skill §Project phases). No network call. |
| **No new Cargo deps** | None | Confirmed `Cargo.toml:18-41`. The design system needs only `serde` (already a dep) for any future structured-constant table; this PR doesn't even need that. |
| **Scaffolding with no consumer (`dead_code`)** | Medium | `src/design/tokens.rs` has consumers in the form of contract tests but no live `style:` consumer yet. Mark the module with `#![allow(dead_code)]` at the top (pattern from `components/theme_selector.rs:9` and `components/resource_table.rs:9`). Strip the allow once a consuming ticket references the constants. |

## 9. PR description (template at `.github/pull_request_template.md`)

The PR body draft lives at `.hermes/plans/PR-description-draft.md` next to this plan, pre-filled per the template (L1–37 of `.github/pull_request_template.md`). Sections used: `## 📚 Description` (L1) → `### <topic>` bullets (L5, L9) → `### Relevant Plane Tickets:` (L13, simplified to the singular `Relevant Plane Ticket:` per skill rule) → `## 🔍 Types of Changes` with all 6 checkboxes (L17–26, only "Feature" ticked) → `## ✅ Checklist` with all 6 items (L28–37, all 6 ticked). Plane hyperlink uses the verified workitem UUID `a61bdbd7-2ad3-46b3-b7e8-fa9cad654060` as a markdown URL.

## 10. OpenKite skill rules to follow

Pulled from the `openkite-dev` skill — checked against this plan:

- **One ticket at a time.** This plan covers only OKT-29. No cross-ticket scope creep.
- **Branch from `origin/main`, not from `feat/okt31-shell-complete`.** OKT-31 already merged (PRs #43, #45). `git switch -c feat/okt29-design-system origin/main`.
- **Conventional commit `feat(openkite): liquid frost glass design system (OKT-29)`.** The `(OKT-N)` suffix is in the commit trailer only — NOT in code comments, per the 2026-08-30 user directive (skill §Code conventions, third bullet).
- **No `OKT-N` in source comments.** Module doc-comments for `src/design/mod.rs` and `src/design/tokens.rs` describe what the design system *is*, not which ticket built it. The brand spec and mockup URLs (if cited) carry the provenance, not a ticket id.
- **Plane ticket hyperlink uses markdown URL, not bare text.** `[OKT-29 — Design system — Liquid Frost Glass primitives](https://plane.maklab.net/maklab/projects/71ba0e95-7c1a-4ea6-a50a-c42b0591492f/issues/a61bdbd7-2ad3-46b3-b7e8-fa9cad654060)`.
- **PR body follows `.github/pull_request_template.md`.** All 6 type checkboxes and all 6 checklist items present, even if only "Feature" is ticked and the 6th item (SDK version bump) is N/A — checked to indicate "no SDK change". (Rule: missing checkboxes look unfinished.)
- **Doc-comments on every `pub` item** in `src/design/tokens.rs` — except trivial `pub const X: &str = "...";` one-liners (per skill §Code conventions). The module-level doc on `src/design/mod.rs` is the entry point.
- **CI-as-compiler loop** (skill §CI-as-compiler loop): `cargo fmt --all -- --check` locally before every push. CI is the build/test gate. No link-stage builds locally (no `glib-2.0`).
- **Comments-as-context, not change-history.** `src/design/tokens.rs` and `docs/design-system.md` describe what each token is for in the *current* codebase, not when it was added or by whom.
- **Foundation tests don't import sibling un-merged modules.** `tests/design.rs` reads `assets/main.css` via `include_str!` and asserts on raw strings — no `use openkite::design;`. The Rust constant table is tested inside `tokens.rs` itself. This keeps the test self-contained for the foundation PR.

## Branch + commit plan

```bash
# Pre-flight (from current branch feat/okt31-shell-complete)
git switch main
git pull --ff-only origin main
git switch -c feat/okt29-design-system

# After each chunk's push (A → I), CI gates per the loop in skill §CI-as-compiler
cargo fmt --all -- --check
git add -A
git commit -m "feat(openkite): liquid frost glass design system (OKT-29)" -m "..."
git push -u origin feat/okt29-design-system
gh pr create --base main --title "feat(openkite): liquid frost glass design system" \
             --body-file .hermes/plans/PR-description-draft.md
gh pr checks <N> --watch --interval 15
# On green: gh pr merge <N> --squash --delete-branch
# Move ticket In Progress → Done, post the PR URL as a comment.
```

Per the foundation-first pattern (skill §Foundation-first workflow), this PR can merge **before** its consumers (the Phase-2 view tickets) are ready. The asset handler in `src/router.rs:81-86` already serves `main.css`; the new tokens and classes are pure additions that don't change the existing shell render. The remaining ticket for OKT-29 (move to Done) is gated on the PR being merged.
