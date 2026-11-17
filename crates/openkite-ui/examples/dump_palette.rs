//! Dumps the shell with the command palette open, for the design screenshot.
//!
//! Usage: `cargo run -p openkite-ui --example dump_palette -- <out.html> [objects|labels|commands]`

use dioxus::prelude::*;
use openkite_ui::palette::{Entry, Palette};
use openkite_ui::palette_view::PaletteOverlay;
use openkite_ui::shell::{AppShell, Connection, Counts};

fn entries() -> Vec<Entry> {
    vec![
        Entry::new("pod", "prod", "api-7d9c4f8b6-2xqzt").with_labels(&["app=api", "tier=web"]),
        Entry::new("pod", "prod", "api-canary-849c9b7b8-k4m2p").with_labels(&[
            "app=api",
            "tier=web",
            "track=canary",
        ]),
        Entry::new("pod", "prod", "gateway-6b8f7d5c4-hh9sl")
            .with_labels(&["app=gateway", "tier=web"]),
        Entry::new("pod", "prod", "checkout-5d78c9869d-8kx7p")
            .with_labels(&["app=checkout", "tier=web"]),
        Entry::new("pod", "prod", "catalog-7f4c8d6b9-qq7tz")
            .with_labels(&["app=catalog", "tier=web"]),
        Entry::new("pod", "prod", "ledger-6b8f7d5c4-w2n8r")
            .with_labels(&["app=ledger", "tier=batch"]),
        Entry::new("pod", "prod", "worker-7d9c4f8b6-tt4vh")
            .with_labels(&["app=worker", "tier=batch"]),
        Entry::new("pod", "staging", "api-5d78c9869d-pp3ld").with_labels(&["app=api", "tier=web"]),
        Entry::new("pod", "kube-system", "coredns-5d78c9869d-8kx7p")
            .with_labels(&["k8s-app=dns", "tier=system"]),
        Entry::new("pod", "observability", "prometheus-0")
            .with_labels(&["app=prometheus", "tier=system"]),
        Entry::new("deployment", "prod", "api").with_labels(&["app=api", "tier=web"]),
        Entry::new("deployment", "prod", "checkout").with_labels(&["app=checkout", "tier=web"]),
        Entry::new("deployment", "prod", "catalog").with_labels(&["app=catalog", "tier=web"]),
        Entry::new("deployment", "staging", "api").with_labels(&["app=api", "tier=web"]),
        Entry::new("node", "", "ip-10-0-3-17.eu-west-1.compute.internal")
            .with_labels(&["role=worker", "pool=general"]),
        Entry::new("node", "", "ip-10-0-4-22.eu-west-1.compute.internal")
            .with_labels(&["role=worker", "pool=general"]),
        Entry::new(
            "event",
            "prod",
            "BackOff api-canary-849c9b7b8-k4m2p restarting container",
        )
        .with_labels(&["reason=BackOff"]),
        Entry::new(
            "event",
            "prod",
            "Pulled api-7d9c4f8b6-2xqzt image gateway:v1.8.2",
        )
        .with_labels(&["reason=Pulled"]),
        Entry::new("namespace", "", "prod").with_labels(&["team=payments"]),
        Entry::new("namespace", "", "staging").with_labels(&["team=payments"]),
        Entry::new("argoapp", "argo", "openkite-prod").with_labels(&["sync=Synced"]),
    ]
}

fn query_for(mode: &str) -> &'static str {
    match mode {
        "labels" => "app=api",
        "commands" => ">",
        _ => "api",
    }
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    query: String,
    hits: Vec<openkite_ui::palette::Hit>,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_query = use_callback(|_: String| {});
    let on_active = use_callback(|_: usize| {});
    let on_accept = use_callback(|_: openkite_ui::palette::Hit| {});
    let on_close = use_callback(|_: ()| {});
    rsx! {
        AppShell {
            counts: Counts::new()
                .with("pods", 34)
                .with("deployments", 12)
                .with("nodes", 6)
                .with("events", 128),
            connection: Connection::Live,
            active_path: "/pods".to_string(),
            cluster: "prod-eu-west-1".to_string(),
            PaletteOverlay {
                open: true,
                query: props.query.clone(),
                hits: props.hits.clone(),
                active: 0,
                on_query,
                on_active,
                on_accept,
                on_close,
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "palette.html".to_string());
    let mode = args.get(2).map(String::as_str).unwrap_or("objects");
    let query = query_for(mode);
    let palette = Palette::build(entries());
    let hits = palette.query(query);

    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            query: query.to_string(),
            hits,
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>openkite · command palette</title>\
         <link rel=\"stylesheet\" href=\"{}/assets/console.css\"></head>\
         <body class=\"console\">{html}</body></html>",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&out, page).expect("write dump");
    println!("{out}");
}
