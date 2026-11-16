//! Dumps the rendered shell to a self-contained HTML file for visual review.

use dioxus::prelude::*;
use openkite_ui::{AppShell, AppShellProps, Connection, Counts};

fn main() {
    let mut dom = VirtualDom::new_with_props(
        AppShell,
        AppShellProps {
            counts: Counts::new()
                .with("pods", 42)
                .with("deployments", 18)
                .with("daemonsets", 6)
                .with("statefulsets", 4)
                .with("replicasets", 31)
                .with("jobs", 9)
                .with("cronjobs", 5)
                .with("configmaps", 26)
                .with("secrets", 12)
                .with("hpas", 3)
                .with("services", 24)
                .with("ingresses", 7)
                .with("networkpolicies", 11)
                .with("pvcs", 8)
                .with("pvs", 8)
                .with("storageclasses", 2)
                .with("serviceaccounts", 33)
                .with("roles", 14)
                .with("clusterroles", 15)
                .with("rolebindings", 14)
                .with("helm", 6)
                .with("crds", 21)
                .with("certificates", 5)
                .with("servicemonitors", 4)
                .with("nodes", 9)
                .with("namespaces", 17)
                .with("events", 128),
            active_path: "/workloads/pods".to_string(),
            cluster: "prod-east-1".to_string(),
            connection: Connection::Live,
            children: rsx! {
                div { class: "page-head", h1 { "Pods" } }
                div { class: "card", "Page body rendered by the routed surface." }
            },
        },
    );
    dom.rebuild_in_place();
    let mut body = dioxus_ssr::render(&dom);

    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/console.css"))
        .unwrap()
        .replace(
            "url(\"/assets/fonts/",
            &format!("url(\"file://{}/assets/fonts/", env!("CARGO_MANIFEST_DIR")),
        );
    let sprite =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons.svg")).unwrap();
    // Non-web asset URLs resolve to the crate directory; point the sprite refs in-document.
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    body = body.replace(&format!("{crate_dir}/assets/icons.svg#"), "#");
    body = body.replace(&format!("{crate_dir}#"), "#");

    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>OpenKite shell</title><style>{css}</style></head>\n<body>{sprite}\n{body}</body></html>\n"
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "shell.html".to_string());
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
