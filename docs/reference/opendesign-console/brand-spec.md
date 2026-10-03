# OpenKite — Brand Spec

**System:** Liquid frost glass — Zed's tight precision on Apple-style translucent depth, built on a cool light canvas with a dark opaque terminal anchor.

## Tokens

```css
:root {
  --bg: oklch(0.9755 0.0045 258.3);            /* #f5f7fa */
  --surface: oklch(1 0 0 / 0.85);              /* frosted white */
  --surface-solid: oklch(1 0 0 / 0.97);
  --fg: oklch(0.29 0.018 258);                 /* near-ink */
  --muted: oklch(0.56 0.02 262);
  --subtle: oklch(0.70 0.015 262);
  --border: oklch(0.9188 0.0071 268.5);        /* #e2e4e9 */
  --accent: oklch(0.6420 0.1531 257.9);        /* #4d8ce8 */
  --progress: oklch(0.6420 0.1531 257.9);       /* progressing status */
  --on-accent: oklch(0.16 0.02 250);            /* text on bright fills */
  --success: oklch(0.6212 0.1182 149.1);       /* #4d9a5e */
  --warn: oklch(0.6629 0.1331 72.6);           /* #c4841d */
  --danger: oklch(0.6294 0.1776 23.7);         /* #e05252 */
  --violet: oklch(0.6056 0.2189 292.7);        /* #8b5cf6 controllers */
  --argo: oklch(0.7069 0.1554 41.6);           /* #ef7b4d */
  --brand: oklch(0.6002 0.1038 184.7);         /* #0d9488 kite */
  --terminal-bg: oklch(0.2431 0.0082 264.4);   /* #1e2024 */
  --terminal-fg: oklch(0.7621 0.0202 263.0);   /* #abb2bf */
}
```

## Fonts

- UI: `IBM Plex Sans`, weights 400 / 500 / 600 / 700
- Code, logs, resource names: `IBM Plex Mono`, weights 400 / 500 / 600

## Posture

- Radii stay tight: 6px controls, 8px panels.
- Surfaces use translucent fill + `backdrop-filter`; never opaque light cards except the terminal.
- Shadows escalate with depth: resting → hover/active → floating terminal.
- Dense but not cramped: 44px touch targets, 12–14px UI type.
- One blue accent per view; status colors stay semantic.
- Argo CD owns orange; the kite mark owns teal.
