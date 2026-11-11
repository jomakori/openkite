//! The G3 parity fixture, proven at the desktop host's loader boundary.
//!
//! No window and no cluster: the desktop host loads JavaScript plugins from
//! `~/.openkite/plugins` through `plugin_js::collect_bundles`, so this test
//! installs the fixture with the same script CI calls
//! (`e2e/fixtures/install-plugin.sh`), points the same scan at the result, and
//! then reproduces the registrations the webview would produce — the real
//! bundle is evaluated against the real `OPENKITE_BRIDGE_JS` bootstrap in
//! `node`, and every POST it makes is fed through the host's own
//! `apply_register`, not a mock.

use std::path::{Path, PathBuf};
use std::process::Command;

use openkite::plugin_js::{collect_bundles, load_source};
use openkite_host::bridge::apply_register;
use openkite_ui::plugin_api::{RegistrationStore, OPENKITE_BRIDGE_JS};
use serde_json::Value;

const PLUGIN: &str = "parity-fixture";
const ROUTE: &str = "/plugins/parity-fixture";
const LABEL: &str = "Parity Fixture";

/// The installer the parity job calls: `install-plugin.sh <plugins-root> [host]`.
fn installer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/fixtures/install-plugin.sh")
}

/// Install the fixture the way CI does — script, target directory, host mode —
/// and hand back the plugins root the loader scan gets pointed at.
fn install_fixture(host: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("plugins root");
    let output = Command::new("bash")
        .arg(installer())
        .arg(root.path())
        .arg(host)
        .output()
        .expect("bash runs install-plugin.sh");
    assert!(
        output.status.success(),
        "install-plugin.sh {host} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    root
}

/// What the host observes when it runs the bundle: the registration POSTs, and
/// the markup the bundle's `_renderRoute` writes when the host hands it a mount
/// node (the contract `route_views::JsRouteMount` / `JsRouteEvaluator` use).
struct Observed {
    posts: Vec<Value>,
    rendered: String,
    other_route_rendered: String,
    unmount_kind: String,
}

/// Evaluate `source` in `node` the way the host does: bootstrap the bridge,
/// name the plugin, run the bundle, keep every POST it makes.
///
/// `None` where no `node` is installed — the repo's other JS-backed checks skip
/// the same way rather than fail a Rust-only image.
fn observe_via_bridge(source: &str) -> Option<Observed> {
    let entry = serde_json::to_string(source).expect("entry source is a JSON string literal");
    let route = serde_json::to_string(ROUTE).expect("route is a JSON string literal");
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
const mount = {{ innerHTML: "" }};
const unmount = window.openkite._renderRoute({ROUTE}, mount);
const elsewhere = {{ innerHTML: "" }};
window.openkite._renderRoute("/plugins/some-other-route", elsewhere);
console.log(JSON.stringify({{
  posts,
  rendered: mount.innerHTML,
  other_route_rendered: elsewhere.innerHTML,
  unmount_kind: typeof unmount,
}}));
"#,
        PLUGIN = PLUGIN,
        BRIDGE = OPENKITE_BRIDGE_JS,
        ENTRY = entry,
        ROUTE = route,
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
    let observed: Value = serde_json::from_str(stdout.trim()).expect("node prints JSON");
    Some(Observed {
        posts: serde_json::from_value(observed["posts"].clone()).expect("posts is an array"),
        rendered: observed["rendered"]
            .as_str()
            .expect("rendered is a string")
            .to_string(),
        other_route_rendered: observed["other_route_rendered"]
            .as_str()
            .expect("other_route_rendered is a string")
            .to_string(),
        unmount_kind: observed["unmount_kind"]
            .as_str()
            .expect("unmount_kind is a string")
            .to_string(),
    })
}

#[test]
fn parity_fixture_registers_exactly_one_sidebar_item_and_one_route() {
    // 1. Install + loader boundary: the script CI calls, then the host's own
    // scan of the plugins root it installed into.
    let root = install_fixture("desktop");
    let (bundles, errors) = collect_bundles(root.path(), |_| true);
    assert!(errors.is_empty(), "fixture must load clean: {errors:?}");
    assert_eq!(
        bundles.len(),
        1,
        "one bundle under the installed root: {bundles:?}"
    );
    let bundle = &bundles[0];
    assert_eq!(bundle.name, PLUGIN);
    assert!(
        bundle.entry.ends_with("parity-fixture/main.js"),
        "entry is the installed main.js: {}",
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
    let Some(observed) = observe_via_bridge(&source) else {
        eprintln!("skipping the registration half: no node on PATH");
        return;
    };

    let mut store = RegistrationStore::new();
    let mut kinds = Vec::new();
    for post in &observed.posts {
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
    // No `route_renderer` registration: the bootstrap offers
    // `registerRouteRenderer`, but `apply_register` has no arm for the kind it
    // posts (`crates/openkite-host/src/bridge.rs`, its `other =>` arm), so a
    // bundle cannot own its route's markup yet. Both hosts therefore paint this
    // route through the shared crate — which is what keeps the fixture inside
    // the parity gate's subject.
    assert!(
        registered.renderers.is_empty(),
        "no renderer: the host paints this route's chrome"
    );
    assert_eq!(store.all_sidebar_items().len(), 1);
    assert_eq!(store.all_routes().len(), 1);
    assert!(store.all_renderer_paths().is_empty());
    assert_eq!(store.owner_of_route(ROUTE), Some(PLUGIN));

    // 4. Capability flags: the desktop host advertises plugin routes, which is
    // what makes the fixture's route a graded page here rather than the
    // declared-unsupported panel the browser host shows.
    let capabilities = openkite_api::capability::Capabilities::in_process();
    assert!(
        capabilities.plugins,
        "the desktop host renders plugin routes"
    );

    // 5. The boot filter the desktop host applies at its own call site —
    // `plugin_js::collect_bundles(&root, |name| config.is_enabled(name))` in
    // `crates/openkite-desktop/src/lib.rs` — must let the fixture through
    // without a config file, or installing it would not be enough in CI.
    assert!(
        openkite::config::OpenKiteConfig::default().is_enabled(PLUGIN),
        "a default config loads every installed plugin"
    );

    // 6. The render hook is inert by construction: the bundle paints nothing for
    // its own path (it registers no renderer, so the host never hands it a mount
    // node) and nothing for any other path either. Pinning that keeps a future
    // fixture from smuggling cluster- or host-dependent markup in.
    assert_eq!(
        observed.rendered, "",
        "the fixture paints no markup of its own: {}",
        observed.rendered
    );
    assert_eq!(observed.other_route_rendered, "");
    assert_eq!(observed.unmount_kind, "function", "the stub contract holds");
}
