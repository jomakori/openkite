//! The bottom dock survives the browser host's SSR snapshot.
//!
//! The dock lives inside the shared `AppShell`, so the web host renders the
//! same tabs the desktop host does. These tests pin that the shell composes it
//! and that its state comes from the runtime globals the SSR pass reads.

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_ui::runtime::{close_all_dock_tabs, open_pod_tab, open_terminal_tab};
use openkite_web::app::{App, AppProps};
use openkite_web::ssr::Snapshot;

fn snapshot() -> Snapshot {
    Snapshot {
        capabilities: Capabilities::in_process(),
        connected: true,
        context: Some("in-cluster".into()),
        secrets: Vec::new(),
        selection: None,
        route: "/".into(),
        workloads: Default::default(),
    }
}

fn pod(name: &str) -> PodObject {
    PodObject {
        name: name.into(),
        namespace: Some("default".into()),
        summary: PodSummary::default(),
        containers: vec![ContainerInfo {
            name: format!("{name}-c"),
            image: "nginx".into(),
            ready: true,
            restarts: 0,
            state: "Running".into(),
        }],
        labels: Default::default(),
        annotations: Default::default(),
        yaml: String::new(),
    }
}

fn render(seed: impl FnOnce()) -> String {
    let mut dom = VirtualDom::new_with_props(
        App,
        AppProps {
            snapshot: snapshot(),
        },
    );
    dom.in_runtime(seed);
    dom.rebuild(&mut dioxus::core::NoOpMutations);
    dioxus_ssr::pre_render(&dom)
}

#[test]
fn a_host_with_no_tabs_renders_no_dock() {
    let html = render(close_all_dock_tabs);
    assert!(!html.contains("data-dock"), "got: {html}");
}

#[test]
fn the_ssr_snapshot_carries_the_open_tabs() {
    let html = render(|| {
        close_all_dock_tabs();
        open_terminal_tab(pod("web-1"));
        open_pod_tab(pod("api-2"));
    });
    assert!(html.contains("data-dock=\"open\""), "got: {html}");
    assert!(html.contains("data-dock-tab-count=\"2\""), "got: {html}");
    assert!(html.contains(">web-1<"), "got: {html}");
    assert!(html.contains(">api-2<"), "got: {html}");
    assert!(html.contains("data-temporary=\"1\""), "got: {html}");
}
