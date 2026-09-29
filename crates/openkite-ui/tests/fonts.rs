//! Vendored-typeface contract.
//!
//! The stylesheet, the face table and the files on disk have to agree: an
//! `@font-face` naming a file nobody vendored is a 404 in the browser and an
//! unreachable URL in the webview, and a vendored file no rule names is dead
//! weight in the binary. Both halves are asserted here, plus the embedded
//! rendering the desktop host inlines.

use base64::Engine as _;
use openkite_ui::assets::{
    data_uri, embedded_css, face_by_file, payload_bytes, src_url, FACES, FONT_ROUTE,
};

/// The shipped stylesheet, embedded at compile time.
const STYLESHEET: &str = openkite_ui::MAIN_CSS;

#[test]
fn every_vendored_face_is_declared_exactly_once() {
    assert_eq!(
        STYLESHEET.matches("@font-face {").count(),
        FACES.len(),
        "one @font-face rule per vendored face"
    );
    for f in FACES {
        let src = format!("src: url(\"{}\") format(\"woff2\");", src_url(f));
        assert_eq!(
            STYLESHEET.matches(&src).count(),
            1,
            "{} is not declared exactly once in assets/main.css",
            f.file
        );
        assert!(
            STYLESHEET.contains(&format!("font-family: \"{}\";", f.family)),
            "{} does not declare its family",
            f.file
        );
        assert!(
            STYLESHEET.contains(&format!("font-weight: {};", f.weight)),
            "{} does not declare its weight",
            f.file
        );
    }
}

#[test]
fn the_families_are_the_ones_the_type_stacks_name() {
    assert!(STYLESHEET.contains("--font-sans: \"IBM Plex Sans\""));
    assert!(STYLESHEET.contains("--font-mono: \"IBM Plex Mono\""));
    for f in FACES {
        assert!(
            STYLESHEET.contains(&format!("\"{}\"", f.family)),
            "{} names a family no token resolves",
            f.file
        );
    }
}

#[test]
fn no_face_is_fetched_from_an_external_host() {
    for needle in ["googleapis", "gstatic", "@import", "http://", "https://"] {
        assert!(
            !STYLESHEET.contains(needle),
            "assets/main.css references {needle}; every face is vendored"
        );
    }
}

#[test]
fn every_face_is_a_woff2_payload_under_a_budget() {
    for f in FACES {
        assert!(f.bytes.len() > 4, "{} is empty", f.file);
        assert_eq!(&f.bytes[..4], b"wOF2", "{} is not WOFF2", f.file);
    }
    // The Latin1 subsets are 7 files. Swapping in the all-script "complete"
    // faces would take the embedded payload past half a megabyte of the
    // desktop head, so the budget is part of the contract.
    assert!(
        (100 * 1024..160 * 1024).contains(&payload_bytes()),
        "face payload is {} bytes",
        payload_bytes()
    );
}

#[test]
fn the_route_serves_exactly_the_vendored_files() {
    for face in FACES {
        assert!(src_url(face).starts_with(FONT_ROUTE));
        assert_eq!(
            face_by_file(face.file).map(|served| served.file),
            Some(face.file)
        );
    }
    assert!(face_by_file("ibm-plex-sans-800.woff2").is_none());
    assert!(face_by_file("../assets/fonts/ibm-plex-sans-400.woff2").is_none());
    assert!(face_by_file("").is_none());
}

#[test]
fn embedded_css_replaces_every_face_url_with_its_bytes() {
    let css = embedded_css();

    assert!(
        !css.contains(FONT_ROUTE),
        "the embedded stylesheet must not fetch a face it cannot reach"
    );
    assert_eq!(css.matches("@font-face {").count(), FACES.len());
    assert!(css.len() > STYLESHEET.len());

    for f in FACES {
        let uri = data_uri(f);
        assert!(css.contains(&uri), "{} is not embedded", f.file);
        let encoded = uri
            .strip_prefix("data:font/woff2;base64,")
            .expect("a data URI prefixes its payload");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("the payload is base64");
        assert_eq!(decoded, f.bytes, "{} did not survive embedding", f.file);
    }
}

#[test]
fn embedded_css_is_the_stylesheet_with_only_the_faces_swapped() {
    let css = embedded_css();
    for outside in [
        "var(--font-sans)",
        "var(--font-mono)",
        ".app-shell",
        ".log-panel",
        "--r-pill: 999px;",
    ] {
        assert!(css.contains(outside), "{outside} was lost in embedding");
    }
}
