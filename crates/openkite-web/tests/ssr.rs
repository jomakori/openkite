//! SSR + hydration assertions for the crate-rendered console.
//!
//! The page is the *shared* console: these tests pin the shell chrome the
//! shared model produces, the surfaces the snapshot fills, and the hydration
//! wiring the client attaches to. The class contract with the stylesheet lives
//! in `tests/css_contract.rs`.

use std::sync::Arc;

use base64::Engine as _;
use k8s_openapi::api::core::v1::Pod;
use openkite_api::capability::Capabilities;
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::SecretObject;
use openkite_ui::components::resource_table::ResourceRow;
use openkite_web::ssr::{
    hydration_data, namespaces_of, render_body, render_page, RenderOptions, SecretRef, Snapshot,
    WorkloadsTable,
};

fn snapshot() -> Snapshot {
    Snapshot::default()
}

/// A connected host carrying data, so the surfaces render with rows in them
/// (capability flags on, a cluster context, one secret in scope).
fn connected() -> Snapshot {
    Snapshot {
        capabilities: Capabilities::in_process(),
        connected: true,
        context: Some("in-cluster".into()),
        secrets: vec![SecretRef {
            namespace: "default".into(),
            name: "regcred".into(),
        }],
        ..Snapshot::default()
    }
}

#[test]
fn render_body_includes_the_data_surface_markers() {
    for surface in ["app", "overview", "capabilities"] {
        let needle = format!("data-surface=\"{surface}\"");
        let body = render_body(&snapshot());
        assert!(body.contains(&needle), "ssr body missing {surface}: {body}");
    }
}

#[test]
fn render_body_renders_the_shared_shell_chrome() {
    let body = render_body(&connected());
    for chrome in [
        "class=\"app\"",
        "class=\"sidebar\"",
        "class=\"brand\"",
        "class=\"brand-mark\"",
        "class=\"brand-word\"",
        "class=\"nav\"",
        "class=\"nav-section\"",
        "class=\"nav-title\"",
        "class=\"nav-item\"",
        "class=\"topbar\"",
        "class=\"breadcrumbs\"",
        "class=\"topbar-actions\"",
        "class=\"main\"",
        "class=\"view active\"",
        "class=\"sidebar-footer\"",
        "class=\"status-line\"",
    ] {
        assert!(
            body.contains(chrome),
            "ssr body missing shell chrome {chrome}: {body}"
        );
    }
}

#[test]
fn render_body_renders_the_shared_sidebar_model() {
    let body = render_body(&snapshot());
    for (label, route) in [
        ("Nodes", "/cluster"),
        ("Pods", "/workloads"),
        ("ConfigMaps", "/config"),
    ] {
        assert!(
            body.contains(&format!(">{label}<")),
            "nav label {label}: {body}"
        );
        assert!(
            body.contains(&format!("href=\"{route}\"")),
            "nav route {route}: {body}"
        );
    }
    // The browser console has no live cluster in this snapshot: every count is
    // absent, and the disconnected shape renders no badge at all.
    assert!(
        !body.contains("class=\"nav-badge\""),
        "a disconnected console draws no counts: {body}"
    );
}

#[test]
fn render_body_paints_a_connected_snapshot() {
    let body = render_body(&connected());
    assert!(
        body.contains("in-cluster · Connected"),
        "status footer: {body}"
    );
    assert!(body.contains("class=\"pill success\""), "pill: {body}");
    assert!(body.contains(">Running<"), "pill label: {body}");
    assert!(body.contains(">in-process<"), "gateway kind: {body}");
    assert!(body.contains(">regcred<"), "secret row: {body}");
    assert!(
        body.contains("class=\"resource-name\""),
        "secret row markup: {body}"
    );
    let server_side = render_body(&Snapshot {
        capabilities: Capabilities::server_side(),
        connected: true,
        context: Some("kubeconfig".into()),
        ..Snapshot::default()
    });
    assert!(
        server_side.contains(">server-side<"),
        "gateway kind: {server_side}"
    );
    assert!(
        server_side.contains(">off<"),
        "capability flags: {server_side}"
    );
    assert!(
        server_side.contains("kubeconfig · Connected"),
        "footer: {server_side}"
    );
}

