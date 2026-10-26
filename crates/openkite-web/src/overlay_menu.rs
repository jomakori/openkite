//! A reusable overlay menu: one anchored panel, dismissed by Escape or by a
//! press outside it, that hands focus back to the control that opened it.
//!
//! # Why the panel is `position: fixed`
//!
//! A menu's trigger usually does not own its container. The top bar's account
//! control is the shared shell's avatar; the panel must paint outside that
//! bar's overflow and stacking context. So the panel is `position: fixed` (the
//! same shape as the command palette and the drawer) and is *placed* by Rust
//! from the trigger's measured rect: below the trigger when there is room,
//! flipped above when there is not, clamped inside the viewport otherwise.
//! Keeping placement in [`place`] means the flip and clamp rules are testable
//! without a browser.
//!
//! # What the browser half does
//!
//! On open, one script measures the trigger and the panel, sends the rects to
//! Rust, and installs the menu's key handling:
//!
//! - `Escape` dismisses (and swallows the key so nothing behind the menu also
//!   reacts to it);
//! - `ArrowDown` / `ArrowUp` move focus through the items, wrapping;
//! - `Tab` dismisses, which is also how a menu hands focus onward.
//!
//! On close — by any path — Rust runs one more script that focuses the trigger
//! again, so a keyboard user lands back where they started. A full-viewport
//! scrim under the panel is what turns "press outside" into a plain click, and
//! it covers the trigger too, so pressing the trigger while the menu is open
//! closes it instead of reopening it.
//!
//! # Why this lives in the browser host
//!
//! The shared crate renders the shell for two hosts and owns no browser
//! behaviour; the ticket keeps the primitive here (`crates/openkite-web`) until
//! the desktop host needs it too. Styles ship from `assets/overlay-menu.css`
//! and every selector is anchored to `[data-overlay-menu]`: plugins share the
//! DOM with the host (D3 in `docs/design-surfaces.md`), so a plugin sheet must
//! not be able to reach into this one, or the reverse.

use dioxus::prelude::*;
use serde::Deserialize;

/// The gap between the trigger and the panel, in CSS pixels.
pub const GAP: f64 = 6.0;

/// The smallest distance the panel keeps from any viewport edge.
pub const MARGIN: f64 = 8.0;

/// The primitive's stylesheet, injected by the host next to the console sheet.
pub const CSS: &str = include_str!("../assets/overlay-menu.css");

/// The trigger's client rect, in viewport coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Anchor {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The panel's measured size.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Panel {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The viewport the panel has to stay inside.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Viewport {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Which edge of the trigger the panel shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// The panel's left edge meets the trigger's left edge.
    Start,
    /// The panel's right edge meets the trigger's right edge (top-bar menus).
    End,
}

/// Which side of the trigger the panel opened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Under the trigger.
    Below,
    /// Above the trigger: no room below.
    Above,
}

impl Side {
    /// The value the panel writes to `data-side`.
    pub fn data_value(self) -> &'static str {
        match self {
            Side::Below => "below",
            Side::Above => "above",
        }
    }
}

/// Where the panel goes, and which side it took.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Distance from the viewport top.
    pub top: f64,
    /// Distance from the viewport left.
    pub left: f64,
    /// The side the panel opened on.
    pub side: Side,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            top: MARGIN,
            left: MARGIN,
            side: Side::Below,
        }
    }
}

impl Placement {
    /// The inline style that pins the panel.
    pub fn style(&self) -> String {
        format!(
            "position: fixed; top: {:.1}px; left: {:.1}px;",
            self.top, self.left
        )
    }
}

/// Place a panel against its trigger.
///
/// Below the trigger when the panel plus [`GAP`] fits above the bottom
/// [`MARGIN`], above it when there is room there but not below, and below
/// (clamped) when neither side fits. Horizontally the panel hangs off the
/// trigger's [`Align`] edge and is clamped to [`MARGIN`]..`viewport - MARGIN`.
pub fn place(anchor: Anchor, panel: Panel, viewport: Viewport, align: Align) -> Placement {
    let below = anchor.y + anchor.height + GAP;
    let above = anchor.y - GAP - panel.height;
    let floor = MARGIN;
    let ceiling = viewport.height - MARGIN;

    let side = if below + panel.height <= ceiling {
        Side::Below
    } else if above >= floor {
        Side::Above
    } else {
        Side::Below
    };

    let top = match side {
        Side::Below => below,
        Side::Above => above,
    };

    let preferred = match align {
        Align::Start => anchor.x,
        Align::End => anchor.x + anchor.width - panel.width,
    };
    let rightmost = viewport.width - panel.width - MARGIN;
    let left = if panel.width + 2.0 * MARGIN > viewport.width {
        // Wider than the viewport: the stylesheet's `max-width` shrinks it, and
        // the margin is the only honest anchor.
        MARGIN
    } else {
        preferred.clamp(MARGIN, rightmost.max(MARGIN))
    };

    Placement { top, left, side }
}

