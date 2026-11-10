//! The v1.1 registration kinds the JS bridge accepts: a plugin's own stylesheet
//! and the controls its surface offers.
//!
//! v1.0 knew sidebar/route/status only. v1.1 adds `style` and `control`; these
//! tests pin that the bridge stores what the plugin authored, replaces a sheet
//! on re-registration instead of stacking one, and refuses a malformed payload
//! loudly rather than registering half of it.

use openkite_host::bridge::apply_register;
use openkite_ui::plugin_api::RegistrationStore;
use serde_json::json;

#[test]
fn a_style_registration_is_stored_as_authored() {
    let mut store = RegistrationStore::default();
    apply_register(
        &mut store,
        "argocd",
        "style",
        json!({ "css": ".app-card { color: red }" }),
    )
    .expect("style registration");

    let registration = store.get("argocd").expect("registered");
    assert_eq!(
        registration.style.as_deref(),
        Some(".app-card { color: red }")
    );
}

#[test]
fn re_registering_a_style_replaces_it_instead_of_stacking() {
    let mut store = RegistrationStore::default();
    for css in [".a { color: red }", ".b { color: blue }"] {
        apply_register(&mut store, "argocd", "style", json!({ "css": css })).expect("style");
    }

    let registration = store.get("argocd").expect("registered");
    assert_eq!(registration.style.as_deref(), Some(".b { color: blue }"));
}

#[test]
fn a_style_registration_without_css_is_refused() {
    let mut store = RegistrationStore::default();
    let err = apply_register(&mut store, "argocd", "style", json!({ "sheet": "x" }))
        .expect_err("a style without css cannot be scoped");

    assert!(
        err.contains("css"),
        "error should name the missing field: {err}"
    );
    assert!(
        store.get("argocd").is_none(),
        "nothing is registered on refusal"
    );
}

#[test]
fn a_control_registration_round_trips() {
    let mut store = RegistrationStore::default();
    apply_register(
        &mut store,
        "argocd",
        "control",
        json!({ "label": "Sync", "action": "sync" }),
    )
    .expect("control registration");

    let controls = store
        .get("argocd")
        .map(|reg| reg.controls.clone())
        .unwrap_or_default();
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0].label, "Sync");
    assert_eq!(controls[0].action, "sync");
    assert_eq!(controls[0].unsupported, None);
}

#[test]
fn a_control_the_plugin_cannot_honour_keeps_its_reason() {
    let mut store = RegistrationStore::default();
    apply_register(
        &mut store,
        "argocd",
        "control",
        json!({
            "label": "Rollback",
            "action": "rollback",
            "unsupported": "needs ArgoCD 2.10",
        }),
    )
    .expect("control registration");

    let control = store
        .get("argocd")
        .and_then(|reg| reg.controls.first().cloned())
        .expect("control registered");
    assert_eq!(control.unsupported.as_deref(), Some("needs ArgoCD 2.10"));
}

#[test]
fn an_unknown_registration_kind_is_refused_and_names_itself() {
    let mut store = RegistrationStore::default();
    let err = apply_register(&mut store, "argocd", "telemetry", json!({}))
        .expect_err("v1.1 has no telemetry kind");

    assert!(
        err.contains("telemetry"),
        "error should name the kind: {err}"
    );
}
