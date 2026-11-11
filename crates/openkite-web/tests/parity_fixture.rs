//! The G3 parity fixture through the browser host's registration boundary.
//!
//! The web host does not load plugin bundles on `main`: it publishes
//! `Capabilities::server_side()` (`plugins: false`, `host.rs:113`) and nothing
//! under `crates/openkite-web` reads a plugins directory. What it does have is
//! the other half of the same loader — `POST /openkite` `register` envelopes,
//! merged into the bridge's `RegistrationStore` and painted by SSR.
//!
//! So this test carries the fixture as far as the browser host goes today: it
//! evaluates the real bundle against the real `OPENKITE_BRIDGE_JS` in `node`,
//! posts the exact envelopes the bundle produced, and pins what a capture of
//! this route would actually see — the fixture's nav entry, and the
//! declared-unsupported panel the host shows for a plugin route it cannot
//! render. That second assertion is the known gap: it flips to a real plugin
//! page once the web host grows a bundle loader.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use axum::http::StatusCode;
use openkite_host::bridge::Bridge;
use openkite_ui::plugin_api::OPENKITE_BRIDGE_JS;
use serde_json::Value;
use support::{app, get_body, post_json};

const PLUGIN: &str = "parity-fixture";
const ROUTE: &str = "/plugins/parity-fixture";
const LABEL: &str = "Parity Fixture";

/// The installer the parity job calls: `install-plugin.sh <plugins-root> [host]`.
fn installer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/fixtures/install-plugin.sh")
}

/// Install the fixture the way the job would for this host and read the entry it
/// staged. The browser host has no bundle loader, so what this test proves is the
/// registration path — but it feeds that path the *installed* copy, not a second
/// read of the fixture source tree.
fn installed_entry() -> String {
    let root = tempfile::tempdir().expect("plugins root");
    let output = Command::new("bash")
        .arg(installer())
        .arg(root.path())
        .arg("web")
        .output()
        .expect("bash runs install-plugin.sh");
    assert!(
        output.status.success(),
        "install-plugin.sh web failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(root.path().join(PLUGIN).join("main.js"))
        .expect("the installed entry reads")
}

/// Every POST the fixture's bundle makes, produced by the real bootstrap in
/// `node`. `None` where no `node` is installed, matching the repo's other
/// JS-backed checks.
fn registrations_via_bridge() -> Option<Vec<Value>> {
    let entry = installed_entry();
    let entry = serde_json::to_string(&entry).expect("entry source is a JSON string literal");
    let script = format!(
        r#"
globalThis.window = globalThis;
globalThis.__openkite_plugin = "{PLUGIN}";
const posts = [];
globalThis.fetch = async (url, init) => {{
  if (url !== "/openkite") throw new Error(`unexpected url ${{url}}`);
  posts.push(JSON.parse(init.body));
  return {{ ok: true, json: async () => ({{ status: "ok", result: {{}} }}) }};
}};
{BRIDGE}
(0, eval)({ENTRY});
await new Promise((resolve) => setTimeout(resolve, 50));
console.log(JSON.stringify(posts));
"#,
        PLUGIN = PLUGIN,
        BRIDGE = OPENKITE_BRIDGE_JS,
        ENTRY = entry,
    );
    let output = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    assert!(
        output.status.success(),
        "node failed to evaluate the bundle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("node stdout is UTF-8");
    Some(serde_json::from_str(stdout.trim()).expect("node prints the posts as JSON"))
}

/// The fixture's nav entry paints, and its route paints the panel the browser
/// host shows for a plugin route it advertises no surface for.
#[tokio::test]
async fn the_fixture_registers_one_nav_entry_and_one_route_on_the_web_host() {
    let Some(posts) = registrations_via_bridge() else {
        eprintln!("skipping: no node on PATH");
        return;
    };
    assert_eq!(posts.len(), 2, "one sidebar item and one route: {posts:?}");

    let bridge = Arc::new(Bridge::new());
    let root = tempfile::tempdir().expect("tempdir");
    let app = app(bridge.clone(), root.path());

    for post in &posts {
        let (status, envelope) = post_json(app.clone(), "/openkite", post.clone()).await;
        assert_eq!(status, StatusCode::OK, "POST /openkite: {envelope}");
        assert_eq!(
            envelope["status"], "ok",
            "bridge rejected {post}: {envelope}"
        );
    }

    // The store the shell is built from holds exactly the fixture's two
    // contributions — one sidebar item, one route, nothing else.
    let store = bridge.snapshot();
    assert_eq!(
        store.len(),
        1,
        "one plugin registered: {:?}",
        store.plugins()
    );
    assert_eq!(store.all_sidebar_items().len(), 1, "one sidebar item");
    assert_eq!(store.all_routes().len(), 1, "one route");
    assert!(store.all_status_items().is_empty(), "no status widget");
    assert!(store.all_renderer_paths().is_empty(), "no renderer");
    assert!(store.controls_for_route(ROUTE).is_empty(), "no control");
    assert_eq!(store.owner_of_route(ROUTE), Some(PLUGIN));
    let registered = store.get(PLUGIN).expect("the fixture is registered");
    assert_eq!(registered.sidebar[0].label, LABEL);
    assert_eq!(registered.sidebar[0].route, ROUTE);
    assert_eq!(registered.routes[0].path, ROUTE);
    assert_eq!(registered.routes[0].title, LABEL);

    // The nav entry the shell paints from that store.
    let (status, home) = get_body(app.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        home.contains("href=\"/plugins/parity-fixture\""),
        "the fixture's nav entry is not a link to its route: {home}"
    );
    assert!(
        home.contains(LABEL),
        "the fixture's label is not painted: {home}"
    );

    // ...and the route itself. The browser host advertises no plugin surface
    // (`Capabilities::server_side()` → plugins: false), so the route renders
    // the declared-unsupported panel: the capability chip is present and
    // marked unsatisfied, and the body says `plugin-route`. That second half is
    // the known gap — it becomes the plugin's own page once the web host loads
    // bundles.
    let (status, page) = get_body(app.clone(), ROUTE).await;
    assert_eq!(status, StatusCode::OK, "the plugin route is servable");
    assert!(
        page.contains(LABEL),
        "the route head does not carry the fixture's title: {page}"
    );
    assert!(
        page.contains("data-capability=\"plugin-route\""),
        "the route does not declare its plugin-route capability: {page}"
    );
    assert!(
        page.contains("data-unsupported=\"plugin-route\""),
        "expected the declared-unsupported plugin-route panel until the web host loads bundles: {page}"
    );
}
