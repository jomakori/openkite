# Theming

OpenKite themes via **CSS variables**. Every color the app uses is a `var(--…)`
reference, so switching themes is an instant variable swap — no re-render.

## The variable contract

Declared in `src/theme.rs` (`CSS_VARS`), defaulted in `assets/main.css`:

| Group | Variables |
|---|---|
| Background | `--bg-0` `--bg-1` `--bg-2` |
| Border | `--border` |
| Foreground | `--fg-0` `--fg-1` `--fg-2` |
| Accent | `--accent` |
| Status | `--green` `--yellow` `--red` `--violet` |
| Terminal (xterm-256 base 16) | `--term-black` … `--term-white`, `--term-bright-black` … `--term-bright-white` |

## Theme source: opaline (OKT-30)

Theming is provided by the [opaline](https://crates.io/crates/opaline) token
engine — **39 builtin themes** across 17 families (SilkCircuit, Catppuccin,
GitHub, Monokai Pro, Ayu, Night Owl, Flexoki, Palenight, Dracula, Nord,
Rose Pine, Gruvbox, Solarized, Tokyo Night, Kanagawa, Everforest, One
Dark/Light). The hand-rolled 5 defaults and the Zed importer were replaced by
opaline (it ships those families natively).

- `src/theme_opaline.rs` maps opaline's semantic tokens onto the contract:
  `--bg-0 ← bg.base`, `--accent ← accent.primary`, `--green/--yellow/--red ←
  success/warning/error`, with palette fallbacks; `--term-bright-*` are derived
  by lightening (opaline themes carry no ANSI brights).
- `theme::resolve(name)` loads an opaline theme by kebab id
  (`"catppuccin-mocha"`, `"default"` → SilkCircuit Neon); unknown ids and
  `None` fall back to the default.
- The theme picker (Settings, OKT-42) lists `theme_opaline::list_opaline_themes()`.

## Frost/glass layering

Opaline supplies **colors**; the glass/frost **chrome** lives in the design
system (`assets/main.css`, OKT-29) on top of the variables: frost cards are
`var(--bg-1)` at ~85% opacity + `backdrop-filter: blur(40px)`, elevation via
the shadow system. Tokens are the single source of truth — the chrome never
hardcodes colors.

## Adding a theme

1. Contribute a theme TOML upstream to opaline (palette → token → style
   pipeline), or
2. Drop a theme TOML into the user theme dir (opaline `discovery` — enabled
   once OKT-30 follow-up lands) — it appears in the picker automatically.

## Serialization

```rust
theme.to_css_vars()   // "--bg-0: #1e1e2e;\n--bg-1: #181825;\n…"
theme.save(path)      // pretty JSON to ~/.openkite/theme.json
Theme::load(path)     // read back
```
