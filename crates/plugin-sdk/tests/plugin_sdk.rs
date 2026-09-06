//! Integration tests for the plugin SDK's stable public API
//! (`crates/plugin-sdk`). Complements the inline unit tests with the
//! cross-module surface: sidebar contribution types, route rendering stubs,
//! lifecycle dispatch through a boxed `OpenKitePlugin`, and UI-handle
//! no-ops. Everything here is headless — no kube client, no Dioxus runtime.

use openkite_plugin_sdk::{
    OpenKitePlugin, PluginIcon, PluginMeta, PluginRoute, PluginUiHandle, SidebarEntry,
    SidebarSection,
};

/// A route render stub: `PluginRoute.render` is a `fn() -> Element`, and in
/// Dioxus 0.7 `Element` is `Result<VNode, RenderError>` — an empty node is a
/// valid component result when never mounted.
fn stub_render() -> dioxus::prelude::Element {
    Ok(dioxus::prelude::VNode::empty())
}

#[test]
fn sidebar_section_holds_named_entries() {
    let section = SidebarSection {
        label: "Argo CD".into(),
        icon: PluginIcon::BuiltIn("argo"),
        accent_color: Some("#ef7b4d".into()),
        entries: vec![
            SidebarEntry {
                label: "Applications".into(),
                icon: PluginIcon::BuiltIn("app"),
                route: "/argocd/apps".into(),
                badge: Some("12".into()),
                badge_color: Some("#ef7b4d".into()),
            },
            SidebarEntry {
                label: "Projects".into(),
                icon: PluginIcon::Svg("<svg/>".into()),
                route: "/argocd/projects".into(),
                badge: None,
                badge_color: None,
            },
        ],
    };

    assert_eq!(section.label, "Argo CD");
    assert_eq!(section.entries.len(), 2);
    assert_eq!(section.entries[0].route, "/argocd/apps");
    assert_eq!(section.entries[0].badge.as_deref(), Some("12"));
    assert_eq!(section.entries[1].icon, PluginIcon::Svg("<svg/>".into()));
    assert_eq!(section.entries[1].badge, None);

    // Structural equality is derived and meaningful.
    let clone = section.clone();
    assert_eq!(section, clone);
    assert_ne!(section.entries[0], section.entries[1]);
}

#[test]
fn plugin_route_carries_path_and_render_fn() {
    let route = PluginRoute {
        path: "/argocd/apps/:name".into(),
        render: stub_render,
    };
    assert_eq!(route.path, "/argocd/apps/:name");
    // Calling the render fn headless must not panic and yields an empty node.
    assert!((route.render)().is_ok());
}

#[test]
fn plugin_meta_surface_is_readable() {
    let meta = PluginMeta {
        name: "argocd".into(),
        display_name: "Argo CD".into(),
        version: "0.1.0".into(),
        author: "OpenKite".into(),
        icon: PluginIcon::BuiltIn("argocd"),
        accent_color: Some("#ef7b4d".into()),
    };
    assert_eq!(meta.name, "argocd");
    assert_eq!(meta.display_name, "Argo CD");
    assert_eq!(meta.version, "0.1.0");
    assert_eq!(meta.author, "OpenKite");
    assert_eq!(meta.icon, PluginIcon::BuiltIn("argocd"));
    assert_eq!(meta.accent_color.as_deref(), Some("#ef7b4d"));
    // Accent is optional for plugins without a brand color.
    let muted = PluginMeta {
        accent_color: None,
        ..meta
    };
    assert_eq!(muted.accent_color, None);
}

/// A full plugin implementation exercising the lifecycle trait.
struct CountingPlugin {
    connects: usize,
    disconnects: usize,
    unloads: usize,
}

impl OpenKitePlugin for CountingPlugin {
    fn metadata(&self) -> PluginMeta {
        PluginMeta {
            name: "counter".into(),
            display_name: "Counter".into(),
            version: "0.0.0".into(),
            author: "test".into(),
            icon: PluginIcon::BuiltIn("cube"),
            accent_color: None,
        }
    }
    fn on_cluster_connect(
        &mut self,
        _ctx: &openkite_plugin_sdk::PluginContext,
    ) -> anyhow::Result<()> {
        self.connects += 1;
        Ok(())
    }
    fn on_cluster_disconnect(&mut self) {
        self.disconnects += 1;
    }
    fn sidebar_entries(&self) -> Vec<SidebarSection> {
        vec![SidebarSection {
            label: "Counter".into(),
            icon: PluginIcon::BuiltIn("cube"),
            accent_color: None,
            entries: vec![],
        }]
    }
    fn routes(&self) -> Vec<PluginRoute> {
        vec![PluginRoute {
            path: "/counter".into(),
            render: stub_render,
        }]
    }
    fn on_unload(&mut self) {
        self.unloads += 1;
    }
}

#[test]
fn boxed_plugin_dispatch_hits_lifecycle_hooks() {
    let mut plugin: Box<dyn OpenKitePlugin> = Box::new(CountingPlugin {
        connects: 0,
        disconnects: 0,
        unloads: 0,
    });

    // Trait-object dispatch through the boxed reference.
    let meta = plugin.metadata();
    assert_eq!(meta.name, "counter");

    let sections = plugin.sidebar_entries();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].label, "Counter");

    let routes = plugin.routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].path, "/counter");

    // Disconnect + unload fire without a cluster and without panicking.
    plugin.on_cluster_disconnect();
    plugin.on_unload();
}

#[test]
fn plugin_ui_handle_default_toast_is_noop() {
    // A handle with no host sink must not panic on toast.
    let handle = PluginUiHandle::default();
    handle.toast("hello");
}

#[test]
fn plugin_ui_handle_toast_with_channel_forwards() {
    use tokio::sync::mpsc;
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let handle = PluginUiHandle::new(Some(tx));
    handle.toast("plugin ready");
    assert_eq!(rx.try_recv().ok().as_deref(), Some("plugin ready"));
    // Channel empty after drain.
    assert!(rx.try_recv().is_err());
}