#[test]
fn render_body_paints_a_disconnected_snapshot() {
    let body = render_body(&snapshot());
    assert!(body.contains("no cluster · Disconnected"), "footer: {body}");
    assert!(body.contains("class=\"pill danger\""), "pill: {body}");
    assert!(body.contains(">Failed<"), "pill label: {body}");
    assert!(
        body.contains("No secrets in the gateway"),
        "empty secrets surface: {body}"
    );
}

#[test]
fn render_body_carries_no_bespoke_console_markup() {
    for body in [render_body(&connected()), render_body(&snapshot())] {
        for gone in [
            "class=\"surface\"",
            "status-ok",
            "status-warn",
            "status-badge",
        ] {
            assert!(
                !body.contains(gone),
                "bespoke markup {gone} still rendered: {body}"
            );
        }
    }
}

#[test]
fn render_body_includes_a_hydration_marker() {
    let body = render_body(&snapshot());
    assert!(
        body.contains("data-node-hydration"),
        "ssr body missing hydration marker: {body}"
    );
}

#[test]
fn render_body_is_byte_identical_across_runs() {
    let first = render_body(&connected());
    let second = render_body(&connected());
    assert_eq!(first, second, "ssr body should be deterministic");
}

#[test]
fn hydration_data_is_well_formed_base64() {
    let raw = hydration_data();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&raw)
        .expect("hydration data must be valid base64");
    let empty: Vec<Option<Vec<u8>>> =
        ciborium::from_reader(bytes.as_slice()).expect("hydration data must round-trip as CBOR");
    assert!(empty.is_empty(), "empty hydration payload expected");
}

#[test]
fn ssr_only_page_omits_the_wasm_client_bootstrap() {
    let page = render_page(&snapshot(), &RenderOptions::ssr_only());
    assert!(page.contains("<!doctype html>"), "page: {page}");
    assert!(
        page.contains("data-surface=\"app\""),
        "page missing the app surface: {page}"
    );
    assert!(
        page.contains("window.hydrate_queue"),
        "ssr-only page must not expose the hydrate glue: {page}"
    );
    assert!(
        !page.contains("initial_dioxus_hydration_data"),
        "ssr-only page must not embed the wasm hydration payload: {page}"
    );
}

#[test]
fn hydrating_page_embeds_the_snapshot_and_client_import() {
    let page = render_page(&snapshot(), &RenderOptions::hydrating());
    assert!(page.contains("<!doctype html>"), "page: {page}");
    assert!(
        page.contains("initial_dioxus_hydration_data="),
        "hydrating page must embed the wasm hydration payload: {page}"
    );
    assert!(
        page.contains("id=\"openkite-snapshot\""),
        "hydrating page must embed the snapshot JSON: {page}"
    );
    assert!(
        page.contains("/openkite-web-client.js"),
        "hydrating page must point at the wasm client bundle: {page}"
    );
}

#[test]
fn every_page_declares_the_vendored_typefaces() {
    for options in [RenderOptions::ssr_only(), RenderOptions::hydrating()] {
        let page = render_page(&snapshot(), &options);
        for face in openkite_ui::assets::FACES {
            assert!(
                page.contains(&openkite_ui::assets::src_url(face)),
                "page does not serve {}: {page}",
                face.file
            );
            assert!(
                page.contains(&format!("font-family: \"{}\";", face.family)),
                "page does not declare {}: {page}",
                face.family
            );
        }
        for external in ["fonts.googleapis.com", "fonts.gstatic.com"] {
            assert!(
                !page.contains(external),
                "page still asks {external} for type"
            );
        }
    }
}

// --- The route the host resolved (OKT-180) ---------------------------------

/// A snapshot addressed at a route, with no cluster data of its own.
fn route_snapshot(route: &str) -> Snapshot {
    Snapshot {
        route: route.to_string(),
        ..Snapshot::default()
    }
}

/// The snapshot JSON the page embeds for the client to boot from.
fn embedded_snapshot(page: &str) -> &str {
    let (_, rest) = page
        .split_once("id=\"openkite-snapshot\"")
        .expect("the page embeds the snapshot");
    let (_, json) = rest
        .split_once('>')
        .expect("the snapshot script tag closes");
    json.split_once("</script>")
        .expect("the snapshot script closes")
        .0
}

