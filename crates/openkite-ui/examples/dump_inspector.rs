//! Dumps the shell + resource table + inspector to a standalone HTML file for review.

use dioxus::prelude::*;
use openkite_ui::inspector::{
    Container, Detail, Event, Gauge, InspectorData, InspectorState, Manifest, ResourceLink,
    SchemaNode, Tab,
};
use openkite_ui::inspector_view::Inspector;
use openkite_ui::nav::CATALOGUE;
use openkite_ui::shell::{AppShell, Connection, Counts};
use openkite_ui::table::{columns, Density, Row, Sort, SortDir, TableState};
use openkite_ui::table_view::ResourceTable;

const NAMES: [(&str, &str, &str, &str, &str); 10] = [
    ("api-7d9c4f8b6-2xqzt", "prod", "Running", "3d", "0"),
    ("api-7d9c4f8b6-9wq4m", "prod", "Running", "3d", "1"),
    ("worker-6b8f7d5c4-zz2lt", "prod", "Pending", "12m", "4"),
    (
        "coredns-5d78c9869d-4xk2p",
        "kube-system",
        "Running",
        "9d",
        "1",
    ),
    (
        "metrics-server-7c9f8b6d5-qj4vt",
        "kube-system",
        "Running",
        "9d",
        "0",
    ),
    (
        "ingress-nginx-9f4b7c8d6-hs7wn",
        "ingress",
        "Running",
        "4d",
        "2",
    ),
    (
        "cert-manager-6c8d9f7b5-lm3qd",
        "cert-manager",
        "Running",
        "4d",
        "0",
    ),
    ("postgres-0", "data", "Running", "21d", "0"),
    ("redis-1", "data", "Running", "21d", "0"),
    ("job-migrate-4kz9p", "prod", "Completed", "2h", "0"),
];

fn rows() -> Vec<Row> {
    NAMES
        .iter()
        .map(|(name, ns, status, age, restarts)| {
            Row::new(format!("pod/{ns}/{name}"), *name)
                .with_namespace(*ns)
                .with_cell("ns", *ns)
                .with_cell("status", *status)
                .with_cell("ready", "1/1")
                .with_cell("restarts", *restarts)
                .with_cell("age", *age)
                .with_cell("ctrl", "Deployment/api")
                .with_cell("qos", "Burstable")
                .with_cell("node", "prod-east-1-node-3")
        })
        .collect()
}

fn detail() -> Detail {
    Detail {
        props: vec![
            ("Namespace".to_string(), "prod".to_string()),
            ("Node".to_string(), "prod-east-1-node-3".to_string()),
            ("Pod IP".to_string(), "10.42.3.19".to_string()),
            ("QoS class".to_string(), "Burstable".to_string()),
            ("Started".to_string(), "3d 4h ago".to_string()),
            ("Owner".to_string(), "Deployment/api".to_string()),
        ],
        labels: vec![
            ("app".to_string(), "api".to_string()),
            ("pod-template-hash".to_string(), "7d9c4f8b6".to_string()),
            ("team".to_string(), "platform".to_string()),
        ],
        annotations: vec![
            ("checksum/config".to_string(), "9f2c14ab".to_string()),
            ("prometheus.io/scrape".to_string(), "true".to_string()),
        ],
        conditions: vec![
            ("Ready".to_string(), true),
            ("ContainersReady".to_string(), true),
            ("PodScheduled".to_string(), true),
        ],
    }
}

