//! Dumps the two confirmation dialogs to a self-contained HTML file for review.
use dioxus::prelude::*;
use openkite_ui::actions_view::{DeleteDialog, DrainDialog, DrainRow, DrainToggles, Propagation};
use openkite_ui::{AppShell, Connection, Counts};

#[derive(Props, Clone, PartialEq)]
struct PairProps {
    typed: String,
    propagation: Propagation,
    toggles: DrainToggles,
}

#[allow(non_snake_case)]
fn Pair(props: PairProps) -> Element {
    let noop = use_callback(|_: ()| {});
    let on_typed = use_callback(|_: String| {});
    let on_propagation = use_callback(|_: Propagation| {});
    let on_toggles = use_callback(|_: DrainToggles| {});

    rsx! {
        DeleteDialog {
            open: true,
            kind: "Deployment".to_string(),
            name: "checkout-api".to_string(),
            typed: props.typed.clone(),
            propagation: props.propagation,
            on_typed,
            on_propagation,
            on_confirm: noop,
            on_cancel: noop,
        }
        DrainDialog {
            open: true,
            node: "ip-10-0-5-8".to_string(),
            rows: vec![
                DrainRow::evicting("prod/checkout-api-7d9c4f8b6-2xqzt"),
                DrainRow::evicting("prod/worker-5f6d8c9b4-h7lmn"),
                DrainRow::skipping("prod/metrics-9c4d2f7b8-z8vxn", "owned by a DaemonSet"),
                DrainRow::skipping("kube-system/coredns-6b8f7d9c4-q2wrt", "owned by a DaemonSet"),
                DrainRow::skipping("prod/session-cache-2f7c9d4b8-p9qrs", "would lose its emptyDir data"),
            ],
            toggles: props.toggles,
            on_toggles,
            on_confirm: noop,
            on_cancel: noop,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PageProps {
    typed: String,
    propagation: Propagation,
    toggles: DrainToggles,
}

#[allow(non_snake_case)]
fn Page(props: PageProps) -> Element {
    let counts = Counts::new()
        .with("pods", 99)
        .with("deployments", 18)
        .with("nodes", 4)
        .with("namespaces", 12);
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
                        p { "Destructive controls, confirmed" }
                    }
                }
                div { class: "figure-stack",
                    Pair {
                        typed: props.typed.clone(),
                        propagation: props.propagation,
                        toggles: props.toggles,
                    }
                }
            },
        }
    }
}

fn main() {
    let props = PageProps {
        typed: "checkout-api".to_string(),
        propagation: Propagation::Foreground,
        toggles: DrainToggles::default(),
    };
    let mut dom = VirtualDom::new_with_props(Page, props);
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
         <title>openkite · confirmations</title>\n<style>\n{css}\n\
         .figure-stack{{display:flex;flex-direction:column;gap:28px;padding:24px}}\
         .figure-stack .dlg-scrim{{position:static;inset:auto;background:none;padding:0;backdrop-filter:none}}\n\
         </style>\n</head>\n\
         <body>\n{sprite}\n{body}\n</body>\n</html>\n"
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dialogs.html".to_string());
    std::fs::write(&out, html).unwrap();
    println!("wrote {out}");
}
