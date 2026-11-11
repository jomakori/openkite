//! The G3 parity fixture, proven at the desktop host's loader boundary.
//!
//! No window and no cluster: the desktop host loads JavaScript plugins from
//! `~/.openkite/plugins` through `plugin_js::collect_bundles`, so the fixture is
//! read off disk exactly the way that directory is read. The registrations it
//! produces are then produced the way the webview produces them — the real
//! bundle is evaluated against the real `OPENKITE_BRIDGE_JS` bootstrap in
//! `node`, and every POST it makes is fed through the host's own
//! `apply_register`, not a mock.

use std::path::{Path, PathBuf};
use std::process::Command;

use openkite::plugin_js::{collect_bundles, load_source};
use openkite_host::bridge::apply_register;
use openkite_ui::plugin_api::{RegistrationStore, OPENKITE_BRIDGE_JS};
use serde_json::Value;

/// `e2e/fixtures/plugins`, the root `e2e/fixtures/install-plugin.sh` installs
/// into and the root this test scans.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/fixtures/plugins")
}

const PLUGIN: &str = "parity-fixture";
const ROUTE: &str = "/plugins/parity-fixture";
const LABEL: &str = "Parity Fixture";

/// Evaluate `source` in `node` the way the host does: bootstrap the bridge,
/// name the plugin, run the bundle, keep every POST it makes.
///
/// `None` where no `node` is installed — the repo's other JS-backed checks skip
/// the same way rather than fail a Rust-only image.
fn registrations_via_bridge(source: &str) -> Option<Vec<Value>> {
    let entry = serde_json::to_string(source).expect("entry source is a JSON string literal");
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

#[test]
fn parity_fixture_registers_exactly_one_sidebar_item_and_one_route() {
    // 1. Loader boundary — the scan of the plugins root.
    let (bundles, errors) = collect_bundles(&fixture_root(), |_| true);
    assert!(errors.is_empty(), "fixture must load clean: {errors:?}");
    assert_eq!(
        bundles.len(),
        1,
        "one bundle under the fixtures root: {bundles:?}"
    );
    let bundle = &bundles[0];
    assert_eq!(bundle.name, PLUGIN);
    assert!(
        bundle.entry.ends_with("parity-fixture/main.js"),
        "entry is the declared main.js: {}",
        bundle.entry.display()
    );
    let style = bundle.style.as_ref().expect("manifest declares style.css");
    assert!(
        style.contains("[data-plugin=\"parity-fixture\"]")
            && style.contains(".parity-fixture-body"),
        "stylesheet is scoped to the plugin's own section: {style}"
    );

    // 2. Registration boundary — what the bundle posts through the real bridge.
    let source = load_source(bundle).expect("bundle entry reads");
    let Some(posts) = registrations_via_bridge(&source) else {
        eprintln!("skipping the registration half: no node on PATH");
        return;
    };

    let mut store = RegistrationStore::new();
    let mut kinds = Vec::new();
    for post in &posts {
        assert_eq!(
            post["plugin"], PLUGIN,
            "the host stamps the bundle's plugin name"
        );
        let request = &post["request"];
        assert_eq!(
            request["op"], "register",
            "the fixture asks the cluster for nothing: {request}"
        );
        let kind = request["kind"].as_str().expect("a register kind");
        apply_register(&mut store, PLUGIN, kind, request["payload"].clone())
            .unwrap_or_else(|err| panic!("host rejected a {kind} registration: {err}"));
        kinds.push(kind.to_string());
    }
    kinds.sort();
    assert_eq!(
        kinds,
        vec!["route", "sidebar"],
        "exactly one of each, nothing else"
    );

    // 3. The store both hosts render from.
    let registered = store.get(PLUGIN).expect("the fixture is registered");
    assert_eq!(registered.sidebar.len(), 1, "one sidebar item");
    assert_eq!(registered.sidebar[0].label, LABEL);
    assert_eq!(registered.sidebar[0].route, ROUTE);
    assert_eq!(registered.routes.len(), 1, "one route");
    assert_eq!(registered.routes[0].path, ROUTE);
    assert_eq!(registered.routes[0].title, LABEL);
    assert!(registered.status.is_empty(), "no status widget");
    assert!(registered.controls.is_empty(), "no control");
    assert!(
        registered.renderers.is_empty(),
        "no renderer: the host paints this route's chrome"
    );
    assert_eq!(store.all_sidebar_items().len(), 1);
    assert_eq!(store.all_routes().len(), 1);
    assert!(store.all_renderer_paths().is_empty());

    // 4. Capability flags: the desktop host advertises plugin routes, which is
    // what makes the fixture's route a graded page here rather than the
    // declared-unsupported panel the browser host shows.
    let capabilities = openkite_api::capability::Capabilities::in_process();
    assert!(
        capabilities.plugins,
        "the desktop host renders plugin routes"
    );
}
