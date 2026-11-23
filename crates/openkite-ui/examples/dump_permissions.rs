//! Dumps the same table under three identities to one HTML file for visual review.

use dioxus::prelude::*;
use openkite_ui::access::{Permissions, Verb};
use openkite_ui::table::{Density, Row, Sort, SortDir, TableState};
use openkite_ui::table_view::ResourceTable;

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
    permissions: Permissions,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    let state = TableState {
        sort: Sort::new("name", SortDir::Asc),
        density: Density::Comfortable,
        selected: ["pod/prod/api-7d9c4f8b6-2xqzt".to_string()]
            .into_iter()
            .collect(),
        ..TableState::default()
    };
    rsx! {
        ResourceTable {
            kind: "pods".to_string(),
            label: "Pods".to_string(),
            rows: rows(),
            state,
            namespaces: Vec::new(),
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
            permissions: props.permissions.clone(),
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PageProps {}

#[allow(non_snake_case)]
fn Page(_props: PageProps) -> Element {
    let mut viewer = Permissions::deny_all("the apiserver did not explain");
    viewer.set(Verb::Get, true, None);
    viewer.set(Verb::List, true, None);

    let denied = Permissions::deny_all("pods is forbidden: User \"viewer\" cannot list resource \"pods\" in API group \"\" in the namespace \"prod\"");

    rsx! {
        div { class: "sheet",
            div { class: "sheet-row",
                p { class: "sheet-label", "Editor — every verb allowed, Delete shown" }
                Harness { permissions: Permissions::allow_all() }
            }
            div { class: "sheet-row",
                p { class: "sheet-label", "Viewer — may list, so no Delete and no danger styling" }
                Harness { permissions: viewer }
            }
            div { class: "sheet-row",
                p { class: "sheet-label", "Denied — the list is refused, the reason replaces the table" }
                Harness { permissions: denied }
            }
        }
    }
}

fn main() {
    let mut dom = VirtualDom::new_with_props(Page, PageProps {});
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

    let extra = ".sheet{padding:24px;display:flex;flex-direction:column;gap:20px;max-width:1280px}"
        .to_string()
        + ".sheet-row{display:flex;flex-direction:column;gap:8px}"
        + ".sheet-label{margin:0;font:600 13px/1.4 ui-sans-serif,system-ui;color:#8b93a7;"
        + "text-transform:uppercase;letter-spacing:.06em}";

    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>openkite · permissions</title>\n<style>\n{css}\n{extra}\n</style>\n</head>\n\
         <body>\n{sprite}\n{body}\n</body>\n</html>\n"
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "permissions.html".to_string());
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
