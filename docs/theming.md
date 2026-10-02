# Theming

OpenKite themes via **CSS variables**. Every color the app uses is a `var(--…)`
reference, so switching themes is an instant variable swap — no re-render.

## The variable contract

Declared in `src/theme.rs` (`CSS_VARS`), defaulted in `crates/openkite-ui/assets/main.css`:

| Group | Variables |
|---|---|
| Background | `--bg` `--surface` `--surface-solid` |
| Border | `--border` |
| Foreground | `--fg` `--muted` `--subtle` |
| Accent | `--accent` `--progress` |
| Status | `--success` `--warn` `--danger` `--violet` |
| Brand | `--brand` `--argo` `--on-accent` (design-system additions) |
| Terminal | `--terminal-bg` `--terminal-fg` |
| Log levels | `--log-info` `--log-method` `--log-error` |
| Terminal (xterm-256 base 16) | `--term-black` … `--term-white`, `--term-bright-black` … `--term-bright-white` |

## Theme source: opaline (OKT-30)

Theming is provided by the [opaline](https://crates.io/crates/opaline) token
engine — **39 builtin themes** across 17 families (SilkCircuit, Catppuccin,
GitHub, Monokai Pro, Ayu, Night Owl, Flexoki, Palenight, Dracula, Nord,
Rose Pine, Gruvbox, Solarized, Tokyo Night, Kanagawa, Everforest, One
Dark/Light). The hand-rolled 5 defaults and the Zed importer were replaced by
opaline (it ships those families natively).

- `src/theme_opaline.rs` maps opaline's semantic tokens onto the contract:
  `--bg ← bg.base`, `--accent ← accent.primary`, `--success/--warn/--danger ←
  success/warning/error`, with palette fallbacks; `--term-bright-*` are derived
  by lightening (opaline themes carry no ANSI brights).
- `theme::resolve(name)` loads an opaline theme by kebab id
  (`"catppuccin-mocha"`, `"default"` → SilkCircuit Neon); unknown ids and
  `None` fall back to the default.
- The theme picker (Settings, OKT-42) lists `theme_opaline::list_opaline_themes()`.

## Frost/glass layering

Opaline supplies **colors**; the glass/frost **chrome** lives in the design
system (`crates/openkite-ui/assets/main.css`, OKT-29) on top of the variables: frost cards are
`var(--surface)` at ~85% opacity + `backdrop-filter: blur(40px)`, elevation via
the shadow system. Tokens are the single source of truth — the chrome never
hardcodes colors.

## Adding a theme

1. Contribute a theme TOML upstream to opaline (palette → token → style
   pipeline), or
2. Drop a theme TOML into the user theme dir (opaline `discovery` — enabled
   once OKT-30 follow-up lands) — it appears in the picker automatically.

## Serialization

```rust
theme.to_css_vars()   // "--bg: #1e1e2e;\n--surface: #181825;\n…"
theme.save(path)      // pretty JSON to ~/.openkite/theme.json
Theme::load(path)     // read back
```

## OS decoration theme (OKT-100)

The native window title bar / OS chrome theme is **separate** from the app
theme above. It is controlled by `titleBarTheme = "system" | "light" | "dark"`
in `~/.openkite/config.toml` (`system` = follow the OS) and changed live from
the palette's View actions (`Title Bar Theme: System / Light / Dark`), which
map onto tao's `Option<Theme>` (`None` = follow the OS). Per tao, the runtime
call is per-window on Windows and app-wide on Linux/macOS. Opaline / the CSS
variable contract is untouched; no automatic linkage between the two is
applied.
