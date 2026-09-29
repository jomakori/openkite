//! Class-vocabulary contract: every class the root route renders must be
//! declared by the console stylesheet the hosts inject.
//!
//! The shared UI crate owns the stylesheet (`openkite_ui::MAIN_CSS`,
//! vendored from `crates/openkite-ui/assets/main.css`); the test reads it
//! from the manifest path that ships it so a moved file fails loudly here
//! rather than silently unpainted in a browser.
//!
//! Historically the web root route rendered `class="surface"` and
//! `status-ok`, neither of which any stylesheet declared at the time: the
//! page carried console classes that could not paint. OKT-150 since added
//! `.surface` and `.status-ok` rules for the desktop surfaces — the page
//! still must not render them, because they are the bespoke chrome the
//! shared shell replaced; `the_old_bespoke_classes_render_nowhere` pins the
//! rendering half while the SSR suite pins the classes' absence from the
//! composed markup.

use std::collections::BTreeSet;
use std::path::PathBuf;

use openkite_api::capability::Capabilities;
use openkite_web::ssr::{render_body, SecretRef, Snapshot};

/// Candidate locations, relative to this crate's manifest, in lookup order.
const STYLESHEET_LOCATIONS: [&str; 2] = [
    "../openkite-ui/assets/main.css",
    "../openkite-desktop/assets/main.css",
];

/// The console stylesheet, read from the first location that exists.
fn stylesheet() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in STYLESHEET_LOCATIONS {
        let path = root.join(relative);
        if path.exists() {
            let css = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
            assert!(
                css.contains(".app-shell"),
                "{} does not look like the console stylesheet",
                path.display()
            );
            return css;
        }
    }
    panic!("no console stylesheet found; looked for {STYLESHEET_LOCATIONS:?}");
}

/// Every class name a stylesheet declares, read from the `.<name>` tokens in
/// its selectors (comments and property values are skipped by construction:
/// a token only counts when it starts with a letter, `_` or `-`).
fn declared_classes(css: &str) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();
    let bytes = css.as_bytes();
    let mut index = 0;
    let mut in_comment = false;
    while index < bytes.len() {
        if in_comment {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            in_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'.' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'-')
            {
                end += 1;
            }
            if end > start && !bytes[start].is_ascii_digit() {
                classes.insert(String::from_utf8_lossy(&bytes[start..end]).into_owned());
            }
            index = end;
            continue;
        }
        index += 1;
    }
    classes
}

/// Every class token in the rendered `class="…"` attributes, in document
/// order, so a failure lists the classes in the order they paint.
fn rendered_classes(html: &str) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();
    let mut rest = html;
    while let Some(at) = rest.find("class=\"") {
        rest = &rest[at + "class=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        for token in rest[..end].split_whitespace() {
            classes.insert(token.to_string());
        }
        rest = &rest[end..];
    }
    classes
}

/// The snapshot shapes the root route has to paint: disconnected, connected
/// with a secret in scope, and connected with nothing in scope.
fn snapshots() -> Vec<Snapshot> {
    vec![
        Snapshot::default(),
        Snapshot {
            capabilities: Capabilities::in_process(),
            connected: true,
            context: Some("in-cluster".into()),
            secrets: vec![SecretRef {
                namespace: "default".into(),
                name: "regcred".into(),
            }],
        },
        Snapshot {
            capabilities: Capabilities::server_side(),
            connected: true,
            context: Some("kubeconfig".into()),
            secrets: Vec::new(),
        },
    ]
}

#[test]
fn every_class_the_root_route_renders_is_declared() {
    let css = stylesheet();
    let declared = declared_classes(&css);

    let mut missing: BTreeSet<String> = BTreeSet::new();
    let mut rendered: BTreeSet<String> = BTreeSet::new();
    for snapshot in snapshots() {
        let body = render_body(&snapshot);
        for class in rendered_classes(&body) {
            rendered.insert(class.clone());
            if !declared.contains(&class) {
                missing.insert(class);
            }
        }
    }

    assert!(
        !rendered.is_empty(),
        "the root route rendered no classes at all — the markup moved?"
    );
    assert!(
        missing.is_empty(),
        "classes rendered by the root route with no rule in the console \
         stylesheet: {missing:?}\ndeclared by the route: {rendered:?}"
    );
}

#[test]
fn the_route_renders_the_shared_shell_and_a_status_pill() {
    let body = render_body(&snapshots().remove(1));
    for class in [
        "app-shell",
        "sidebar",
        "nav-item",
        "topbar",
        "content",
        "panel",
        "kv-list",
        "status-entry",
        "status-dot",
        "pill",
    ] {
        assert!(
            rendered_classes(&body).contains(class),
            "root route must render .{class}: {body}"
        );
    }
}

#[test]
fn the_old_bespoke_classes_render_nowhere() {
    for snapshot in snapshots() {
        let rendered = rendered_classes(&render_body(&snapshot));
        assert!(!rendered.contains("surface"), "route renders .surface");
        assert!(!rendered.contains("status-ok"), "route renders .status-ok");
    }
}
