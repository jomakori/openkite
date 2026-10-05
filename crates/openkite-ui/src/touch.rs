//! Touch input: the press rules behind the explicit substitute a touch screen
//! needs for a double-click, and the copy the promoted dock tab toasts.
//!
//! Pure logic — no Dioxus, no host. A consumer keeps a [`PressGesture`], feeds
//! it the pointer's coordinates, and turns a [`PressIntent::LongPress`] into
//! the same action the double-click performs.

/// How long a press must rest without moving before it becomes a long-press.
pub const LONG_PRESS_MS: u32 = 500;

/// How far a pointer may drift and still count as a press rather than a
/// scroll.
pub const PRESS_MOVE_TOLERANCE_PX: f64 = 10.0;

/// The action a long-press on a dock's temporary tab performs — the touch
/// stand-in for the double-click that promotes it.
pub const KEEP_TAB_LABEL: &str = "Keep tab";

/// What a press resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressIntent {
    /// Resting past [`LONG_PRESS_MS`] without moving: the double-click's
    /// touch substitute.
    LongPress,
    /// A quick, still press: an ordinary tap.
    Tap,
    /// The pointer travelled: a scroll (or a drag), never a long-press.
    Scroll,
}

/// Resolve a press from the distance it travelled and how long it was held.
///
/// Movement is judged first, so a long-press can never be confused with a
/// scroll: a gesture that travels past [`PRESS_MOVE_TOLERANCE_PX`] is a
/// scroll however long the finger then rests on the glass.
pub fn resolve_press(dx: f64, dy: f64, held_ms: u32) -> PressIntent {
    if dx.hypot(dy) > PRESS_MOVE_TOLERANCE_PX {
        PressIntent::Scroll
    } else if held_ms >= LONG_PRESS_MS {
        PressIntent::LongPress
    } else {
        PressIntent::Tap
    }
}

/// The confirmation a kept tab toasts.
pub fn keep_tab_toast(label: &str) -> String {
    format!("Keeping {label} — later selections will not replace it.")
}

/// A press in flight: its origin and whether it has already become a scroll.
///
/// The scroll latch is the point. A finger that drifts past the tolerance and
/// settles back onto its origin is *still* a scroll — the browser has already
/// moved the surface under it — so the latch is never cleared until the
/// gesture ends.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PressGesture {
    origin: Option<(f64, f64)>,
    scrolled: bool,
}

impl PressGesture {
    /// Begin a press at `(x, y)`.
    pub fn down(&mut self, x: f64, y: f64) {
        self.origin = Some((x, y));
        self.scrolled = false;
    }

    /// Track a moved pointer; returns whether the press is still a candidate
    /// for a long-press (false once it has become a scroll).
    pub fn moved(&mut self, x: f64, y: f64) -> bool {
        let Some((x0, y0)) = self.origin else {
            return false;
        };
        if resolve_press(x - x0, y - y0, 0) == PressIntent::Scroll {
            self.scrolled = true;
        }
        self.armed()
    }

    /// Resolve the press when it ends or when its long-press timer elapses.
    ///
    /// A latched scroll stays a scroll; an abandoned press resolves to
    /// [`PressIntent::Scroll`] so a stray timer can never fire.
    pub fn resolve(&self, held_ms: u32) -> PressIntent {
        if self.scrolled || self.origin.is_none() {
            PressIntent::Scroll
        } else {
            resolve_press(0.0, 0.0, held_ms)
        }
    }

    /// Whether the press is still a candidate for a long-press.
    pub fn armed(&self) -> bool {
        self.origin.is_some() && !self.scrolled
    }

    /// Abandon the press (`pointercancel`, or the surface leaving the frame),
    /// so nothing it was waiting on can still fire.
    pub fn cancel(&mut self) {
        self.origin = None;
        self.scrolled = false;
    }
}
