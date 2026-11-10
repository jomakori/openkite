# Gates G4 / G5 / G6 (OKT-194)

Three machine-checkable gates over the browser host, run in CI by the
`ui-state-gates` job in `.github/workflows/build-test-staging.yml`.

```bash
OPENKITE_WEB_BIN=target/debug/openkite-web \
OPENKITE_WEB_ROOT=/tmp/web-bundle \
OPENKITE_BROWSER="$(command -v chromium)" \
  e2e/gates/run.sh e2e/artifacts/gates
```

`run.sh` checks its inputs, runs the static half, then drives the browser half
and writes `report.json` into the artifact directory. Non-zero exit means a
gate failed; every failure line names the route, surface or selector.

## G4 — every data surface renders empty, loading and error

A surface that only works with data is a surface that lies. The host's data
surfaces carry `data-state` (`ready`, `empty`, `loading`, `error`), and the
fixture host takes its answer from `?fixture=`:

```text
/workloads?fixture=empty     → the table paints its empty state
/workloads?fixture=loading   → the table paints its loading state
/workloads?fixture=error     → the table paints its error state
```

`?fixture=` is read only where there is no cluster credential: a host with a
client answers from the cluster, never from a query string. The state holds for
the document that asked for it — the client's 10 s poll refetches from the
gateway and returns the page to the host's own answer. The gate walks four
routes × three states, and for each surface asserts the state on the DOM, that a
node carries the same `data-state`, that the state renders text, and that the
stylesheet actually reached it (padding, background or border). Blank or
unstyled states fail.

Surfaces graded: `overview` and `secrets` on `/`, the resource table on
`/workloads`, and the route body on `/cluster` and `/config`.

## G5 — 44px touch floor at 767px or below

At 390 and 767 CSS px wide the gate collects every interactive element
(`a[href]`, `button`, `input`, `select`, `textarea`, `summary`, `[role=button]`,
`[role=tab]`, `[role=menuitem]`, focusable `[tabindex]`), skips the ones that do
not render (display none, hidden, disabled, zero box), and measures
`getBoundingClientRect()`. Anything under 44×44 CSS px fails, named by selector
and label.

This is measured from the DOM, not read off the stylesheet: a `min-height` that
a later rule overrides still fails.

## G6 — tokens only, and reduced motion zeroes transitions

Two halves:

- **Static** (`check-css.mjs`): reads `crates/openkite-ui/assets/main.css` as
  text and fails on any literal hex outside a `--token:` declaration. A colour
  written into a surface rule ignores the theme; a hex in the token block is
  where a colour is *defined*. It also requires a
  `@media (prefers-reduced-motion: reduce)` block that sets
  `transition-duration` and `animation-duration` to zero for
  `*, *::before, *::after`.
- **Runtime** (`gates.mjs`): sets `prefers-reduced-motion: reduce` through
  CDP and asserts no element on any graded route keeps a transition or
  animation duration above 0.01ms. The sheet saying so and the browser agreeing
  are different facts.

## Scope and limits

- The gates grade the **browser host**. The desktop shell mounts the same
  `openkite-ui` components and reads the same stylesheet, but wry exposes no
  DOM protocol to measure against, so G5's runtime measurement is browser-only;
  the desktop side is covered by the shared component tree plus the stylesheet
  rules this gate reads.
- Overlays (palette, switcher) and the plugin routes are out of scope: they are
  not route surfaces and the fixture host does not mount a plugin.
- G4 grades the presence and honesty of the states, not their copy.
