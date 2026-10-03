//! The rendered secrets surface is scoped by the console's namespace bar.

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::runtime::set_namespace_selection;
use openkite_web::app::{App, AppProps};
use openkite_web::ssr::{SecretRef, Snapshot};

fn two_namespaces() -> Snapshot {
    Snapshot {
        capabilities: Capabilities::in_process(),
        connected: true,
        context: Some("in-cluster".into()),
        secrets: vec![
            SecretRef {
                namespace: "default".into(),
                name: "regcred".into(),
            },
            SecretRef {
                namespace: "kube-system".into(),
                name: "bootstrap-token".into(),
            },
        ],
    }
}

fn render(selection: Vec<String>) -> String {
    let mut dom = VirtualDom::new_with_props(
        App,
        AppProps {
            snapshot: two_namespaces(),
        },
    );
    dom.in_runtime(|| set_namespace_selection(selection));
    dom.rebuild(&mut dioxus::core::NoOpMutations);
    dioxus_ssr::pre_render(&dom)
}

#[test]
fn no_selection_renders_every_secret() {
    let html = render(Vec::new());
    assert!(html.contains("regcred"), "got: {html}");
    assert!(html.contains("bootstrap-token"), "got: {html}");
    assert!(!html.contains("data-empty=\"secrets\""), "got: {html}");
}

#[test]
fn a_selection_narrows_the_rendered_secrets_surface() {
    let html = render(vec!["kube-system".to_string()]);
    assert!(html.contains("bootstrap-token"), "got: {html}");
    assert!(
        !html.contains("regcred"),
        "the `default` secret is out of scope: {html}"
    );
    assert!(
        html.contains("aria-label=\"Clear namespace filter\""),
        "the bar offers the reset while a selection exists: {html}"
    );
}
