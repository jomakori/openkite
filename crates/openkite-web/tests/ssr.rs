//! SSR + hydration assertions for the crate-rendered console.
//!
//! The page is the *shared* console: these tests pin the shell chrome the
//! shared model produces, the surfaces the snapshot fills, and the hydration
//! wiring the client attaches to. The class contract with the stylesheet lives
//! in `tests/css_contract.rs`.

use std::sync::Arc;

use base64::Engine as _;
use openkite_api::capability::Capabilities;
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::SecretObject;
use openkite_web::ssr::{
    hydration_data, namespaces_of, render_body, render_page, RenderOptions, SecretRef, Snapshot,
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
        ("Cluster", "/cluster"),
        ("Workloads", "/workloads"),
        ("Config", "/config"),
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
        secrets: Vec::new(),
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
