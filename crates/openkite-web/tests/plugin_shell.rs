//! The host shell paints what a plugin registered (OKT-196).
//!
//! A plugin reaches the shell by one path, on both hosts: it POSTs a `register`
//! envelope to `/openkite`, the host merges it into the bridge's live
//! `RegistrationStore`, and the shell is built from that store. On the web host
//! the store is the one the bridge keeps — the same object the desktop mounts —
//! and it rides the snapshot, so the SSR document and the hydrating client are
//! built from one set of registrations.
//!
//! These tests drive the real router rather than the model, because the defect
//! they cover was exactly that the render never read the store: every call
//! succeeded, the store held the registration, and the painted sidebar was
//! empty. A model-level test would have passed over it.

mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use openkite_host::bridge::Bridge;
use serde_json::{json, Value};
use support::{app, get_body, post_json};

fn temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// The envelope the browser's bridge JS posts for one registration.
fn register_envelope(id: u64, plugin: &str, kind: &str, payload: Value) -> Value {
    json!({
        "id": id,
        "plugin": plugin,
        "request": {"op": "register", "kind": kind, "payload": payload},
    })
}

/// A registered sidebar entry is in the nav of a full page render.
#[tokio::test]
async fn a_registered_plugin_nav_entry_renders_in_the_host_sidebar() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    let (status, envelope) = post_json(
        app.clone(),
        "/openkite",
        register_envelope(
            1,
            "argocd",
            "sidebar",
            json!({"label": "Argo CD", "route": "/argocd"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "POST /openkite: {envelope}");
    assert_eq!(
        envelope["status"], "ok",
        "the bridge accepted the registration: {envelope}"
    );

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    // The plugin's section, built by the shell from the live store.
    assert!(
        body.contains("data-plugin=\"argocd\""),
        "the sidebar has no section for the registered plugin: {body}"
    );
    // Its entry is a link to the route it named — the nav the user clicks.
    assert!(
        body.contains("href=\"/argocd\""),
        "the registered nav entry is not a link: {body}"
    );
}

/// A plugin whose stylesheet the crate refuses keeps a row: disabled, with the
/// reason, instead of disappearing.
#[tokio::test]
async fn a_refused_stylesheet_renders_a_disabled_row_with_the_reason() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    for envelope in [
        register_envelope(
            1,
            "flux",
            "sidebar",
            json!({"label": "Flux", "route": "/flux"}),
        ),
        // `@font-face` is global by construction: the crate cannot scope it to
        // the plugin's subtree, so the sheet is refused.
        register_envelope(
            2,
            "flux",
            "style",
            json!({"css": "@font-face { font-family: x; src: url(x.woff2); }"}),
        ),
    ] {
        let (status, answer) = post_json(app.clone(), "/openkite", envelope).await;
        assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");
        assert_eq!(answer["status"], "ok", "registration refused: {answer}");
    }

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    // The plugin is named by a row the user can see...
    assert!(
        body.contains("data-plugin=\"flux\""),
        "the refused plugin has no row at all: {body}"
    );
    // ...as a disabled one, carrying the reason...
    assert!(
        body.contains("data-disabled=\"1\""),
        "the refused plugin's row is not disabled: {body}"
    );
    assert!(
        body.contains("@font-face is global and cannot be scoped"),
        "the disabled row does not carry the reason: {body}"
    );
    // ...with nowhere to go, and no sheet painted for it.
    assert!(
        !body.contains("href=\"/flux\""),
        "the disabled row is still a link: {body}"
    );
    assert!(
        !body.contains("data-plugin-css=\"flux\""),
        "the refused sheet was painted anyway: {body}"
    );
}

/// The SSR document and the hydrating client are built from one snapshot, so
/// the registrations the server painted are the ones the client hydrates.
#[tokio::test]
async fn the_hydration_snapshot_carries_the_live_registrations() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    let (status, answer) = post_json(
        app.clone(),
        "/openkite",
        register_envelope(
            1,
            "argocd",
            "sidebar",
            json!({"label": "Argo CD", "route": "/argocd"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    let snapshot = embedded_snapshot(&body);
    let registrations = &snapshot["registrations"];
    assert!(
        registrations["by_plugin"]["argocd"]["sidebar"][0]["route"] == "/argocd",
        "the client would hydrate an empty shell: {registrations}"
    );
}

/// A control a plugin registers on its own route reaches the browser host live,
/// and a click lands in the plugin's handler (OKT-209).
///
/// The SSR pass has no document, so the button's markup is where the host's
/// wiring shows: a control on a plugin-owned route renders as a live button
/// carrying its owner, not as the inert `data-unsupported="unwired"`
/// affordance the crate paints for a host that supplied no handler. The hop the
/// click takes is the crate's own dispatch string, executed here against the
/// same bridge the page injects — one string, so the desktop's `document::eval`
/// and this host's are the same bytes.
#[tokio::test]
async fn a_plugin_control_renders_live_and_fires_through_the_page_bridge() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    for envelope in [
        register_envelope(
            1,
            "argocd",
            "route",
            json!({"path": "/argocd/apps", "title": "Argo CD Applications"}),
        ),
        register_envelope(
            2,
            "argocd",
            "control",
            json!({"label": "Sync all", "action": "sync"}),
        ),
        // A second plugin owns a different route and declares its own control:
        // its button must not appear on argocd's.
        register_envelope(
            3,
            "flux",
            "route",
            json!({"path": "/flux", "title": "Flux"}),
        ),
        register_envelope(
            4,
            "flux",
            "control",
            json!({"label": "Reconcile", "action": "reconcile"}),
        ),
    ] {
        let (status, answer) = post_json(app.clone(), "/openkite", envelope).await;
        assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");
        assert_eq!(answer["status"], "ok", "registration refused: {answer}");
    }

    let (status, body) = get_body(app, "/argocd/apps").await;
    assert_eq!(status, StatusCode::OK);
    let button = button_for(&body, "sync");
    assert!(
        button.contains("data-plugin=\"argocd\""),
        "the control does not carry its owner: {button}"
    );
    assert!(
        !button.contains("disabled") && !button.contains("data-unsupported"),
        "the host rendered the control inert on a route it owns: {button}"
    );
    assert!(
        !body.contains("data-control=\"reconcile\""),
        "another plugin's control painted on argocd's route: {body}"
    );

    // The click hop: the bridge the page injects, plus the script the host
    // evals, reaching the handler a plugin installs with `openkite.onControl`.
    let fired = fire_control("argocd", "sync");
    assert_eq!(
        fired.as_deref(),
        Some(&["argocd".to_string(), "sync".to_string()][..]),
        "the click did not reach the plugin's onControl handler"
    );
}

/// A route a plugin registers is a page of its own on the web host (OKT-209).
///
/// Before the host resolved the path against the live store, `/argocd/apps`
/// painted the landing dashboard: the registration succeeded, the sidebar
/// linked to the path, and the link went nowhere the plugin owned. The chrome
/// now serves the plugin's route — its head, its controls and the declaration
/// of what this host cannot mount — and a path nothing registered still falls
/// back to `/`.
#[tokio::test]
async fn a_registered_plugin_route_is_navigable_on_the_web_host() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    for envelope in [
        // No sidebar entry: the deep-linkable detail view a plugin declares is
        // still a console view of its own.
        register_envelope(
            1,
            "argocd",
            "route",
            json!({"path": "/argocd/apps", "title": "Argo CD Applications"}),
        ),
        register_envelope(
            2,
            "argocd",
            "control",
            json!({"label": "Sync all", "action": "sync"}),
        ),
    ] {
        let (status, answer) = post_json(app.clone(), "/openkite", envelope).await;
        assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");
        assert_eq!(answer["status"], "ok", "registration refused: {answer}");
    }

    let (status, body) = get_body(app.clone(), "/argocd/apps").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-route=\"/argocd/apps\""),
        "the plugin route did not render as itself: {body}"
    );
    assert!(
        !body.contains("data-route=\"/\""),
        "the landing route painted over the plugin route: {body}"
    );
    // The browser host serves no plugin bundles, and says so instead of
    // painting a blank body it cannot fill.
    assert!(
        body.contains("data-unsupported=\"plugin-route\""),
        "the route does not declare the surface this host lacks: {body}"
    );
    assert!(
        body.contains("data-control=\"sync\""),
        "the plugin's control is not on its route: {body}"
    );

    // The same page for a trailing slash...
    let (status, body) = get_body(app.clone(), "/argocd/apps/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-route=\"/argocd/apps\""),
        "a trailing slash lost the plugin route: {body}"
    );

    // ...and a path nothing registered still lands on the landing route.
    let (status, body) = get_body(app, "/argocd/other").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-route=\"/\""),
        "an unregistered path did not fall back to `/`: {body}"
    );
}

