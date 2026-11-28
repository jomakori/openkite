//! Dumps the two metrics surfaces to self-contained HTML files for visual review: a pods
//! list whose rows carry trend lines, and a workload chart with its threshold mark.

use dioxus::prelude::*;
use openkite_ui::chart::{WorkloadChart, WorkloadSeries};
use openkite_ui::metrics::{MetricSource, Series, Threshold};
use openkite_ui::table::{Row, TableState};
use openkite_ui::table_view::ResourceTable;
use openkite_ui::{AppShell, Connection, Counts};

fn pod(name: &str, ns: &str, status: &str, restarts: &str, age: &str, cpu: &[u8]) -> Row {
    Row::new(format!("pod/{ns}/{name}"), name)
        .with_namespace(ns)
        .with_cell("ns", ns)
        .with_cell("status", status)
        .with_cell("ready", "1/1")
        .with_cell("restarts", restarts)
        .with_cell("age", age)
        .with_cell("ctrl", "Deployment/api")
        .with_cell("qos", "Burstable")
        .with_cell("node", "ip-10-0-5-8")
        .with_trend(Series::with_samples(cpu.iter().copied()))
}

fn rows() -> Vec<Row> {
    vec![
        pod(
            "api-7d9c4f8b6-2xqzt",
            "prod",
            "Running",
            "0",
            "3d",
            &[38, 41, 44, 52, 49, 55, 61, 58, 64, 71, 68, 74],
        ),
        pod(
            "worker-5f6d8c9b4-h7lmn",
            "prod",
            "Running",
            "4",
            "12m",
            &[12, 14, 11, 9, 15, 18, 16, 21, 19, 24, 22, 26],
        ),
        pod(
            "coredns-6b8f7d9c4-q2wrt",
            "kube-system",
            "Running",
            "0",
            "9d",
            &[6, 5, 7, 6, 8, 7, 6, 5, 6, 7, 6, 5],
        ),
        pod(
            "metrics-9c4d2f7b8-z8vxn",
            "kube-system",
            "Pending",
            "0",
            "48s",
            &[],
        ),
        pod(
            "ingress-ngx-7b9d5c8f6-k4mnp",
            "ingress",
            "Running",
            "1",
            "6d",
            &[44, 52, 61, 70, 76, 82, 88, 91, 87, 92, 90, 94],
        ),
    ]
}

#[derive(Props, Clone, PartialEq)]
struct TableProps {
    rows: Vec<Row>,
}

#[allow(non_snake_case)]
fn TablePage(props: TableProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    // Narrow the column set so the trend column is on screen in the capture, the way a
    // reader would trim columns that matter less to them.
    let state = TableState {
        hidden: ["ctrl".to_string(), "qos".to_string(), "ready".to_string()]
            .into_iter()
            .collect(),
        ..TableState::default()
    };
    rsx! {
        ResourceTable {
            kind: "pods".to_string(),
            label: "Pods".to_string(),
            rows: props.rows.clone(),
            state,
            namespaces: Vec::new(),
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
        }
    }
}

#[allow(non_snake_case)]
fn TableShell(props: TableProps) -> Element {
    let counts = Counts::new().with("pods", props.rows.len());
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
                TablePage { rows: props.rows.clone() }
            },
        }
    }
}

#[allow(non_snake_case)]
fn ChartPage() -> Element {
    let counts = Counts::new().with("pods", 5);
    let cpu = Series::with_samples([31u8, 44, 39, 52, 61, 58, 66, 72, 69, 77]);
    let mem = Series::with_samples([54u8, 56, 61, 63, 66, 64, 68, 71, 74, 79]);
    rsx! {
        AppShell {
            counts,
            active_path: "/workloads/deployments".to_string(),
            cluster: "prod-east-1".to_string(),
            connection: Connection::Live,
            children: rsx! {
                div { class: "page-head",
                    div {
                        h1 { "api" }
                        p { "Deployment · prod" }
                    }
                }
                div { class: "chart-grid", style: "max-width: 720px",
                    WorkloadChart {
                        title: "CPU and memory",
                        sub: "last 10 samples, judged against its request",
                        series: vec![
                            WorkloadSeries::new("CPU", "var(--accent)", cpu),
                            WorkloadSeries::new("Memory", "var(--violet)", mem),
                        ],
                        source: MetricSource::MetricsServer,
                        threshold: Threshold::default(),
                        visible: true,
                    }
                    WorkloadChart {
                        title: "Filesystem",
                        sub: "no metrics source on this cluster",
                        series: vec![WorkloadSeries::new(
                            "Filesystem",
                            "var(--brand)",
                            Series::default(),
                        )],
                        source: MetricSource::Absent,
                        threshold: Threshold::default(),
                        visible: true,
                    }
                }
            },
        }
    }
}

fn render(node: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(node);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn render_with_props(rows: Vec<Row>) -> String {
    let mut dom = VirtualDom::new_with_props(TableShell, TableProps { rows });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn page(title: &str, body: String) -> String {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let console = std::fs::read_to_string(format!("{crate_dir}/assets/console.css"))
        .unwrap()
        .replace(
            "url(\"/assets/fonts/",
            &format!("url(\"file://{crate_dir}/assets/fonts/"),
        );
    let charts = std::fs::read_to_string(format!("{crate_dir}/src/chart/chart.css")).unwrap();
    let sprite = std::fs::read_to_string(format!("{crate_dir}/assets/icons.svg")).unwrap();
    let body = body
        .replace(&format!("{crate_dir}/assets/icons.svg#"), "#")
        .replace(&format!("{crate_dir}#"), "#");
    format!(
        "<!doctype html>\n<html lang=\"en\" data-appearance=\"dark\" data-theme=\"frost-dark\">\n\
         <head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n{console}\n{charts}\n</style>\n</head>\n\
         <body>\n{sprite}\n{body}\n</body>\n</html>\n"
    )
}

fn main() {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let table = page("openkite · Pods", render_with_props(rows()));
    let chart = page("openkite · api usage", render(ChartPage));
    for (name, html) in [("pods-trend.html", table), ("workload-chart.html", chart)] {
        let path = format!("{out_dir}/{name}");
        std::fs::write(&path, html).unwrap();
        println!("wrote {path}");
    }
}
