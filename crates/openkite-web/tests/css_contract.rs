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
use openkite_ui::components::resource_pane::ResourceRef;
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
                css.contains(".app {"),
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
/// with a secret in scope, connected with nothing in scope, and connected with
/// the detail pane's selection restored from the address (OKT-175).
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
            ..Snapshot::default()
        },
        Snapshot {
            capabilities: Capabilities::server_side(),
            connected: true,
            context: Some("kubeconfig".into()),
            ..Snapshot::default()
        },
        selection_snapshot(),
    ]
}

/// The snapshot a deep link produces: the pane's selection came out of the
/// query string, so the root route paints the pane open.
fn selection_snapshot() -> Snapshot {
    Snapshot {
        capabilities: Capabilities::server_side(),
        connected: true,
        context: Some("kubeconfig".into()),
        selection: Some(ResourceRef::new("Pod", Some("default".into()), "web-1")),
        ..Snapshot::default()
    }
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
        "app",
        "sidebar",
        "nav-item",
        "topbar",
        "breadcrumbs",
        "view",
        "panel",
        "kv-list",
        "sidebar-footer",
        "status-line",
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

/// A deep-linked selection must paint the pane: the same chrome for the
/// selected resource, and nothing of it when nothing is selected.
#[test]
fn the_selection_paints_the_resource_detail_pane() {
    let body = render_body(&selection_snapshot());
    for class in [
        "inspector",
        "inspector-scrim",
        "inspector-resize",
        "inspector-body",
        "inspector-actions",
        "resource-kind",
    ] {
        assert!(
            rendered_classes(&body).contains(class),
            "the selected route must render .{class}: {body}"
        );
    }
    assert!(body.contains("web-1"), "pane identity: {body}");
    assert!(body.contains("data-pane=\"resource\""), "pane node: {body}");

    let plain = render_body(&unselected_snapshot());
    assert!(
        !plain.contains("data-pane=\"resource\""),
        "no selection, no pane: {plain}"
    );
}

/// The root route's own snapshot: no resource is selected.
fn unselected_snapshot() -> Snapshot {
    Snapshot::default()
}

/// The stylesheet with `/* … */` comments removed, so a rule block can be read
/// without a preceding comment's prose matching first.
fn uncommented(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The value of one declaration inside a selector's rule block.
fn declaration(css: &str, selector: &str, property: &str) -> Option<String> {
    let needle = format!("{selector} {{");
    let start = css.find(&needle)?;
    let open = start + needle.len();
    let end = css[open..].find('}')? + open;
    let block = &css[open..end];
    let at = block.find(property)? + property.len();
    let value: String = block[at..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == ':')
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ';')
        .collect();
    Some(value)
}

/// The ≤767px sheet is dismissed by tapping its scrim, so the pane has to sit
/// **above** the scrim. The reference's ladder is 65 scrim / 70 pane; this
/// contract fails if the pane is ever given a lower stop, which would cover
/// the bottom sheet with its own backdrop and leave the × unreachable.
#[test]
fn the_scrim_sits_under_the_pane_it_dismisses() {
    let css = uncommented(&stylesheet());
    let scrim: i64 = declaration(&css, ".inspector-scrim", "z-index")
        .expect(".inspector-scrim must declare a z-index")
        .parse()
        .expect("scrim z-index is a number");
    let pane: i64 = declaration(&css, ".inspector", "z-index")
        .expect(".inspector must declare a z-index")
        .parse()
        .expect("pane z-index is a number");
    assert!(
        scrim < pane,
        "the scrim (z-index {scrim}) must sit under the pane (z-index {pane}), \
         or the ≤767px sheet is covered by its own backdrop and cannot close"
    );
}

/// The ≤767px sheet keeps the reference's own rules: the scrim comes alive,
/// the drag handle is hidden, and the kv label column narrows so the value
/// keeps room on a full-width sheet.
#[test]
fn the_mobile_sheet_keeps_the_reference_rules() {
    let css = uncommented(&stylesheet());
    let at = css
        .find("@media (max-width: 767px)")
        .expect("the ≤767px block");
    let mobile = &css[at..];
    for rule in [
        ".inspector-scrim.show { display: block; }",
        ".inspector-resize { display: none; }",
        ".kv-row { grid-template-columns: 110px 1fr; }",
    ] {
        assert!(
            mobile.contains(rule),
            "the ≤767px sheet is missing `{rule}`"
        );
    }
}
