//! Dumps the app shell with the log dock open, for the design screenshot.
//!
//! Usage: `cargo run -p openkite-ui --example dump_logs -- <out.html> [single|split|paused]`

use std::fs;

use dioxus::prelude::*;
use openkite_ui::logs::{DockAction, LogDock, LogPane, PodChoice};
use openkite_ui::logs_view::Dock;
use openkite_ui::AppShell;
use openkite_ui::Connection;
use openkite_ui::Counts;

fn pod(name: &str) -> PodChoice {
    PodChoice::new(&format!("pod/prod/{name}"), name, "prod").with_containers(&["main"])
}

fn api_lines() -> Vec<String> {
    vec![
        "10:24:31.001 INFO  starting gateway v1.8.2 (commit fc1c125)",
        "10:24:31.284 INFO  loaded routes from /etc/gateway/routes.yaml (34 routes)",
        "10:24:31.690 INFO  upstream pool ready: checkout=4 catalog=6 identity=3",
        "10:24:32.115 INFO  GET /healthz 200 1ms",
        "10:24:33.451 WARN  upstream checkout-7f4c slow: 412ms over 500ms budget",
        "10:24:34.002 INFO  GET /api/cart 200 38ms user=8f21",
        "10:24:34.887 INFO  POST /api/order 201 96ms user=8f21",
        "10:24:35.220 ERROR dial tcp 10.0.3.7:5432: connect: connection refused, retrying",
        "10:24:35.331 WARN  connection pool saturation: active=20 idle=0 max=20",
        "10:24:36.010 DEBUG retry 1 of 5 for order-service.take_payment",
        "10:24:36.744 INFO  retry succeeded after 1 attempt",
        "10:24:37.900 TRACE span closed name=POST /api/order dur=5122ms",
        "10:24:38.140 INFO  GET /api/cart 200 31ms user=2c9a",
        "10:24:39.002 INFO  scrape /metrics 200 4ms",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn worker_lines() -> Vec<String> {
    vec![
        "10:24:30.114 INFO  worker-1 picked job 4821 (email.receipt)",
        "10:24:30.887 INFO  job 4821 done in 773ms",
        "10:24:31.002 INFO  worker-1 picked job 4822 (email.receipt)",
        "10:24:33.501 WARN  smtp relay slow: 1.8s for job 4822",
        "10:24:34.220 INFO  job 4822 done in 3218ms",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    dock: LogDock,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_action = use_callback(|_: DockAction| {});
    rsx! {
        AppShell {
            counts: Counts::new()
                .with("pods", 34)
                .with("deployments", 12)
                .with("nodes", 6)
                .with("events", 128),
            active_path: "/pods".to_string(),
            cluster: "prod-eu".to_string(),
            connection: Connection::Live,
            div { class: "page-body",
                h1 { "Pods" }
                p { class: "muted", "The log dock is open for api-7d9c4f8b6-2xqzt." }
            }
            Dock { dock: props.dock.clone(), on_action }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "logs.html".to_string());
    let mode = args.get(2).cloned().unwrap_or_else(|| "single".to_string());

    let api = pod("api-7d9c4f8b6-2xqzt");
    let worker = pod("worker-6b8f7d5c4-zz2lt");

    let mut dock = LogDock::default();
    dock.set_choices(vec![api.clone(), worker.clone()]);

    let mut first = LogPane::new(api.clone());
    first.ingest(&api_lines());
    dock.panes.push(first);
    dock.open = true;

    if mode == "split" || mode == "paused" {
        let mut second = LogPane::new(worker);
        second.ingest(&worker_lines());
        second.pinned = true;
        dock.panes.push(second);
        dock.focused = 1;
    }
    if mode == "paused" {
        dock.panes[0].paused = true;
        dock.focused = 0;
    }

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { dock });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>openkite · logs</title>\
         <link rel=\"stylesheet\" href=\"file://{}/assets/console.css\"></head>\
         <body><div id=\"app\">{html}</div></body></html>",
        env!("CARGO_MANIFEST_DIR")
    );
    fs::write(&out, page).expect("write dump");
    println!("wrote {out} ({mode})");
}
