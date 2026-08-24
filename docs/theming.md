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

## Built-in themes

`Theme::builtins()` returns five:

- GPUI Light / GPUI Dark
- Catppuccin Mocha
- Tokyo Night
- Rosé Pine

Each covers the full `CSS_VARS` contract. `Theme` is an ordered map
(`BTreeMap<String, String>`) with `serde` transparent serialization, so it
round-trips as a flat JSON object.

## Adding a theme

1. Add a `pub fn my_theme() -> Theme` using the `theme!` macro + `with_term` in
   `src/theme.rs`.
2. Register it in `builtins()`.
3. Extend `tests/theme.rs` if there's new behavior worth pinning.

```rust
pub fn my_theme() -> Theme {
    with_term(theme! {
        "--bg-0" => "#1e1e2e",
        "--bg-1" => "#181825",
        // … remaining core vars …
    })
}
```

## Serialization

```rust
theme.to_css_vars()   // "--bg-0: #1e1e2e;\n--bg-1: #181825;\n…"
theme.save(path)      // pretty JSON to ~/.openkite/theme.json
Theme::load(path)     // read back
```

## Zed theme import

`import_zed(json)` accepts a JSON object — flat or nested — and maps Zed keys to
OpenKite variables:

```json
{
  "background": "#1e1e1e",
  "foreground": "#f5f5f0",
  "accent": "#0070f3",
  "terminal": { "ansi": { "black": "#282c34", "white": "#ffffff" } }
}
```

| Zed key | OpenKite var |
|---|---|
| `background` | `--bg-0` |
| `foreground` | `--fg-0` |
| `border` | `--border` |
| `accent` | `--accent` |
| `terminal.ansi.*` | `--term-*` |

Unknown keys and malformed input are rejected with `ThemeError` (a loud error
rather than a silently broken theme).

## Typography

`assets/main.css` sets the IBM Plex Sans (UI) and IBM Plex Mono (code/terminal)
font stacks.