/// One pod as the API returns it, in the console's own row mapping.
fn web_pod_row() -> ResourceRow {
    let pod: Pod = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {"name": "web-1", "namespace": "default", "resourceVersion": "1"},
        "spec": {"containers": [{"name": "app", "image": "nginx"}]},
        "status": {"phase": "Running"},
    }))
    .expect("a pod");
    openkite::workloads::pod_row(&pod)
}

/// The workloads route's snapshot: the console's columns over one pod row.
fn workloads_snapshot() -> Snapshot {
    Snapshot {
        connected: true,
        context: Some("in-cluster".into()),
        route: "/workloads".into(),
        workloads: WorkloadsTable {
            columns: openkite::workloads::pod_columns(),
            rows: vec![web_pod_row()],
            error: None,
        },
        ..Snapshot::default()
    }
}

#[test]
fn the_route_the_host_served_reaches_the_rendered_page() {
    for (route, page) in [
        ("/", "Overview"),
        ("/cluster", "Nodes"),
        ("/workloads", "Pods"),
        ("/config", "ConfigMaps"),
    ] {
        let body = render_body(&route_snapshot(route));
        assert!(
            body.contains(&format!("data-route=\"{route}\"")),
            "{route} must reach the route chrome: {body}"
        );
        assert!(
            body.contains(&format!("data-page=\"{page}\"")),
            "{route} must be titled {page}: {body}"
        );
    }
}

#[test]
fn the_current_route_marks_its_own_sidebar_entry() {
    let body = render_body(&route_snapshot("/workloads"));
    assert_eq!(
        body.matches("class=\"nav-item active\"").count(),
        1,
        "exactly one entry is current: {body}"
    );
    let active = body
        .find("class=\"nav-item active\"")
        .expect("an active entry");
    assert!(
        body[active..].contains(">Pods<"),
        "the current route's own entry is the active one: {body}"
    );
    // The home route is a sidebar entry labelled Overview.
    let home = render_body(&route_snapshot("/"));
    assert_eq!(
        home.matches("class=\"nav-item active\"").count(),
        1,
        "exactly one entry is current on the overview route: {home}"
    );
    let active = home
        .find("class=\"nav-item active\"")
        .expect("an active overview entry");
    assert!(
        home[active..].contains(">Overview<"),
        "the overview route marks Overview active: {home}"
    );
}

#[test]
fn the_workloads_route_paints_the_consoles_own_table() {
    let body = render_body(&workloads_snapshot());
    for marker in [
        "data-surface=\"workloads\"",
        "class=\"resource-table\"",
        "class=\"table-header table-row\"",
        "data-label=\"Name\"",
        ">web-1<",
    ] {
        assert!(
            body.contains(marker),
            "the workloads route must render {marker}: {body}"
        );
    }
    for label in ["Name", "Health", "Ready", "Restarts", "Age", "Status"] {
        assert!(
            body.contains(&format!(">{label}<")),
            "the table's own columns include {label}: {body}"
        );
    }
}

#[test]
fn a_refused_gateway_is_stated_not_left_blank() {
    let snapshot = Snapshot {
        connected: true,
        route: "/workloads".into(),
        workloads: WorkloadsTable {
            columns: openkite::workloads::pod_columns(),
            rows: Vec::new(),
            error: Some("no cluster connected".into()),
        },
        ..Snapshot::default()
    };
    let body = render_body(&snapshot);
    assert!(
        body.contains("class=\"table-state table-error\""),
        "the route declares the refusal: {body}"
    );
    assert!(
        body.contains("no cluster connected"),
        "the gateway's own words: {body}"
    );
    assert!(
        !body.contains("class=\"resource-table\""),
        "a refused gateway paints no blank table: {body}"
    );
}

#[test]
fn the_workloads_route_is_byte_identical_across_runs() {
    assert_eq!(
        render_body(&workloads_snapshot()),
        render_body(&workloads_snapshot()),
        "the workloads route should be deterministic"
    );
}