// ---------------------------------------------------------------------------
// Browser scripts
// ---------------------------------------------------------------------------

/// Measure, then listen. Runs when the panel appears.
///
/// The promise never settles: the `dioxus.send` channel has to outlive the
/// script for a later Escape to arrive, and re-rendering the panel must not
/// lose the listener.
const OPEN_JS: &str = r#"
new Promise(() => {
  const id = __ID__;
  const anchorSelector = __ANCHOR__;
  const panel = () => document.querySelector('[data-overlay-menu-panel="' + id + '"]');
  const send = (message) => dioxus.send(JSON.stringify(message));

  const measure = () => {
    const node = panel();
    if (!node) return;
    const trigger = document.querySelector(anchorSelector);
    if (!trigger) return;
    const a = trigger.getBoundingClientRect();
    const p = node.getBoundingClientRect();
    send({
      kind: 'measure',
      anchor: { x: a.left, y: a.top, width: a.width, height: a.height },
      panel: { width: p.width, height: p.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    });
  };

  const key = '__openkite_overlay_' + id;
  let uninstall = () => {};

  const items = () => Array.from(
    panel()?.querySelectorAll(
      '[role="menuitem"]:not([aria-disabled="true"]):not([disabled])',
    ) ?? [],
  );

  uninstall = () => {
    document.removeEventListener('keydown', onKey, true);
    if (window[key] === uninstall) delete window[key];
  };

  const onKey = (event) => {
    if (!panel()) {
      // The host closed the menu without an Escape or a Tab: stop listening.
      uninstall();
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      uninstall();
      send({ kind: 'dismiss' });
      return;
    }
    if (event.key === 'Tab') {
      uninstall();
      send({ kind: 'dismiss' });
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    const list = items();
    if (!list.length) return;
    event.preventDefault();
    const at = list.indexOf(document.activeElement);
    const step = event.key === 'ArrowDown' ? 1 : -1;
    const next = at < 0
      ? (step > 0 ? 0 : list.length - 1)
      : (at + step + list.length) % list.length;
    list[next].focus();
  };

  // One session's listener per menu id: a reopen replaces the last one instead
  // of stacking a second handler that would dismiss twice.
  window[key]?.();
  window[key] = uninstall;
  document.addEventListener('keydown', onKey, true);
  requestAnimationFrame(measure);
});
"#;

/// Put focus on the panel's first item once the panel is placed.
const FOCUS_JS: &str = r#"
(() => {
  const panel = document.querySelector('[data-overlay-menu-panel="' + __ID__ + '"]');
  if (!panel) return;
  const first = panel.querySelector(
    '[role="menuitem"]:not([aria-disabled="true"]):not([disabled])',
  );
  (first ?? panel).focus({ preventScroll: true });
})();
"#;

/// Give focus back to the trigger. Runs on every close.
const RESTORE_JS: &str = r#"
(() => {
  const trigger = document.querySelector(__ANCHOR__);
  if (!trigger || typeof trigger.focus !== 'function') return;
  if (trigger.tabIndex < 0 && !trigger.hasAttribute('tabindex')) {
    trigger.setAttribute('tabindex', '-1');
  }
  trigger.focus({ preventScroll: true });
})();
"#;

/// What the browser half reports back.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Message {
    /// The trigger and panel have been measured.
    Measure {
        /// The trigger's rect.
        anchor: Anchor,
        /// The panel's size.
        panel: Panel,
        /// The viewport the panel must stay inside.
        viewport: Viewport,
    },
    /// Escape, Tab, or a press outside the panel.
    Dismiss,
}

/// Fill a script template with the JSON-quoted menu id and trigger selector.
fn script(template: &str, id: &str, anchor: &str) -> String {
    template
        .replace("__ID__", &quote(id))
        .replace("__ANCHOR__", &quote(anchor))
}

/// A JS string literal for `value`.
fn quote(value: &str) -> String {
    serde_json::to_string(value).expect("a string serializes as JSON")
}

/// Read one message from the browser half.
fn parse_message(raw: &str) -> Option<Message> {
    serde_json::from_str(raw).ok()
}

/// A reusable anchored menu.
///
/// `open` is the caller's state: the primitive reports dismissal through
/// `on_dismiss` and never closes itself. `anchor` is the CSS selector of the
/// control the panel hangs off; `id` names the panel element and is what the
/// trigger's `aria-controls` should point at.
#[component]
pub fn OverlayMenu(
    /// The panel's element id, e.g. `openkite-account-menu`.
    id: String,
    /// The CSS selector of the control the panel hangs off.
    anchor: String,
    /// Whether the menu is showing.
    open: bool,
    /// The panel's accessible name.
    label: String,
    /// Which edge of the trigger the panel shares.
    #[props(default = Align::End)]
    align: Align,
    /// Called when the user dismisses the menu (Escape, Tab, a press outside).
    on_dismiss: EventHandler<()>,
    /// The menu's items.
    children: Element,
) -> Element {
    let mut placement = use_signal(|| Option::<Placement>::None);
    // The previous `open`, so the effect can tell an opening from a closing.
    let mut was_open = use_signal(|| false);

    // The effect re-runs on every open and close, so it owns its own copies of
    // the two strings the scripts are built from.
    let effect_id = id.clone();
    let effect_anchor = anchor.clone();
    use_effect(move || {
        let is_open = open;
        let was = was_open();
        if is_open == was {
            return;
        }
        was_open.set(is_open);

        if !is_open {
            placement.set(None);
            let _ = document::eval(&script(RESTORE_JS, &effect_id, &effect_anchor));
            return;
        }

        let session_id = effect_id.clone();
        let session_anchor = effect_anchor.clone();
        let mut eval = document::eval(&script(OPEN_JS, &effect_id, &effect_anchor));
        spawn(async move {
            while let Ok(raw) = eval.recv::<String>().await {
                match parse_message(&raw) {
                    Some(Message::Measure {
                        anchor,
                        panel,
                        viewport,
                    }) => {
                        placement.set(Some(place(anchor, panel, viewport, align)));
                        let _ = document::eval(&script(FOCUS_JS, &session_id, &session_anchor));
                    }
                    Some(Message::Dismiss) => on_dismiss.call(()),
                    None => {}
                }
            }
        });
    });

    rsx! {
        if open {
            // A press anywhere but the panel lands here: the trigger included,
            // so pressing it while open closes the menu rather than toggling.
            div {
                class: "overlay-menu-scrim",
                "data-overlay-menu": "",
                "aria-hidden": "true",
                onclick: move |_| on_dismiss.call(()),
            }
            div {
                class: "overlay-menu",
                id: "{id}",
                "data-overlay-menu": "",
                "data-overlay-menu-panel": "{id}",
                "data-positioned": if placement().is_some() { "true" } else { "false" },
                "data-side": placement().map(|placed| placed.side.data_value()).unwrap_or("below"),
                style: placement().map(|placed| placed.style()).unwrap_or_default(),
                role: "menu",
                "aria-label": "{label}",
                tabindex: "-1",
                {children}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measurement_round_trips() {
        let raw = r#"{"kind":"measure","anchor":{"x":10.0,"y":20.0,"width":30.0,"height":40.0},
            "panel":{"width":200.0,"height":100.0},
            "viewport":{"width":800.0,"height":600.0}}"#;
        assert_eq!(
            parse_message(raw),
            Some(Message::Measure {
                anchor: Anchor {
                    x: 10.0,
                    y: 20.0,
                    width: 30.0,
                    height: 40.0,
                },
                panel: Panel {
                    width: 200.0,
                    height: 100.0,
                },
                viewport: Viewport {
                    width: 800.0,
                    height: 600.0,
                },
            })
        );
        assert_eq!(
            parse_message(r#"{"kind":"dismiss"}"#),
            Some(Message::Dismiss)
        );
        assert_eq!(parse_message("not json"), None);
    }

    #[test]
    fn the_scripts_quote_their_inputs() {
        let filled = script(OPEN_JS, "menu\"id", ".topbar .avatar");
        assert!(filled.contains(r#"const id = "menu\"id";"#), "{filled}");
        assert!(
            filled.contains(r#"anchorSelector = ".topbar .avatar";"#),
            "{filled}"
        );
        assert!(!filled.contains("__ID__"), "{filled}");
        assert!(!filled.contains("__ANCHOR__"), "{filled}");
    }

    #[test]
    fn escape_is_swallowed_and_dismisses() {
        assert!(OPEN_JS.contains("event.stopPropagation()"), "{OPEN_JS}");
        assert!(OPEN_JS.contains("event.key === 'Escape'"), "{OPEN_JS}");
        assert!(OPEN_JS.contains("kind: 'dismiss'"), "{OPEN_JS}");
        assert!(RESTORE_JS.contains("trigger.focus("), "{RESTORE_JS}");
    }
}