/// The opening `<button …>` that carries `data-control="<action>"`.
fn button_for<'a>(body: &'a str, action: &str) -> &'a str {
    let needle = format!("data-control=\"{action}\"");
    let at = body
        .find(&needle)
        .unwrap_or_else(|| panic!("no control `{action}` in the page: {body}"));
    let start = body[..at]
        .rfind("<button")
        .expect("the control is a button");
    let end = at + body[at..].find('>').expect("the button tag closes");
    &body[start..=end]
}

/// Run one control click through the bridge the page injects, in `node`.
///
/// Returns the `(plugin, action)` the plugin's `onControl` handler received, or
/// `None` where no JS engine is installed — the repo's other JS-backed checks
/// skip the same way rather than fail a Rust-only image.
fn fire_control(plugin: &str, action: &str) -> Option<Vec<String>> {
    if !std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
    {
        eprintln!("skipping the click hop: node is not installed");
        return None;
    }

    // `{` in the literals below is doubled where `format!` must not read them.
    let script = format!(
        r#"
globalThis.window = globalThis;
globalThis.fetch = async () => ({{ ok: true, json: async () => ({{ status: "ok", result: {{}} }}) }});
{BRIDGE}
const seen = [];
window.openkite.onControl((action, plugin) => seen.push([plugin, action]));
const dispatch = {DISPATCH};
(0, eval)(dispatch);
console.log(JSON.stringify({{ seen }}));
"#,
        BRIDGE = openkite_ui::plugin_api::OPENKITE_BRIDGE_JS,
        DISPATCH = serde_json::to_string(&openkite_ui::plugin_api::control_dispatch_js(
            plugin, action
        ))
        .expect("the dispatch script is a string"),
    );

    let dir = temp_root();
    let path = dir.path().join("control-hop.mjs");
    std::fs::write(&path, script).expect("write the hop script");
    let out = std::process::Command::new("node")
        .arg(&path)
        .output()
        .expect("run node");
    assert!(
        out.status.success(),
        "node failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let answer: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|error| {
        panic!(
            "the hop printed no JSON ({error}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    let pair: Vec<String> = answer["seen"][0]
        .as_array()
        .unwrap_or_else(|| panic!("the handler never fired: {answer}"))
        .iter()
        .map(|part| part.as_str().unwrap_or_default().to_string())
        .collect();
    Some(pair)
}

/// The snapshot JSON the page embeds for the hydrating client.
fn embedded_snapshot(body: &str) -> Value {
    const OPEN: &str = "<script id=\"openkite-snapshot\" type=\"application/json\">";
    let start = body
        .find(OPEN)
        .unwrap_or_else(|| panic!("no snapshot script in the page: {body}"))
        + OPEN.len();
    let rest = &body[start..];
    let end = rest
        .find("</script>")
        .unwrap_or_else(|| panic!("unterminated snapshot script"));
    serde_json::from_str(&rest[..end]).expect("embedded snapshot is JSON")
}