#[test]
fn the_workloads_snapshot_still_hydrates() {
    let snapshot = workloads_snapshot();
    let json = serde_json::to_string(&snapshot).expect("serialize the snapshot");
    let parsed: Snapshot = serde_json::from_str(&json).expect("the client boots from this JSON");
    assert_eq!(parsed, snapshot, "the snapshot must round-trip");

    let page = render_page(&snapshot, &RenderOptions::hydrating());
    let client: Snapshot =
        serde_json::from_str(embedded_snapshot(&page)).expect("the embedded JSON parses");
    assert_eq!(client.route, "/workloads");
    assert_eq!(client.workloads.rows.len(), 1);
    assert_eq!(client.workloads.rows[0].id, "default/web-1");
}

// --- Secret inventory from the gateway (OKT-179) ----------------------------

/// A gateway whose list op answers with the refs under test; `None` makes the
/// list fail, so the disconnected fallback is exercised too.
struct RefGateway(Option<Vec<SecretRef>>);

impl Gateway for RefGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }

    fn apply(&self, _: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async { Ok(()) })
    }

    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        Box::pin(async move {
            Ok(SecretObject {
                name,
                namespace: Some(namespace),
                ..SecretObject::default()
            })
        })
    }

    fn secret_refs(&self) -> GatewayFuture<'_, Result<Vec<SecretRef>, GatewayError>> {
        let outcome = match &self.0 {
            Some(refs) => Ok(refs.clone()),
            None => Err(GatewayError::new("no cluster")),
        };
        Box::pin(async move { outcome })
    }
}

fn ref_gateway(refs: Option<Vec<SecretRef>>) -> Arc<dyn Gateway> {
    Arc::new(RefGateway(refs))
}

#[tokio::test]
async fn snapshot_lists_the_secrets_the_gateway_returns() {
    let gateway = ref_gateway(Some(vec![
        SecretRef {
            namespace: "default".into(),
            name: "regcred".into(),
        },
        SecretRef {
            namespace: "team-a".into(),
            name: "db".into(),
        },
    ]));
    let snapshot = Snapshot::from_gateway(&gateway, true, Some("in-cluster".into())).await;
    assert_eq!(snapshot.secrets.len(), 2);
    assert_eq!(namespaces_of(&snapshot.secrets), vec!["default", "team-a"]);

    // The page paints the rows and the namespace bar the same list fed.
    let body = render_body(&snapshot);
    assert!(body.contains(">regcred<"), "secret row: {body}");
    assert!(
        body.contains("data-ns=\"team-a\""),
        "namespace chip: {body}"
    );
    assert!(
        !body.contains("data-empty=\"secrets\""),
        "secrets surface must not paint empty: {body}"
    );
    assert!(
        !body.contains("data-empty=\"namespaces\""),
        "namespace bar must not paint empty: {body}"
    );
}

#[tokio::test]
async fn snapshot_keeps_the_declared_empty_state_when_the_gateway_lists_nothing() {
    let gateway = ref_gateway(Some(Vec::new()));
    let snapshot = Snapshot::from_gateway(&gateway, true, Some("in-cluster".into())).await;
    assert!(snapshot.secrets.is_empty());

    let body = render_body(&snapshot);
    assert!(
        body.contains("data-empty=\"secrets\""),
        "empty secrets surface: {body}"
    );
    assert!(
        body.contains("data-empty=\"namespaces\""),
        "declared empty namespace bar: {body}"
    );
}

#[tokio::test]
async fn snapshot_keeps_the_declared_empty_state_when_the_gateway_cannot_list() {
    let gateway = ref_gateway(None);
    let snapshot = Snapshot::from_gateway(&gateway, false, None).await;
    assert!(snapshot.secrets.is_empty());
    assert!(!snapshot.connected);

    let body = render_body(&snapshot);
    assert!(
        body.contains("data-empty=\"secrets\""),
        "disconnected page must keep the empty secrets surface: {body}"
    );
    assert!(
        body.contains("data-empty=\"namespaces\""),
        "disconnected page must keep the declared namespace bar: {body}"
    );
}