fn data() -> InspectorData {
    InspectorData {
        detail: detail(),
        gauges: vec![
            Gauge {
                label: "CPU",
                value: "180m / 500m".to_string(),
                pct: 36,
            },
            Gauge {
                label: "Memory",
                value: "420Mi / 512Mi".to_string(),
                pct: 82,
            },
        ],
        containers: vec![
            Container {
                name: "api".to_string(),
                image: "ghcr.io/jomakori/openkite:0.4.2".to_string(),
                ports: "8080/TCP".to_string(),
                restarts: 1,
                requests: "100m / 128Mi".to_string(),
                limits: "500m / 512Mi".to_string(),
                liveness: "httpGet /healthz:8080".to_string(),
                mounts: "/var/run/secrets, /etc/openkite".to_string(),
            },
            Container {
                name: "envoy".to_string(),
                image: "istio/proxyv2:1.23.0".to_string(),
                ports: "15090/TCP".to_string(),
                restarts: 0,
                requests: "50m / 64Mi".to_string(),
                limits: "200m / 256Mi".to_string(),
                liveness: "httpGet /healthz/ready:15021".to_string(),
                mounts: "/etc/istio/proxy".to_string(),
            },
        ],
        events: vec![
            Event {
                kind: "Normal",
                reason: "Scheduled".to_string(),
                message: "Successfully assigned prod/api-7d9c4f8b6-2xqzt to node-3".to_string(),
                age: "3d".to_string(),
                count: 1,
                span: String::new(),
            },
            Event {
                kind: "Warning",
                reason: "BackOff".to_string(),
                message: "Back-off restarting failed container api".to_string(),
                age: "2h".to_string(),
                count: 41,
                span: "40s".to_string(),
            },
        ],
        resources: vec![
            ResourceLink {
                id: "deployment/prod/api".to_string(),
                label: "Deployment/api".to_string(),
                status: Some("3/3 ready".to_string()),
            },
            ResourceLink {
                id: "service/prod/api".to_string(),
                label: "Service/api".to_string(),
                status: Some("ClusterIP".to_string()),
            },
        ],
        schema: vec![SchemaNode::new("spec", "object", "Desired state of the pod")
            .with_children(vec![
                SchemaNode::new("containers", "array", "Member containers"),
                SchemaNode::new("nodeSelector", "object", "Scheduling constraints"),
            ])],
        manifest: Some(Manifest::new(
            "apiVersion: v1\nkind: Pod\nmetadata:\n  name: api-7d9c4f8b6-2xqzt\n  namespace: prod\nspec:\n  containers:\n    - name: api\n      image: ghcr.io/jomakori/openkite:0.4.2\n",
        )),
    }
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    kind: String,
    label: String,
    rows: Vec<Row>,
    state: TableState,
    inspector: InspectorState,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    let on_close = use_callback(|_: ()| {});
    let on_tab = use_callback(|_: Tab| {});
    let on_action = use_callback(|_: String| {});
    let on_goto = use_callback(|_: String| {});
    rsx! {
        ResourceTable {
            kind: props.kind.clone(),
            label: props.label.clone(),
            rows: props.rows.clone(),
            state: props.state.clone(),
            namespaces: Vec::new(),
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
        }
        Inspector {
            name: "api-7d9c4f8b6-2xqzt".to_string(),
            kind_label: "Pod".to_string(),
            kind: props.kind.clone(),
            namespace: Some("prod".to_string()),
            state: props.inspector.clone(),
            data: data(),
            on_close,
            on_tab,
            on_action,
            on_goto,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PageProps {
    rows: Vec<Row>,
    state: TableState,
    inspector: InspectorState,
}

#[allow(non_snake_case)]
fn Page(props: PageProps) -> Element {
    let counts = Counts::new()
        .with("pods", props.rows.len())
        .with("deployments", 18)
        .with("nodes", 9)
        .with("namespaces", 4)
        .with("events", 128);
    rsx! {
        AppShell {
            counts,
            active_path: "/workloads/pods".to_string(),
            cluster: "prod-east-1".to_string(),
            connection: Connection::Live,
            children: rsx! {
                div { class: "page-head",
                    div {
                        h1 { "Pods" }
                        p { "All pods in prod-east-1" }
                    }
                }
                Harness {
                    kind: "pods".to_string(),
                    label: "pods".to_string(),
                    rows: props.rows.clone(),
                    state: props.state.clone(),
                    inspector: props.inspector.clone(),
                }
            },
        }
    }
}

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: dump_inspector <out.html> [overview|containers|metrics|yaml|events]");
        std::process::exit(2);
    };
    let tab = match std::env::args().nth(2).as_deref() {
        Some("containers") => Tab::Containers,
        Some("metrics") => Tab::Metrics,
        Some("yaml") => Tab::Yaml,
        Some("events") => Tab::Events,
        _ => Tab::Overview,
    };

    let table_state = TableState {
        sort: Sort::new("name", SortDir::Asc),
        density: Density::Comfortable,
        selected: ["pod/prod/api-7d9c4f8b6-2xqzt".to_string()]
            .into_iter()
            .collect(),
        ..TableState::default()
    };
    let mut inspector = InspectorState::default();
    inspector.open("pod/prod/api-7d9c4f8b6-2xqzt", "pods");
    inspector.tab = tab;

    let mut dom = VirtualDom::new_with_props(
        Page,
        PageProps {
            rows: rows(),
            state: table_state,
            inspector,
        },
    );
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
         <title>OpenKite — pods inspector</title><style>{css}</style></head>\
         <body>{sprite}{body}</body></html>"
    );
    std::fs::write(&out, html).unwrap();
    println!(
        "wrote {out} ({} catalogue kinds, tab {})",
        CATALOGUE.len(),
        tab.id()
    );
    let _: Vec<&str> = columns("pods").iter().map(|c| c.id).collect();
}
