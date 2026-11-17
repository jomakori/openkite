//! Dumps the shell + overview board to a standalone HTML file for review.

use dioxus::prelude::*;
use openkite_ui::overview::{
    ArgoRow, BoardAction, CapacityGauge, EventRow, NodeRow, OverviewData, OverviewLayout, WidgetId,
};
use openkite_ui::overview_view::Overview;
use openkite_ui::shell::{AppShell, Connection, Counts};

fn data() -> OverviewData {
    let pods = vec![
        ("api-7d9c4f8b6-2xqzt", "prod", "Running", "0"),
        ("api-7d9c4f8b6-9wq4m", "prod", "Running", "2"),
        ("worker-6b8f7d5c4-zz2lt", "prod", "Pending", "4"),
        ("coredns-5d78c9869d-4xk2p", "kube-system", "Running", "1"),
        ("ingress-nginx-9f4b7c8d6-hs7wn", "ingress", "Running", "0"),
        ("migrate-9c2fd-7xk4p", "data", "CrashLoopBackOff", "11"),
        (
            "cert-manager-6c8d9f7b5-lm3qd",
            "cert-manager",
            "Running",
            "0",
        ),
        ("postgres-0", "data", "Running", "0"),
    ];
    let rows: Vec<openkite_ui::Row> = pods
        .iter()
        .map(|(name, ns, status, restarts)| {
            openkite_ui::Row::new(format!("pod/{ns}/{name}"), *name)
                .with_namespace(*ns)
                .with_cell("status", *status)
                .with_cell("restarts", *restarts)
        })
        .collect();

    let counts = Counts::new()
        .with("pods", rows.len())
        .with("cpu_pct", 42)
        .with("mem_pct", 68);

    let mut d = OverviewData::from_counts("prod-east-1", &counts).with_pods(rows, 3);
    d.nodes = vec![
        NodeRow {
            id: "node/prod-east-1-node-1".into(),
            name: "node-1".into(),
            cpu_pct: 38,
            mem_pct: 54,
            note: "Ready".into(),
            degraded: false,
        },
        NodeRow {
            id: "node/prod-east-1-node-2".into(),
            name: "node-2".into(),
            cpu_pct: 71,
            mem_pct: 82,
            note: "Ready".into(),
            degraded: false,
        },
        NodeRow {
            id: "node/prod-east-1-node-3".into(),
            name: "node-3".into(),
            cpu_pct: 94,
            mem_pct: 97,
            note: "MemoryPressure".into(),
            degraded: true,
        },
    ];
    d.argo = vec![
        ArgoRow {
            id: "argo/openkite".into(),
            name: "openkite".into(),
            sync: "Synced".into(),
            health: "Healthy".into(),
        },
        ArgoRow {
            id: "argo/monitoring".into(),
            name: "monitoring".into(),
            sync: "OutOfSync".into(),
            health: "Degraded".into(),
        },
        ArgoRow {
            id: "argo/ingress".into(),
            name: "ingress".into(),
            sync: "Synced".into(),
            health: "Healthy".into(),
        },
    ];
    d.events = vec![
        EventRow {
            target: Some("pod/prod/migrate-9c2fd-7xk4p".into()),
            kind: "Warning".into(),
            reason: "BackOff".into(),
            object: "pod/migrate-9c2fd-7xk4p".into(),
            message: "Back-off restarting failed container".into(),
            age: "2m".into(),
        },
        EventRow {
            target: Some("node/prod-east-1-node-3".into()),
            kind: "Warning".into(),
            reason: "EvictionThresholdMet".into(),
            object: "node/node-3".into(),
            message: "Attempting to reclaim memory".into(),
            age: "6m".into(),
        },
        EventRow {
            target: None,
            kind: "Normal".into(),
            reason: "Pulled".into(),
            object: "pod/api-7d9c4f8b6-2xqzt".into(),
            message: "Container image already present on machine".into(),
            age: "18m".into(),
        },
        EventRow {
            target: Some("pod/prod/worker-6b8f7d5c4-zz2lt".into()),
            kind: "Warning".into(),
            reason: "FailedScheduling".into(),
            object: "pod/worker-6b8f7d5c4-zz2lt".into(),
            message: "0/9 nodes are available: 1 Insufficient memory".into(),
            age: "31m".into(),
        },
    ];
    d.namespaces = vec![
        ("prod".into(), 48),
        ("kube-system".into(), 22),
        ("ingress".into(), 11),
        ("data".into(), 8),
        ("cert-manager".into(), 4),
    ];
    d.capacity = vec![
        CapacityGauge {
            label: "CPU requests".into(),
            value: "18.4 / 32 cores".into(),
            pct: 57,
            tone: "warn",
        },
        CapacityGauge {
            label: "Memory requests".into(),
            value: "96 / 128 GiB".into(),
            pct: 75,
            tone: "warn",
        },
        CapacityGauge {
            label: "Pods".into(),
            value: "93 / 220".into(),
            pct: 42,
            tone: "",
        },
        CapacityGauge {
            label: "Storage".into(),
            value: "310 / 800 GiB".into(),
            pct: 39,
            tone: "",
        },
    ];
    d
}

#[derive(Props, Clone, PartialEq)]
struct PageProps {
    layout: OverviewLayout,
}

#[allow(non_snake_case)]
fn Page(props: PageProps) -> Element {
    let on_action = use_callback(|_: BoardAction| {});
    let counts = Counts::new()
        .with("pods", 8)
        .with("deployments", 18)
        .with("nodes", 3)
        .with("namespaces", 5)
        .with("events", 128);
    rsx! {
        AppShell {
            counts,
            active_path: "/overview".to_string(),
            cluster: "prod-east-1".to_string(),
            connection: Connection::Live,
            children: rsx! {
                Overview {
                    layout: props.layout.clone(),
                    data: data(),
                    namespaces: Vec::new(),
                    on_action,
                }
            },
        }
    }
}

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: dump_overview <out.html> [idle|editing]");
        std::process::exit(2);
    };
    let mut layout = OverviewLayout::default();
    if std::env::args().nth(2).as_deref() == Some("editing") {
        layout.editing = true;
        layout.hide(WidgetId::Argo);
    }

    let mut dom = VirtualDom::new_with_props(Page, PageProps { layout });
    dom.rebuild_in_place();
    let mut body = dioxus_ssr::render(&dom);

    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let css = std::fs::read_to_string(format!("{crate_dir}/assets/console.css"))
        .unwrap()
        .replace(
            "url(\"/assets/fonts/",
            &format!("url(\"file://{crate_dir}/assets/fonts/"),
        );
    let sprite = std::fs::read_to_string(format!("{crate_dir}/assets/icons.svg")).unwrap();
    body = body.replace(&format!("{crate_dir}/assets/icons.svg#"), "#");
    body = body.replace(&format!("{crate_dir}#"), "#");

    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>OpenKite — overview</title><style>{css}</style></head>\
         <body>{sprite}{body}</body></html>"
    );
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
