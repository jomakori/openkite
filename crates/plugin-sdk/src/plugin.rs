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
