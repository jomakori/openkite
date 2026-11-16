//! Dumps a rendered resource table to a self-contained HTML file for visual review.

use dioxus::prelude::*;
use openkite_ui::table::{Density, Row, Sort, SortDir, TableState};
use openkite_ui::table_view::ResourceTable;
use openkite_ui::{AppShell, Connection, Counts};

fn pod(name: &str, ns: &str, status: &str, ready: &str, restarts: &str, age: &str) -> Row {
    Row::new(format!("pod/{ns}/{name}"), name)
        .with_namespace(ns)
        .with_cell("ns", ns)
        .with_cell("status", status)
        .with_cell("ready", ready)
        .with_cell("restarts", restarts)
        .with_cell("age", age)
        .with_cell(
            "ctrl",
            format!(
                "StatefulSet/{}",
                name.rsplit('-').next_back().unwrap_or(name)
            ),
        )
        .with_cell("qos", "Guaranteed")
        .with_cell("node", "ip-10-0-5-8")
}

fn rows() -> Vec<Row> {
    vec![
        pod("api-7d9c4f8b6-2xqzt", "prod", "Running", "2/2", "0", "3d"),
        pod(
            "worker-5f6d8c9b4-h7lmn",
            "prod",
            "Running",
            "1/1",
            "4",
            "12m",
        ),
        pod(
            "coredns-6b8f7d9c4-q2wrt",
            "kube-system",
            "Running",
            "1/1",
            "0",
            "9d",
        ),
        pod(
            "metrics-9c4d2f7b8-z8vxn",
            "kube-system",
            "Pending",
            "0/1",
            "0",
            "48s",
        ),
        pod(
            "ingress-ngx-7b9d5c8f6-k4mnp",
            "ingress",
            "Running",
            "1/1",
            "1",
            "6d",
        ),
        pod(
            "batch-runner-2f7c9d4b8-p9qrs",
            "jobs",
            "Completed",
            "0/1",
            "0",
            "2d",
        ),
    ]
    .into_iter()
    .map(|row| {
        let kids = if row.name.starts_with("api") {
            vec![pod(
                "api-7d9c4f8b6-2xqzt",
                "prod",
                "Running",
                "2/2",
                "0",
                "3d",
            )]
        } else {
            Vec::new()
        };
        row.with_kids(kids)
    })
    .collect()
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    rows: Vec<Row>,
    state: TableState,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    rsx! {
        ResourceTable {
            kind: "pods".to_string(),
            label: "Pods".to_string(),
            rows: props.rows.clone(),
            state: props.state.clone(),
            namespaces: Vec::new(),
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PageProps {
    rows: Vec<Row>,
    state: TableState,
}

#[allow(non_snake_case)]
fn Page(props: PageProps) -> Element {
    let mut counts = Counts::new()
        .with("pods", props.rows.len())
        .with("deployments", 18);
    counts = counts
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
                Harness { rows: props.rows.clone(), state: props.state.clone() }
            },
        }
    }
}

fn main() {
    let state = TableState {
        sort: Sort::new("name", SortDir::Asc),
        density: Density::Comfortable,
        expanded: ["pod/prod/api-7d9c4f8b6-2xqzt".to_string()]
            .into_iter()
            .collect(),
        ..TableState::default()
    };
    let mut dom = VirtualDom::new_with_props(
        Page,
        PageProps {
            rows: rows(),
            state,
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
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>openkite · Pods</title>\n<style>\n{css}\n</style>\n</head>\n\
         <body>\n{sprite}\n{body}\n</body>\n</html>\n"
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "table.html".to_string());
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
