//! SSR + hydration assertions for the crate-rendered console.

use base64::Engine as _;
use openkite_web::ssr::{hydration_data, render_body, render_page, RenderOptions, Snapshot};

fn snapshot() -> Snapshot {
    Snapshot::default()
}

#[test]
fn render_body_includes_the_data_surface_markers() {
    let body = render_body(&snapshot());
    assert!(
        body.contains("data-surface=\"app\""),
        "ssr body missing app surface: {body}"
    );
    assert!(
        body.contains("data-surface=\"overview\""),
        "ssr body missing overview surface: {body}"
    );
    assert!(
        body.contains("data-surface=\"capabilities\""),
        "ssr body missing capabilities surface: {body}"
    );
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
    let first = render_body(&snapshot());
    let second = render_body(&snapshot());
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
