//! Console settings: themes, density, shortcuts and kubeconfig paths.
//!
//! A theme may only override the token names the console already paints with,
//! so every surface keeps reading `var(--…)` and a switch is a repaint, not a
//! reload.

use serde::Deserialize;

/// Every custom property a theme is allowed to set.
pub const TOKENS: [&str; 12] = [
    "bg",
    "surface",
    "surface-solid",
    "fg",
    "muted",
    "subtle",
    "border",
    "accent",
    "success",
    "warn",
    "danger",
    // A bare `L C H` triple, not a colour: chrome reads it as `oklch(var(--glass)/a)`.
    "glass",
];

/// Whether a theme paints light or dark, so the platform scrollbars match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// Which appearance the console should paint, before the system is consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    System,
    Light,
    Dark,
}

impl Preference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Whether a theme of this appearance belongs to the slot being chosen.
    ///
    /// Under `System` both slots are live, so every preset is pickable; pinning one appearance
    /// narrows the list to the presets that slot can hold.
    pub fn accepts(self, appearance: Appearance) -> bool {
        match self {
            Self::System => true,
            Self::Light => appearance == Appearance::Light,
            Self::Dark => appearance == Appearance::Dark,
        }
    }

    /// The appearance to paint, given what the platform reports.
    pub fn resolve(self, system: Appearance) -> Appearance {
        match self {
            Self::System => system,
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
        }
    }
}

/// Row height and padding of dense surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        }
    }
}

/// A set of token overrides, plus the name it shows up as in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    tokens: Vec<(String, String)>,
}

impl Theme {
    pub fn tokens(&self) -> &[(String, String)] {
        &self.tokens
    }

    /// The override block as an inline `style`, for themes that are not compiled in.
    pub fn css_vars(&self) -> String {
        self.tokens
            .iter()
            .map(|(name, value)| format!("--{name}:{value}"))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn builtin(id: &str, name: &str, appearance: Appearance, tokens: &[(&str, &str)]) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            appearance,
            tokens: tokens
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }
}

/// The frost pair the console ships with: the frosted surfaces are built for it.
/// Both appearances are drawn from Apple's system palette — the blue, green,
/// orange and red the OS uses, on the neutral canvases it uses — because the
/// glass chrome only reads as glass when the tint behind it is the system's own.
pub const DEFAULT_LIGHT: &str = "frost";
pub const DEFAULT_DARK: &str = "frost-dark";

