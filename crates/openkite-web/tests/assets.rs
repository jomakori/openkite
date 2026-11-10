//! Cache correctness of the hydration pair (OKT-195).
//!
//! wasm-bindgen does not emit one file: the glue imports its `_bg.wasm` sibling
//! *and* a `snippets/` tree, all by relative path. The document and that whole
//! set are one unit. The host used to import
//! both from an unversioned `/openkite-web-client.js` and serve them with no
//! cache directive at all: a returning browser could pair the new HTML it just
//! fetched with the wasm it had kept, and hydrate new markup with old code —
//! a mismatch that fails deep inside wasm-bindgen rather than at the URL.
//!
//! These tests pin the two halves of the fix:
//!
//! * the document names a version — the hash of the bytes this process serves —
//!   so it can only boot the wasm from its own build, and it is `no-store` so
//!   it is never reused for a later navigation;
//! * the hashed set — the glue, the module, and the snippets the glue imports —
//!   is served `immutable` (its bytes can never change) and a version the
//!   process did not compute is a 404, so old bytes are never answered under a
//!   new name or the reverse.

mod support;

use std::sync::Arc;
use std::time::Duration;

use axum::http::{header, StatusCode};
use openkite_host::bridge::Bridge;
use openkite_web::assets::{Bundle, DOCUMENT, IMMUTABLE};
use support::{app, get_full};

const CLIENT_JS: &str = "export default function init() {}\n";
const CLIENT_WASM: &[u8] = b"\0asm\x01\0\0\0";
const SNIPPET_PATH: &str = "snippets/dioxus-interpreter-js-20451f6aefec841d/inline0.js";
const SNIPPET_JS: &str = "export function RawInterpreter() {}\n";

/// A web root carrying the set the image stages.
///
/// The snippets are not decoration: wasm-bindgen's output imports them by
/// relative path exactly as it imports the wasm, so a mount that carries only
/// the two named files serves a module graph that cannot load.
fn staged_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("openkite-web-client.js"), CLIENT_JS).expect("write glue");
    std::fs::write(root.path().join("openkite-web-client_bg.wasm"), CLIENT_WASM)
        .expect("write wasm");
    let snippet = root.path().join(SNIPPET_PATH);
    std::fs::create_dir_all(snippet.parent().expect("snippet dir")).expect("mkdir snippets");
    std::fs::write(&snippet, SNIPPET_JS).expect("write snippet");
    root
}

fn host(root: &std::path::Path) -> axum::Router {
    app(Arc::new(Bridge::new()), root)
}

/// The client URL the document imports.
fn client_import(page: &str) -> String {
    const OPEN: &str = "import init from \"";
    let start = page
        .find(OPEN)
        .unwrap_or_else(|| panic!("the document imports no client bundle: {page}"))
        + OPEN.len();
    let rest = &page[start..];
    rest[..rest.find('"').expect("the import is a quoted URL")].to_string()
}

fn header(response: &axum::http::HeaderMap, name: header::HeaderName) -> Option<&str> {
    response.get(name).and_then(|value| value.to_str().ok())
}

/// One GET, with the body as text: these tests read documents, not bytes.
async fn fetch(app: axum::Router, path: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let (status, headers, body) = get_full(app, path).await;
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_document_is_never_reused_and_names_the_versioned_bundle() {
    let root = staged_root();
    let app = host(root.path());

    let (status, headers, body) = fetch(app.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header(&headers, header::CACHE_CONTROL),
        Some(DOCUMENT),
        "the document names the current build, so it must not be stored: a \
         cached document is one that can be paired with another build's wasm"
    );

    let url = client_import(&body);
    assert!(
        url.starts_with("/assets/bundle/"),
        "the document imports the bundle unversioned ({url}): {body}"
    );
    assert!(
        !body.contains("import init from \"/openkite-web-client.js\";"),
        "the unversioned URL is cacheable by any intermediary and names no \
         build, which is the mismatch OKT-195 fixes: {body}"
    );

    // The version in the URL is the hash of this root's bytes, not a label.
    let bundle = Bundle::load(root.path());
    assert_eq!(
        url,
        bundle.client_url(),
        "the document must name the pair this process serves"
    );
}

#[tokio::test]
async fn the_versioned_pair_is_served_immutable_and_byte_for_byte() {
    let root = staged_root();
    let app = host(root.path());
    let (_, _, body) = fetch(app.clone(), "/").await;
    let client_url = client_import(&body);
    let wasm_url = client_url.replace("openkite-web-client.js", "openkite-web-client_bg.wasm");

    let (status, headers, served) = fetch(app.clone(), &client_url).await;
    assert_eq!(status, StatusCode::OK, "the imported glue is not served");
    assert_eq!(served, CLIENT_JS, "the glue must round-trip byte for byte");
    assert_eq!(
        header(&headers, header::CACHE_CONTROL),
        Some(IMMUTABLE),
        "a hashed URL names bytes that cannot change, so it may be kept"
    );
    assert_eq!(
        header(&headers, header::CONTENT_TYPE),
        Some("text/javascript; charset=utf-8"),
        "the glue is an ES module; a browser refuses it under the wrong type"
    );

    let (status, headers, served) = fetch(app.clone(), &wasm_url).await;
    assert_eq!(status, StatusCode::OK, "the imported module is not served");
    assert_eq!(
        served.as_bytes(),
        CLIENT_WASM,
        "the module must be the same bytes"
    );
    assert_eq!(header(&headers, header::CACHE_CONTROL), Some(IMMUTABLE));
    assert_eq!(
        header(&headers, header::CONTENT_TYPE),
        Some("application/wasm"),
        "the module only compiles as application/wasm"
    );
}

#[tokio::test]
async fn a_version_this_build_did_not_compute_is_not_served() {
    let root = staged_root();
    let app = host(root.path());

    for stale in ["deadbeefdeadbeef", "0000000000000000", "0123456789abcdef"] {
        for file in ["openkite-web-client.js", "openkite-web-client_bg.wasm"] {
            let (status, _, _) =
                fetch(app.clone(), &format!("/assets/bundle/{stale}/{file}")).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "{file} under version {stale} must not be answered with this build's bytes — \
                 a browser that kept that URL would hydrate with the wrong module"
            );
        }
    }
}

