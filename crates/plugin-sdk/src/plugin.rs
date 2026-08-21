use crate::context::PluginContext;
use crate::meta::PluginMeta;
use crate::route::PluginRoute;
use crate::sidebar::SidebarSection;

/// Every OpenKite plugin implements this trait. It is the **only** interface
/// between core and plugins.
pub trait OpenKitePlugin: Send + Sync {
    /// Plugin identity.
    fn metadata(&self) -> PluginMeta;

    /// Called when a cluster connection is established.
    /// Use `ctx.kube_client` to probe CRDs, start watchers, open gRPC, etc.
    fn on_cluster_connect(&mut self, ctx: &PluginContext) -> anyhow::Result<()>;

    /// Called on cluster disconnect. Clean up watchers, connections, tasks.
    fn on_cluster_disconnect(&mut self);

    /// Sidebar sections to register. Return an empty vec to hide.
    /// Called after `on_cluster_connect` — may be conditional on detection.
    fn sidebar_entries(&self) -> Vec<SidebarSection>;

    /// Routes this plugin handles, merged into the app router (via wildcard
    /// dispatcher — Dioxus Router is compile-time).
    fn routes(&self) -> Vec<PluginRoute>;

    /// Called on app shutdown. Final cleanup.
    fn on_unload(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPlugin {
        connects: usize,
        unloads: usize,
    }

    impl OpenKitePlugin for MockPlugin {
        fn metadata(&self) -> PluginMeta {
            PluginMeta {
                name: "mock".into(),
                display_name: "Mock".into(),
                version: "0.0.0".into(),
                author: "test".into(),
                icon: crate::PluginIcon::BuiltIn("cube"),
                accent_color: None,
            }
        }
        fn on_cluster_connect(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
            self.connects += 1;
            Ok(())
        }
        fn on_cluster_disconnect(&mut self) {}
        fn sidebar_entries(&self) -> Vec<SidebarSection> {
            vec![]
        }
        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }
        fn on_unload(&mut self) {
            self.unloads += 1;
        }
    }

    #[test]
    fn trait_object_construction() {
        let mut plugin: Box<dyn OpenKitePlugin> = Box::new(MockPlugin {
            connects: 0,
            unloads: 0,
        });

        // Stateless methods work without a PluginContext.
        assert_eq!(plugin.metadata().name, "mock");
        assert!(plugin.sidebar_entries().is_empty());
        assert!(plugin.routes().is_empty());

        // Lifecycle methods can be invoked through the trait object.
        plugin.on_cluster_disconnect();
        plugin.on_unload();
    }
}