#[test]
fn namespaces_are_the_sorted_distinct_set_of_the_refs() {
    let refs = vec![
        SecretRef {
            namespace: "team-b".into(),
            name: "one".into(),
        },
        SecretRef {
            namespace: "default".into(),
            name: "two".into(),
        },
        SecretRef {
            namespace: "team-b".into(),
            name: "three".into(),
        },
    ];
    assert_eq!(namespaces_of(&refs), vec!["default", "team-b"]);
    assert!(namespaces_of(&[]).is_empty());
}

#[test]
fn every_page_carries_the_shell_stylesheet() {
    let wrapped = format!("<style>{}</style>", openkite_ui::MAIN_CSS);
    for options in [RenderOptions::ssr_only(), RenderOptions::hydrating()] {
        let page = render_page(&snapshot(), &options);
        assert!(
            page.contains(&wrapped),
            "page must inline the shared shell stylesheet"
        );
    }
}

#[test]
fn rendered_page_defines_the_openkite_bridge() {
    for options in [RenderOptions::ssr_only(), RenderOptions::hydrating()] {
        let page = render_page(&snapshot(), &options);
        assert!(
            page.contains("window.openkite"),
            "the page must define the bridge the console's JS calls: {page}"
        );
        assert!(
            page.contains("_pushState"),
            "the page must route pushed updates: {page}"
        );
    }
}

struct RefGateway(Option<Vec<SecretRef>>);

impl Gateway for RefGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }
    fn apply(&self, _: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async { Ok(()) })
    }
    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        Box::pin(async move {
            Ok(SecretObject {
                name,
                namespace: Some(namespace),
                ..SecretObject::default()
            })
        })
    }
    fn secret_refs(&self) -> GatewayFuture<'_, Result<Vec<SecretRef>, GatewayError>> {
        match &self.0 {
            Some(refs) => {
                let refs = refs.clone();
                Box::pin(async move { Ok(refs) })
            }
            None => Box::pin(async { Err(GatewayError::new("no cluster")) }),
        }
    }
}

fn ref_gateway(refs: Option<Vec<SecretRef>>) -> Arc<dyn Gateway> {
    Arc::new(RefGateway(refs))
}

#[tokio::test]
async fn snapshot_lists_gateway_secret_refs_and_preserves_empty_state() {
    let gateway = ref_gateway(Some(vec![
        SecretRef {
            namespace: "default".into(),
            name: "regcred".into(),
        },
        SecretRef {
            namespace: "team-a".into(),
            name: "db".into(),
        },
    ]));
    let snapshot = Snapshot::from_gateway(&gateway, true, Some("in-cluster".into())).await;
    assert_eq!(snapshot.secrets.len(), 2);
    assert_eq!(namespaces_of(&snapshot.secrets), vec!["default", "team-a"]);
    let body = render_body(&snapshot);
    assert!(body.contains(">regcred<"), "secret row: {body}");
    assert!(
        body.contains("data-ns=\"team-a\""),
        "namespace chip: {body}"
    );
    assert!(
        !body.contains("data-empty=\"secrets\""),
        "secrets surface: {body}"
    );
    assert!(
        !body.contains("data-empty=\"namespaces\""),
        "namespace surface: {body}"
    );

    let empty = Snapshot::from_gateway(&ref_gateway(Some(Vec::new())), true, None).await;
    assert!(empty.secrets.is_empty());
    let body = render_body(&empty);
    assert!(
        body.contains("data-empty=\"secrets\""),
        "empty secrets: {body}"
    );
    assert!(
        body.contains("data-empty=\"namespaces\""),
        "empty namespaces: {body}"
    );

    let disconnected = Snapshot::from_gateway(&ref_gateway(None), false, None).await;
    assert!(disconnected.secrets.is_empty());
    assert!(render_body(&disconnected).contains("data-empty=\"secrets\""));
}

#[test]
fn namespace_refs_are_sorted_and_deduplicated() {
    let refs = vec![
        SecretRef {
            namespace: "team-b".into(),
            name: "one".into(),
        },
        SecretRef {
            namespace: "default".into(),
            name: "two".into(),
        },
        SecretRef {
            namespace: "team-b".into(),
            name: "three".into(),
        },
    ];
    assert_eq!(namespaces_of(&refs), vec!["default", "team-b"]);
    assert!(namespaces_of(&[]).is_empty());
}