#[tokio::test]
async fn the_glue_relative_imports_land_under_the_same_version() {
    // This is the half that makes the URL a fix rather than a label: the glue
    // resolves `./snippets/...` against its own URL, so the snippet must answer
    // under the same version or the module graph 404s and hydration dies.
    let root = staged_root();
    let app = host(root.path());
    let (_, _, body) = fetch(app.clone(), "/").await;
    let client_url = client_import(&body);
    let version = Bundle::load(root.path()).version().to_string();

    let snippet_url = client_url.replace("openkite-web-client.js", SNIPPET_PATH);
    assert!(snippet_url.contains(&version));

    let (status, headers, served) = fetch(app.clone(), &snippet_url).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the glue imports {SNIPPET_PATH} relatively; it must be staged under the \
         version the document names or hydration fails: {served}"
    );
    assert_eq!(
        served, SNIPPET_JS,
        "the snippet must round-trip byte for byte"
    );
    assert_eq!(header(&headers, header::CACHE_CONTROL), Some(IMMUTABLE));
    assert_eq!(
        header(&headers, header::CONTENT_TYPE),
        Some("text/javascript; charset=utf-8"),
        "a snippet is an ES module too"
    );
}

#[tokio::test]
async fn only_the_staged_set_exists_under_the_mount() {
    let root = staged_root();
    // A file next to the set is in the bundle root — but not part of it: the
    // mount answers the set, so nothing else can be smuggled in by path.
    std::fs::write(root.path().join("secret.txt"), "not part of the bundle").expect("write");
    let app = host(root.path());
    let version = Bundle::load(root.path()).version().to_string();

    let (status, _, _) = fetch(app.clone(), &format!("/assets/bundle/{version}/secret.txt")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the mount answers the set, not the directory"
    );

    // Traversal is refused on the name rather than normalised into a hit, and a
    // `..` cannot name a sibling file however it is spelled.
    for traversal in ["../secret.txt", "..%2fsecret.txt", "%2e%2e/secret.txt"] {
        let (status, _, body) = fetch(
            app.clone(),
            &format!("/assets/bundle/{version}/{traversal}"),
        )
        .await;
        assert!(
            !body.contains("not part of the bundle"),
            "the mount leaked a file next to the bundle via {traversal}"
        );
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{traversal} is not a file of the bundle"
        );
    }
}

#[tokio::test]
async fn a_rebuilt_wasm_changes_the_url_the_document_names() {
    // The failure being fixed is a document from one build booting another
    // build's wasm. Two builds must therefore name different URLs — the URL is
    // the only thing that carries the pairing into the browser.
    let first = staged_root();
    let second = staged_root();
    std::fs::write(
        second.path().join("openkite-web-client_bg.wasm"),
        b"\0asm\x01\0\0\0\0",
    )
    .expect("write the rebuilt module");

    let (_, _, page_one) = fetch(host(first.path()), "/").await;
    let (_, _, page_two) = fetch(host(second.path()), "/").await;

    let one = client_import(&page_one);
    let two = client_import(&page_two);
    assert_ne!(
        one, two,
        "a rebuilt wasm must move the document's import URL, or a cached copy \
         of the old URL boots the new page with the old module"
    );

    // And the rebuilt host does not answer the old build's URL.
    let app = host(second.path());
    let (status, _, _) = fetch(app, &one).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A snippet is part of the same build: changing one moves the version too,
    // so a browser cannot pair a new glue with a snippet it kept.
    let third = staged_root();
    std::fs::write(
        third.path().join(SNIPPET_PATH),
        "export function RawInterpreter() { /* rebuilt */ }\n",
    )
    .expect("rewrite the snippet");
    let (_, _, page_three) = fetch(host(third.path()), "/").await;
    assert_ne!(
        two,
        client_import(&page_three),
        "a rebuilt snippet is a new build; the URL must move with it"
    );
}

#[tokio::test]
async fn an_empty_web_root_still_renders_a_document_that_names_a_bundle() {
    // The host boots before the bundle is staged in local runs; the page must
    // still render (server-rendered console), and its import must still be
    // versioned so it cannot poison a cache once the bundle lands.
    let root = tempfile::tempdir().expect("tempdir");
    let (status, headers, body) = fetch(host(root.path()), "/").await;
    assert_eq!(status, StatusCode::OK, "the console must still render");
    assert_eq!(header(&headers, header::CACHE_CONTROL), Some(DOCUMENT));
    assert!(client_import(&body).starts_with("/assets/bundle/"));
}

#[tokio::test]
async fn the_version_is_the_staged_bytes_not_the_clock() {
    // A version that moved per boot would throw away every cached asset on
    // every restart, and one that never moved would keep the bug. It is the
    // content, so the same bytes must give the same version.
    let root = staged_root();
    let a = Bundle::load(root.path());
    std::thread::sleep(Duration::from_millis(20));
    let b = Bundle::load(root.path());
    assert_eq!(a.version(), b.version(), "same bytes, same version");
    assert_eq!(a.version().len(), 16, "a short, URL-safe version");
    assert!(a.version().chars().all(|c| c.is_ascii_hexdigit()));
}
