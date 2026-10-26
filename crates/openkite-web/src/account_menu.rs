//! The top bar's account menu: who this host reads the cluster as, and the one
//! action the account surface can actually run here.
//!
//! # Trigger
//!
//! The shared shell renders `.avatar` from its `identity` prop with no handler,
//! and it has no content slot for the top bar (the ticket keeps this host out
//! of that crate). So the trigger is the design's own avatar and this module
//! binds the documented `.avatar` node — `docs/design-surfaces.md` lists it as
//! the top bar's account control — from the document, writing the
//! `aria-haspopup` / `aria-expanded` / `aria-controls` state the browser reads
//! back. Nothing about the shell's markup changes.
//!
//! # What the menu says
//!
//! There is no signed-in person in this host: it has no session, no login and
//! no user list, so the account is the credential the gateway authenticates
//! with. The menu names that credential and offers the one account-shaped
//! action the host can run (a refresh). "Sign out" is declared, disabled, with
//! the reason on its title: naming a command the host cannot run would be a
//! lie in the chrome.

use dioxus::prelude::*;

use crate::overlay_menu::OverlayMenu;
use crate::ssr::AccountIdentity;

/// The design's account control in the shared top bar.
pub const TRIGGER_SELECTOR: &str = ".topbar .avatar";

/// The panel's element id, so the trigger can point `aria-controls` at it.
pub const PANEL_ID: &str = "openkite-account-menu";

/// Why "Sign out" is disabled, in the words the title shows.
const NO_SESSION: &str =
    "This host has no session to end: it reads the cluster with the host's own credential.";

/// The document listener that turns a press on the shared avatar into a toggle.
///
/// The promise never settles: the channel has to outlive this script for later
/// presses to arrive, and the trigger node is only found again at press time
/// (the shell may re-render the top bar underneath us).
const TRIGGER_JS: &str = r#"
new Promise(() => {
  if (window.__openkite_account_trigger) return;
  window.__openkite_account_trigger = true;
  document.addEventListener('click', (event) => {
    const node = event.target instanceof Element
      ? event.target.closest(__ANCHOR__)
      : null;
    if (!node) return;
    event.preventDefault();
    dioxus.send('toggle');
  }, true);
});
"#;

/// The trigger's expanded state, written where the browser's a11y tree reads it.
const EXPANDED_JS: &str = r#"
(() => {
  const trigger = document.querySelector(__ANCHOR__);
  if (!trigger) return;
  trigger.setAttribute('aria-haspopup', 'menu');
  trigger.setAttribute('aria-controls', __PANEL__);
  trigger.setAttribute('aria-expanded', __EXPANDED__);
})();
"#;

/// Fill the expanded-state template.
fn expanded_script(open: bool) -> String {
    EXPANDED_JS
        .replace(
            "__ANCHOR__",
            &serde_json::to_string(TRIGGER_SELECTOR).expect("selector serializes"),
        )
        .replace(
            "__PANEL__",
            &serde_json::to_string(PANEL_ID).expect("panel id serializes"),
        )
        .replace("__EXPANDED__", if open { "'true'" } else { "'false'" })
}

/// The fill for the trigger listener.
fn trigger_script() -> String {
    TRIGGER_JS.replace(
        "__ANCHOR__",
        &serde_json::to_string(TRIGGER_SELECTOR).expect("selector serializes"),
    )
}

/// The top bar's account menu, built on [`OverlayMenu`].
#[component]
pub fn AccountMenu(
    account: AccountIdentity,
    on_refresh: EventHandler<()>,
    /// Whether the panel starts open. The host lets it start closed; the SSR
    /// tests paint it open to read the panel's markup without a browser.
    #[props(default)]
    initially_open: bool,
) -> Element {
    let mut open = use_signal(move || initially_open);

    // Bound once per page: the listener outlives every open/close of the panel.
    use_effect(move || {
        let mut eval = document::eval(&trigger_script());
        spawn(async move {
            while let Ok(action) = eval.recv::<String>().await {
                if action == "toggle" {
                    open.toggle();
                }
            }
        });
    });

    // The expanded state is the trigger's, and the trigger is the shell's.
    use_effect(move || {
        let _ = document::eval(&expanded_script(open()));
    });

    rsx! {
        OverlayMenu {
            id: PANEL_ID.to_string(),
            anchor: TRIGGER_SELECTOR.to_string(),
            open: open(),
            label: "Account".to_string(),
            on_dismiss: move |_| open.set(false),
            div { class: "overlay-menu-header",
                span { class: "overlay-menu-name", "{account.name}" }
                span { class: "overlay-menu-detail", "{account.detail}" }
            }
            button {
                class: "overlay-menu-item",
                r#type: "button",
                role: "menuitem",
                onclick: move |_| {
                    open.set(false);
                    on_refresh.call(());
                },
                "Refresh"
            }
            button {
                class: "overlay-menu-item",
                r#type: "button",
                role: "menuitem",
                disabled: true,
                "aria-disabled": "true",
                title: NO_SESSION,
                "Sign out"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trigger_is_the_documented_avatar() {
        assert_eq!(TRIGGER_SELECTOR, ".topbar .avatar");
    }

    #[test]
    fn the_expanded_state_is_written_for_both_states() {
        let closed = expanded_script(false);
        assert!(closed.contains(r#"aria-expanded', 'false'"#), "{closed}");
        assert!(closed.contains(r#"".topbar .avatar""#), "{closed}");
        assert!(!closed.contains("__ANCHOR__"));

        let open = expanded_script(true);
        assert!(open.contains(r#"aria-expanded', 'true'"#), "{open}");
        assert!(open.contains(r#""openkite-account-menu""#), "{open}");
    }

    #[test]
    fn the_trigger_listener_quotes_its_selector() {
        let script = trigger_script();
        assert!(script.contains(r#"closest(".topbar .avatar")"#), "{script}");
        assert!(script.contains("dioxus.send('toggle')"), "{script}");
    }
}