/// The presets that ship with the console: twelve dark and twelve light, drawn
/// from the themes people already run in Zed and VS Code. `frost` and
/// `frost-dark` are the defaults and the pair the frosted surfaces are built for.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        // Dark
        Theme::builtin(
            "frost-dark",
            "Frost Dark",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.185 0.004 286)"),
                ("glass", "0.262 0.010 265"),
                ("surface", "oklch(0.262 0.010 265 / 0.72)"),
                ("surface-solid", "oklch(0.262 0.010 265 / 0.96)"),
                ("fg", "oklch(0.971 0.003 286)"),
                ("muted", "oklch(0.768 0.006 268)"),
                ("subtle", "oklch(0.556 0.005 268)"),
                ("border", "oklch(0.352 0.007 268)"),
                ("accent", "oklch(0.624 0.206 256)"),
                ("success", "oklch(0.756 0.208 147)"),
                ("warn", "oklch(0.782 0.171 67)"),
                ("danger", "oklch(0.663 0.224 28)"),
            ],
        ),
        Theme::builtin(
            "catppuccin-mocha",
            "Catppuccin Mocha",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.243 0.030 284)"),
                ("glass", "0.324 0.032 282"),
                ("surface", "oklch(0.324 0.032 282 / 0.86)"),
                ("surface-solid", "oklch(0.324 0.032 282 / 0.97)"),
                ("fg", "oklch(0.879 0.043 272)"),
                ("muted", "oklch(0.713 0.039 275)"),
                ("subtle", "oklch(0.535 0.036 278)"),
                ("border", "oklch(0.383 0.033 281)"),
                ("accent", "oklch(0.766 0.111 260)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "one-dark",
            "One Dark",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.293 0.016 264)"),
                ("glass", "0.368 0.022 267"),
                ("surface", "oklch(0.368 0.022 267 / 0.86)"),
                ("surface-solid", "oklch(0.368 0.022 267 / 0.97)"),
                ("fg", "oklch(0.762 0.020 263)"),
                ("muted", "oklch(0.640 0.019 263)"),
                ("subtle", "oklch(0.509 0.018 264)"),
                ("border", "oklch(0.396 0.017 264)"),
                ("accent", "oklch(0.730 0.121 245)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "dracula",
            "Dracula",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.288 0.022 278)"),
                ("glass", "0.403 0.032 278"),
                ("surface", "oklch(0.403 0.032 278 / 0.86)"),
                ("surface-solid", "oklch(0.403 0.032 278 / 0.97)"),
                ("fg", "oklch(0.977 0.008 107)"),
                ("muted", "oklch(0.798 0.001 185)"),
                ("subtle", "oklch(0.605 0.008 274)"),
                ("border", "oklch(0.440 0.016 276)"),
                ("accent", "oklch(0.742 0.149 302)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "nord-dark",
            "Nord",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.324 0.023 264)"),
                ("glass", "0.379 0.029 266"),
                ("surface", "oklch(0.379 0.029 266 / 0.86)"),
                ("surface-solid", "oklch(0.379 0.029 266 / 0.97)"),
                ("fg", "oklch(0.951 0.007 261)"),
                ("muted", "oklch(0.788 0.011 263)"),
                ("subtle", "oklch(0.613 0.016 263)"),
                ("border", "oklch(0.462 0.020 264)"),
                ("accent", "oklch(0.775 0.062 217)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "gruvbox-dark",
            "Gruvbox Dark",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.277 0.000 90)"),
                ("glass", "0.344 0.007 49"),
                ("surface", "oklch(0.344 0.007 49 / 0.86)"),
                ("surface-solid", "oklch(0.344 0.007 49 / 0.97)"),
                ("fg", "oklch(0.894 0.057 89)"),
                ("muted", "oklch(0.734 0.042 89)"),
                ("subtle", "oklch(0.561 0.026 89)"),
                ("border", "oklch(0.413 0.012 89)"),
                ("accent", "oklch(0.832 0.159 83)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "tokyo-night",
            "Tokyo Night",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.226 0.021 280)"),
                ("glass", "0.306 0.037 273"),
                ("surface", "oklch(0.306 0.037 273 / 0.86)"),
                ("surface-solid", "oklch(0.306 0.037 273 / 0.97)"),
                ("fg", "oklch(0.846 0.061 275)"),
                ("muted", "oklch(0.685 0.051 275)"),
                ("subtle", "oklch(0.511 0.040 276)"),
                ("border", "oklch(0.363 0.030 278)"),
                ("accent", "oklch(0.719 0.132 264)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "solarized-dark",
            "Solarized Dark",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.267 0.049 220)"),
                ("glass", "0.309 0.052 220"),
                ("surface", "oklch(0.309 0.052 220 / 0.86)"),
                ("surface-solid", "oklch(0.309 0.052 220 / 0.97)"),
                ("fg", "oklch(0.698 0.016 197)"),
                ("muted", "oklch(0.586 0.024 209)"),
                ("subtle", "oklch(0.465 0.033 215)"),
                ("border", "oklch(0.362 0.041 218)"),
                ("accent", "oklch(0.615 0.139 245)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "monokai",
            "Monokai",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.274 0.011 115)"),
                ("glass", "0.357 0.018 103"),
                ("surface", "oklch(0.357 0.018 103 / 0.86)"),
                ("surface-solid", "oklch(0.357 0.018 103 / 0.97)"),
                ("fg", "oklch(0.977 0.008 107)"),
                ("muted", "oklch(0.794 0.009 109)"),
                ("subtle", "oklch(0.597 0.009 112)"),
                ("border", "oklch(0.428 0.010 113)"),
                ("accent", "oklch(0.841 0.204 127)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "ayu-dark",
            "Ayu Dark",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.163 0.014 264)"),
                ("glass", "0.205 0.021 268"),
                ("surface", "oklch(0.205 0.021 268 / 0.86)"),
                ("surface-solid", "oklch(0.205 0.021 268 / 0.97)"),
                ("fg", "oklch(0.798 0.010 94)"),
                ("muted", "oklch(0.633 0.004 102)"),
                ("subtle", "oklch(0.455 0.003 249)"),
                ("border", "oklch(0.303 0.008 262)"),
                ("accent", "oklch(0.797 0.130 82)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "kanagawa-wave",
            "Kanagawa Wave",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.243 0.017 285)"),
                ("glass", "0.291 0.024 285"),
                ("surface", "oklch(0.291 0.024 285 / 0.86)"),
                ("surface-solid", "oklch(0.291 0.024 285 / 0.97)"),
                ("fg", "oklch(0.876 0.039 99)"),
                ("muted", "oklch(0.711 0.025 98)"),
                ("subtle", "oklch(0.534 0.009 93)"),
                ("border", "oklch(0.382 0.005 296)"),
                ("accent", "oklch(0.694 0.095 264)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        Theme::builtin(
            "rose-pine",
            "Rosé Pine",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.213 0.025 291)"),
                ("glass", "0.241 0.032 289"),
                ("surface", "oklch(0.241 0.032 289 / 0.86)"),
                ("surface-solid", "oklch(0.241 0.032 289 / 0.97)"),
                ("fg", "oklch(0.909 0.030 290)"),
                ("muted", "oklch(0.728 0.029 290)"),
                ("subtle", "oklch(0.533 0.027 291)"),
                ("border", "oklch(0.366 0.026 291)"),
                ("accent", "oklch(0.776 0.095 305)"),
                ("success", "oklch(0.720 0.130 150)"),
                ("warn", "oklch(0.780 0.130 78)"),
                ("danger", "oklch(0.700 0.170 22)"),
            ],
        ),
        // Light
        Theme::builtin(
            "frost",
            "Frost",
            Appearance::Light,
            &[
                ("bg", "oklch(0.971 0.003 286)"),
                ("glass", "0.996 0.006 265"),
                ("surface", "oklch(1.000 0.000 90 / 0.72)"),
                ("surface-solid", "oklch(1.000 0.000 90 / 0.96)"),
                ("fg", "oklch(0.232 0.004 286)"),
                ("muted", "oklch(0.540 0.008 286)"),
                ("subtle", "oklch(0.680 0.006 286)"),
                ("border", "oklch(0.882 0.004 286)"),
                ("accent", "oklch(0.603 0.218 257)"),
                ("success", "oklch(0.730 0.194 147)"),
                ("warn", "oklch(0.765 0.175 63)"),
                ("danger", "oklch(0.654 0.232 29)"),
            ],
        ),
        Theme::builtin(
            "catppuccin-latte",
            "Catppuccin Latte",
            Appearance::Light,
            &[
                ("bg", "oklch(0.958 0.006 265)"),
                ("glass", "0.933 0.009 265"),
                ("surface", "oklch(0.933 0.009 265 / 0.86)"),
                ("surface-solid", "oklch(0.933 0.009 265 / 0.97)"),
                ("fg", "oklch(0.435 0.043 279)"),
                ("muted", "oklch(0.571 0.033 279)"),
                ("subtle", "oklch(0.723 0.022 277)"),
                ("border", "oklch(0.853 0.013 274)"),
                ("accent", "oklch(0.559 0.226 262)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "one-light",
            "One Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.985 0.000 90)"),
                ("glass", "0.955 0.000 90"),
                ("surface", "oklch(0.955 0.000 90 / 0.86)"),
                ("surface-solid", "oklch(0.955 0.000 90 / 0.97)"),
                ("fg", "oklch(0.350 0.014 275)"),
                ("muted", "oklch(0.515 0.010 275)"),
                ("subtle", "oklch(0.699 0.006 275)"),
                ("border", "oklch(0.858 0.003 275)"),
                ("accent", "oklch(0.602 0.193 263)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "solarized-light",
            "Solarized Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.974 0.026 90)"),
                ("glass", "0.931 0.026 92"),
                ("surface", "oklch(0.931 0.026 92 / 0.86)"),
                ("surface-solid", "oklch(0.931 0.026 92 / 0.97)"),
                ("fg", "oklch(0.523 0.028 219)"),
                ("muted", "oklch(0.640 0.017 202)"),
                ("subtle", "oklch(0.771 0.012 148)"),
                ("border", "oklch(0.883 0.018 104)"),
                ("accent", "oklch(0.615 0.139 245)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "gruvbox-light",
            "Gruvbox Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.956 0.055 96)"),
                ("glass", "0.894 0.057 89"),
                ("surface", "oklch(0.894 0.057 89 / 0.86)"),
                ("surface-solid", "oklch(0.894 0.057 89 / 0.97)"),
                ("fg", "oklch(0.344 0.007 49)"),
                ("muted", "oklch(0.503 0.018 85)"),
                ("subtle", "oklch(0.680 0.033 92)"),
                ("border", "oklch(0.833 0.045 95)"),
                ("accent", "oklch(0.618 0.128 71)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "github-light",
            "GitHub Light",
            Appearance::Light,
            &[
                ("bg", "oklch(1.000 0.000 90)"),
                ("glass", "0.978 0.003 248"),
                ("surface", "oklch(0.978 0.003 248 / 0.86)"),
                ("surface-solid", "oklch(0.978 0.003 248 / 0.97)"),
                ("fg", "oklch(0.254 0.011 254)"),
                ("muted", "oklch(0.448 0.008 254)"),
                ("subtle", "oklch(0.664 0.005 254)"),
                ("border", "oklch(0.851 0.002 254)"),
                ("accent", "oklch(0.540 0.191 257)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "ayu-light",
            "Ayu Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.985 0.000 90)"),
                ("glass", "0.955 0.000 90"),
                ("surface", "oklch(0.955 0.000 90 / 0.86)"),
                ("surface-solid", "oklch(0.955 0.000 90 / 0.97)"),
                ("fg", "oklch(0.490 0.010 248)"),
                ("muted", "oklch(0.619 0.008 248)"),
                ("subtle", "oklch(0.762 0.005 248)"),
                ("border", "oklch(0.886 0.002 248)"),
                ("accent", "oklch(0.775 0.158 58)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "rose-pine-dawn",
            "Rosé Pine Dawn",
            Appearance::Light,
            &[
                ("bg", "oklch(0.970 0.011 72)"),
                ("glass", "0.987 0.011 77"),
                ("surface", "oklch(0.987 0.011 77 / 0.86)"),
                ("surface-solid", "oklch(0.987 0.011 77 / 0.97)"),
                ("fg", "oklch(0.460 0.063 290)"),
                ("muted", "oklch(0.592 0.044 292)"),
                ("subtle", "oklch(0.740 0.024 299)"),
                ("border", "oklch(0.868 0.008 335)"),
                ("accent", "oklch(0.617 0.074 306)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "kanagawa-lotus",
            "Kanagawa Lotus",
            Appearance::Light,
            &[
                ("bg", "oklch(0.936 0.063 102)"),
                ("glass", "0.887 0.077 98"),
                ("surface", "oklch(0.887 0.077 98 / 0.86)"),
                ("surface-solid", "oklch(0.887 0.077 98 / 0.97)"),
                ("fg", "oklch(0.452 0.026 285)"),
                ("muted", "oklch(0.578 0.003 305)"),
                ("subtle", "oklch(0.718 0.023 100)"),
                ("border", "oklch(0.839 0.045 101)"),
                ("accent", "oklch(0.521 0.086 261)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "everforest-light",
            "Everforest Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.974 0.026 90)"),
                ("glass", "0.952 0.031 99"),
                ("surface", "oklch(0.952 0.031 99 / 0.86)"),
                ("surface-solid", "oklch(0.952 0.031 99 / 0.97)"),
                ("fg", "oklch(0.515 0.021 233)"),
                ("muted", "oklch(0.635 0.011 211)"),
                ("subtle", "oklch(0.767 0.009 131)"),
                ("border", "oklch(0.882 0.018 98)"),
                ("accent", "oklch(0.670 0.156 118)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "quiet-light",
            "Quiet Light",
            Appearance::Light,
            &[
                ("bg", "oklch(0.970 0.000 90)"),
                ("glass", "1.000 0.000 90"),
                ("surface", "oklch(1.000 0.000 90 / 0.86)"),
                ("surface-solid", "oklch(1.000 0.000 90 / 0.97)"),
                ("fg", "oklch(0.321 0.000 90)"),
                ("muted", "oklch(0.490 0.000 90)"),
                ("subtle", "oklch(0.678 0.000 90)"),
                ("border", "oklch(0.840 0.000 90)"),
                ("accent", "oklch(0.508 0.104 301)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
        Theme::builtin(
            "xcode-light",
            "Xcode Light",
            Appearance::Light,
            &[
                ("bg", "oklch(1.000 0.000 90)"),
                ("glass", "0.961 0.000 90"),
                ("surface", "oklch(0.961 0.000 90 / 0.86)"),
                ("surface-solid", "oklch(0.961 0.000 90 / 0.97)"),
                ("fg", "oklch(0.269 0.000 90)"),
                ("muted", "oklch(0.459 0.000 90)"),
                ("subtle", "oklch(0.671 0.000 90)"),
                ("border", "oklch(0.854 0.000 90)"),
                ("accent", "oklch(0.603 0.218 257)"),
                ("success", "oklch(0.600 0.110 150)"),
                ("warn", "oklch(0.680 0.130 70)"),
                ("danger", "oklch(0.580 0.170 26)"),
            ],
        ),
    ]
}

pub fn theme_by_id(id: &str) -> Theme {
    let themes = builtin_themes();
    // An unknown id falls back to the default light preset, not to whichever
    // preset happens to sit first in the table.
    let index = themes
        .iter()
        .position(|theme| theme.id == id)
        .or_else(|| themes.iter().position(|theme| theme.id == DEFAULT_LIGHT))
        .unwrap_or(0);
    themes[index].clone()
}

/// The preset a given appearance paints with when nothing else is chosen.
pub fn default_theme(appearance: Appearance) -> Theme {
    theme_by_id(match appearance {
        Appearance::Light => DEFAULT_LIGHT,
        Appearance::Dark => DEFAULT_DARK,
    })
}

/// A Zed theme file, reduced to the tokens the console can honour.
#[derive(Debug, Deserialize)]
struct ZedFile {
    #[serde(default)]
    themes: Vec<ZedTheme>,
}

#[derive(Debug, Deserialize)]
struct ZedTheme {
    name: String,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default)]
    style: ZedStyle,
}

#[derive(Debug, Default, Deserialize)]
struct ZedStyle {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    element: Option<ZedElement>,
    #[serde(default)]
    accents: Vec<ZedAccent>,
    #[serde(default)]
    players: Vec<ZedAccent>,
}

#[derive(Debug, Default, Deserialize)]
struct ZedElement {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ZedAccent {
    #[serde(default)]
    color: Option<String>,
}

/// What went wrong reading a Zed theme file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    /// The file is not JSON, or not shaped like a Zed theme.
    Malformed(String),
    /// The file parsed but carried no theme to import.
    Empty,
    /// The only theme in the file sets no colour we can map.
    NoTokens,
}

impl ThemeError {
    pub fn message(&self) -> String {
        match self {
            Self::Malformed(why) => format!("Could not read that theme: {why}"),
            Self::Empty => "That file carries no themes".to_string(),
            Self::NoTokens => "That theme sets no colours the console can use".to_string(),
        }
    }
}

/// Reads the first theme of a Zed theme file into the tokens the console paints with.
pub fn parse_zed_theme(json: &str) -> Result<Theme, ThemeError> {
    let file: ZedFile =
        serde_json::from_str(json).map_err(|err| ThemeError::Malformed(err.to_string()))?;
    let theme = file.themes.into_iter().next().ok_or(ThemeError::Empty)?;
    let style = theme.style;
    let mut tokens: Vec<(String, String)> = Vec::new();
    let mut push = |name: &str, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            tokens.push((name.to_string(), value));
        }
    };
    push("bg", style.background);
    let element = style.element.unwrap_or_default();
    push(
        "surface",
        style.surface.or_else(|| element.background.clone()),
    );
    push("surface-solid", element.background);
    push("border", style.border.or(element.border));
    push("fg", style.text.or(element.text));
    let accent = style
        .accents
        .first()
        .or(style.players.first())
        .and_then(|accent| accent.color.clone());
    push("accent", accent);
    if tokens.is_empty() {
        return Err(ThemeError::NoTokens);
    }
    // A theme read from a file cannot live in the compiled picker, so its id is
    // derived from its name and it is addressed by the inline block instead.
    let id = format!(
        "zed-{}",
        theme
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );
    Ok(Theme {
        id,
        name: theme.name,
        appearance: match theme.appearance.as_deref() {
            Some("light") => Appearance::Light,
            _ => Appearance::Dark,
        },
        tokens,
    })
}

/// One row of the shortcut sheet.
pub struct Shortcut {
    pub action: &'static str,
    pub keys: &'static str,
}

/// The bindings the console answers to.
pub const SHORTCUTS: [Shortcut; 6] = [
    Shortcut {
        action: "Search and commands",
        keys: "mod+k",
    },
    Shortcut {
        action: "Toggle the log dock",
        keys: "mod+j",
    },
    Shortcut {
        action: "Next namespace",
        keys: "mod+]",
    },
    Shortcut {
        action: "Refresh the view",
        keys: "mod+r",
    },
    Shortcut {
        action: "Close the overlay",
        keys: "esc",
    },
    Shortcut {
        action: "Move through results",
        keys: "↑ ↓",
    },
];

/// The settings the console holds for this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Whether the console follows the system appearance or pins one.
    pub preference: Preference,
    /// The preset painted when the resolved appearance is light.
    pub theme_light: String,
    /// The preset painted when the resolved appearance is dark.
    pub theme_dark: String,
    /// A theme read from a Zed file, if the active theme came from one.
    pub imported: Option<Theme>,
    pub density: Density,
    pub kubeconfig_paths: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preference: Preference::System,
            theme_light: DEFAULT_LIGHT.to_string(),
            theme_dark: DEFAULT_DARK.to_string(),
            imported: None,
            density: Density::Comfortable,
            kubeconfig_paths: vec!["~/.kube/config".to_string()],
        }
    }
}

impl Settings {
    /// The appearance the console paints with, given what the platform reports.
    pub fn appearance(&self, system: Appearance) -> Appearance {
        self.preference.resolve(system)
    }

    /// The preset for an appearance: whichever slot matches it, or the default pair.
    pub fn theme_for(&self, appearance: Appearance) -> Theme {
        let id = match appearance {
            Appearance::Light => &self.theme_light,
            Appearance::Dark => &self.theme_dark,
        };
        match &self.imported {
            Some(theme) if theme.id == *id => theme.clone(),
            _ => theme_by_id(id),
        }
    }

    /// The theme in force, given what the platform reports.
    pub fn theme(&self, system: Appearance) -> Theme {
        self.theme_for(self.appearance(system))
    }

    /// Adds a kubeconfig path, ignoring blanks and duplicates.
    pub fn add_kubeconfig(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() || self.kubeconfig_paths.iter().any(|known| known == path) {
            return;
        }
        self.kubeconfig_paths.push(path.to_string());
    }

    pub fn remove_kubeconfig(&mut self, path: &str) {
        self.kubeconfig_paths.retain(|known| known != path);
    }

    /// Points the slot that matches the theme's own appearance at it, so a dark
    /// preset never lands in the light slot.
    pub fn set_theme(&mut self, id: &str) {
        let theme = theme_by_id(id);
        match theme.appearance {
            Appearance::Light => self.theme_light = theme.id,
            Appearance::Dark => self.theme_dark = theme.id,
        }
    }

    /// Makes an imported theme the active one in the slot its appearance owns.
    pub fn use_theme(&mut self, theme: Theme) {
        match theme.appearance {
            Appearance::Light => self.theme_light = theme.id.clone(),
            Appearance::Dark => self.theme_dark = theme.id.clone(),
        }
        self.imported = Some(theme);
    }

    /// The token block every surface under the theme reads.
    pub fn css_vars(&self, system: Appearance) -> String {
        self.theme(system).css_vars()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_only_sets_known_tokens() {
        for theme in builtin_themes() {
            for (name, _) in theme.tokens() {
                assert!(
                    TOKENS.contains(&name.as_str()),
                    "{} sets unknown token --{name}",
                    theme.id
                );
            }
        }
    }

    #[test]
    fn twelve_themes_ship_per_appearance() {
        let themes = builtin_themes();
        let light = themes
            .iter()
            .filter(|t| t.appearance == Appearance::Light)
            .count();
        let dark = themes
            .iter()
            .filter(|t| t.appearance == Appearance::Dark)
            .count();
        assert_eq!((light, dark), (12, 12), "{themes:?}");
        let mut ids: Vec<_> = themes.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 24, "duplicate preset id");
    }

    #[test]
    fn the_default_is_the_frost_pair_following_the_system() {
        let settings = Settings::default();
        assert_eq!(settings.preference, Preference::System);
        assert_eq!(settings.theme(Appearance::Light).id, "frost");
        assert_eq!(settings.theme(Appearance::Dark).id, "frost-dark");
    }

    #[test]
    fn a_preset_lands_in_the_slot_its_own_appearance_owns() {
        let mut settings = Settings::default();
        settings.set_theme("tokyo-night");
        assert_eq!(settings.theme_dark, "tokyo-night");
        assert_eq!(
            settings.theme_light, "frost",
            "a dark preset must not fill the light slot"
        );
        settings.set_theme("gruvbox-light");
        assert_eq!(settings.theme_light, "gruvbox-light");
        assert_eq!(settings.theme(Appearance::Dark).id, "tokyo-night");
        assert_eq!(settings.theme(Appearance::Light).id, "gruvbox-light");
    }

    #[test]
    fn pinning_an_appearance_ignores_what_the_system_reports() {
        let mut settings = Settings::default();
        settings.set_theme("tokyo-night");
        settings.preference = Preference::Light;
        assert_eq!(settings.appearance(Appearance::Dark), Appearance::Light);
        assert_eq!(settings.theme(Appearance::Dark).id, "frost");
        settings.preference = Preference::Dark;
        assert_eq!(settings.theme(Appearance::Light).id, "tokyo-night");
    }

    #[test]
    fn an_unknown_theme_falls_back_to_frost() {
        assert_eq!(theme_by_id("nope").id, "frost");
        assert_eq!(default_theme(Appearance::Dark).id, "frost-dark");
    }

    #[test]
    fn a_zed_theme_maps_its_colours_onto_console_tokens() {
        let json = r##"{
            "name": "Fixture",
            "themes": [{
                "name": "Fixture Dark",
                "appearance": "dark",
                "style": {
                    "background": "#101216",
                    "border": "#2a2f38",
                    "text": "#e6e8ec",
                    "element": { "background": "#161a20" },
                    "accents": [{ "color": "#7aa2f7" }]
                }
            }]
        }"##;
        let theme = parse_zed_theme(json).expect("theme");
        assert_eq!(theme.id, "zed-fixture-dark");
        assert_eq!(theme.appearance, Appearance::Dark);
        let vars = theme.css_vars();
        assert!(vars.contains("--bg:#101216"), "{vars}");
        assert!(vars.contains("--fg:#e6e8ec"), "{vars}");
        assert!(vars.contains("--accent:#7aa2f7"), "{vars}");
        for (name, _) in theme.tokens() {
            assert!(TOKENS.contains(&name.as_str()), "unknown token --{name}");
        }
    }

    #[test]
    fn a_zed_file_without_themes_is_rejected() {
        assert_eq!(parse_zed_theme("{\"themes\":[]}"), Err(ThemeError::Empty));
        assert!(matches!(
            parse_zed_theme("not json"),
            Err(ThemeError::Malformed(_))
        ));
        assert_eq!(
            parse_zed_theme("{\"themes\":[{\"name\":\"Bare\",\"style\":{}}]}"),
            Err(ThemeError::NoTokens)
        );
    }

    /// WCAG contrast of two `oklch(L C H)` tokens. OKLab's lightness is close
    /// enough to the cube root of relative luminance that squaring it is a
    /// faithful ratio for a guard like this.
    fn contrast(a: &str, b: &str) -> f32 {
        let l = |token: &str| -> f32 {
            token
                .trim_start_matches("oklch(")
                .split(' ')
                .next()
                .expect("lightness")
                .parse::<f32>()
                .expect("number")
                .powi(3)
        };
        let (ya, yb) = (l(a), l(b));
        (ya.max(yb) + 0.05) / (ya.min(yb) + 0.05)
    }

    #[test]
    fn every_preset_keeps_its_ink_readable() {
        for theme in builtin_themes() {
            let token = |name: &str| {
                theme
                    .tokens()
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default()
            };
            let bg = token("bg");
            let body = contrast(&token("fg"), &bg);
            let muted = contrast(&token("muted"), &bg);
            assert!(body >= 4.5, "{} body ink {body:.2}:1 on {}", theme.id, bg);
            assert!(
                muted >= 3.0,
                "{} muted ink {muted:.2}:1 on {}",
                theme.id,
                bg
            );
        }
    }

    #[test]
    fn every_preset_paints_every_token() {
        for theme in builtin_themes() {
            assert_eq!(
                theme.tokens().len(),
                TOKENS.len(),
                "{} is incomplete",
                theme.id
            );
            let vars = theme.css_vars();
            for name in TOKENS {
                assert!(
                    vars.contains(&format!("--{name}:")),
                    "{} misses --{name}",
                    theme.id
                );
            }
        }
    }

    #[test]
    fn an_imported_theme_paints_with_its_own_tokens() {
        let json = r##"{"themes":[{"name":"Imported","style":{"background":"#010203"}}]}"##;
        let mut settings = Settings::default();
        settings.use_theme(parse_zed_theme(json).expect("theme"));
        assert_eq!(
            settings.theme_dark, "zed-imported",
            "no appearance means dark"
        );
        assert_eq!(settings.css_vars(Appearance::Dark), "--bg:#010203");
    }

    #[test]
    fn kubeconfig_paths_ignore_blanks_and_duplicates() {
        let mut settings = Settings::default();
        settings.add_kubeconfig("  ");
        settings.add_kubeconfig("~/.kube/config");
        settings.add_kubeconfig("~/kube/prod.yaml");
        assert_eq!(settings.kubeconfig_paths.len(), 2);
        settings.remove_kubeconfig("~/.kube/config");
        assert_eq!(settings.kubeconfig_paths, vec!["~/kube/prod.yaml"]);
    }

    /// The frost pair is the one theme whose values are a design decision, not a
    /// vendored palette: it has to look like Apple's glass, so its glass lifts
    /// above the canvas with a tint in it and its accents are the system colours.
    #[test]
    fn the_frost_pair_is_cut_from_apples_palette() {
        let token = |appearance: Appearance, name: &str| -> String {
            default_theme(appearance)
                .tokens()
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
                .unwrap_or_default()
        };
        let number = |raw: &str, index: usize| -> f32 {
            raw.trim_start_matches("oklch(")
                .split([' ', '/'])
                .filter(|part| !part.is_empty())
                .nth(index)
                .expect("component")
                .trim_end_matches(')')
                .parse::<f32>()
                .expect("number")
        };
        let alpha = |raw: &str| -> f32 {
            if raw.contains('/') {
                number(raw, 3)
            } else {
                1.0
            }
        };

        for appearance in [Appearance::Light, Appearance::Dark] {
            let id = default_theme(appearance).id;
            let bg = token(appearance, "bg");
            let glass = token(appearance, "glass");
            assert!(
                number(&glass, 0) > number(&bg, 0),
                "{id}: glass sits above the canvas"
            );
            assert!(number(&glass, 1) > 0.004, "{id}: glass carries a tint");
            assert!(
                alpha(&token(appearance, "surface")) < 0.8,
                "{id}: surface lets the canvas through"
            );
            assert!(
                alpha(&token(appearance, "surface-solid")) < 1.0,
                "{id}: even the solid surface keeps a trace of air"
            );

            let hue = |name: &str| number(&token(appearance, name), 2);
            assert!(
                (250.0..=262.0).contains(&hue("accent")),
                "{id}: accent is system blue, got {:.0}",
                hue("accent")
            );
            assert!(
                (140.0..=155.0).contains(&hue("success")),
                "{id}: success is system green"
            );
            assert!(
                (58.0..=72.0).contains(&hue("warn")),
                "{id}: warn is system orange"
            );
            assert!(
                (24.0..=32.0).contains(&hue("danger")),
                "{id}: danger is system red"
            );
        }
    }
}
