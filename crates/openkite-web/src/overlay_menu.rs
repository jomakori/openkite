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

/// Watches for the panel and keeps the menu's browser half in step with it.
///
/// Installed once per mount rather than once per open: it measures whenever the
/// panel appears, listens for keys while the panel is up, and hands focus back
/// to the trigger when it goes. The promise never settles, so the `dioxus.send`
/// channel outlives the script and a later Escape still arrives.
const WATCH_JS: &str = r#"
new Promise(() => {
  const id = __ID__;
  const anchorSelector = __ANCHOR__;
  const key = '__openkite_overlay_' + id;
  // A remount must not stack a second observer on the same menu.
  if (window[key]) return;

  const panel = () => document.querySelector('[data-overlay-menu-panel="' + id + '"]');
  const trigger = () => document.querySelector(anchorSelector);
  const send = (message) => dioxus.send(JSON.stringify(message));
  const items = () => Array.from(
    panel()?.querySelectorAll(
      '[role="menuitem"]:not([aria-disabled="true"]):not([disabled])',
    ) ?? [],
  );

  let open = false;
  let listening = false;
  let focused = false;

  const onKey = (event) => {
    if (!open) return;
    if (event.key === 'Escape' || event.key === 'Tab') {
      // Tab hands focus on, so the panel has no business staying up either.
      if (event.key === 'Escape') event.preventDefault();
      event.stopPropagation();
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

  const appear = () => {
    open = true;
    if (!listening) {
      document.addEventListener('keydown', onKey, true);
      listening = true;
    }
    const node = panel();
    const anchor = trigger();
    if (!node || !anchor) return;
    const a = anchor.getBoundingClientRect();
    const p = node.getBoundingClientRect();
    send({
      kind: 'measure',
      anchor: { x: a.left, y: a.top, width: a.width, height: a.height },
      panel: { width: p.width, height: p.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    });
  };

  const vanish = () => {
    open = false;
    if (listening) {
      document.removeEventListener('keydown', onKey, true);
      listening = false;
    }
    focused = false;
    send({ kind: 'closed' });
    const anchor = trigger();
    if (!anchor || typeof anchor.focus !== 'function') return;
    if (anchor.tabIndex < 0 && !anchor.hasAttribute('tabindex')) {
      anchor.setAttribute('tabindex', '-1');
    }
    anchor.focus({ preventScroll: true });
  };

  const sync = () => {
    const node = panel();
    const present = Boolean(node);
    if (present !== open) {
      if (present) appear();
      else vanish();
    }
    if (!node || focused) return;
    // An unplaced panel is hidden, and a hidden node cannot take focus: wait
    // for the host to paint the placement it computed from our measurement.
    if (node.getAttribute('data-positioned') !== 'true') return;
    focused = true;
    (items()[0] ?? node).focus({ preventScroll: true });
  };

  const observer = new MutationObserver(sync);
  observer.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ['data-positioned'],
  });
  window[key] = () => {
    observer.disconnect();
    if (listening) document.removeEventListener('keydown', onKey, true);
  };
  sync();
});
"#;

/// Drops the watcher when the menu unmounts.
const TEARDOWN_JS: &str = r#"
(() => {
  const key = '__openkite_overlay_' + __ID__;
  if (typeof window[key] === 'function') window[key]();
  delete window[key];
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
    /// The panel left the DOM: forget where it sat.
    Closed,
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

    // The watcher is installed once per mount and follows the panel itself, so
    // nothing here has to react to `open` flipping: the panel appearing is what
    // starts a measuring session, and the panel leaving is what ends it.
    let watch_id = id.clone();
    let watch_anchor = anchor.clone();
    use_effect(move || {
        let mut eval = document::eval(&script(WATCH_JS, &watch_id, &watch_anchor));
        spawn(async move {
            while let Ok(raw) = eval.recv::<String>().await {
                match parse_message(&raw) {
                    Some(Message::Measure {
                        anchor,
                        panel,
                        viewport,
                    }) => placement.set(Some(place(anchor, panel, viewport, align))),
                    Some(Message::Dismiss) => on_dismiss.call(()),
                    Some(Message::Closed) => placement.set(None),
                    None => {}
                }
            }
        });
    });

    let drop_id = id.clone();
    use_drop(move || {
        let _ = document::eval(&TEARDOWN_JS.replace("__ID__", &quote(&drop_id)));
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
        let filled = script(WATCH_JS, "menu\"id", ".topbar .avatar");
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
        assert!(WATCH_JS.contains("event.stopPropagation()"), "{WATCH_JS}");
        assert!(WATCH_JS.contains("event.key === 'Escape'"), "{WATCH_JS}");
        assert!(WATCH_JS.contains("kind: 'dismiss'"), "{WATCH_JS}");
        assert!(WATCH_JS.contains("anchor.focus("), "{WATCH_JS}");
    }

    #[test]
    fn the_watcher_arms_itself_when_the_panel_appears() {
        // The bug this guards: a watcher that only measured on an `open` flip
        // never fired for a panel that was already up at mount.
        assert!(
            WATCH_JS.contains("new MutationObserver(sync)"),
            "{WATCH_JS}"
        );
        assert!(WATCH_JS.contains("sync();"), "{WATCH_JS}");
        assert!(WATCH_JS.contains("kind: 'measure'"), "{WATCH_JS}");
        assert!(WATCH_JS.contains("kind: 'closed'"), "{WATCH_JS}");
        assert!(TEARDOWN_JS.contains("delete window[key]"), "{TEARDOWN_JS}");
    }
}
