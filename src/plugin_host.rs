//! Plugin host — static registry with panic-contained lifecycle fan-out.
//!
//! Static-first strategy (see `docs/plugin-architecture.md`): Phase 1 has no
//! bundled plugins; `load_static` is the hook where feature-gated workspace
//! members register (e.g. `openkite-plugin-argocd` in Phase 2), filtered by
//! the config's enable/disable state. A dylib loader is deliberately
//! deferred until a real dynamic plugin exists.

use openkite_plugin_sdk::{OpenKitePlugin, PluginContext, PluginRoute, SidebarSection};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Registered plugins in load order. Lifecycle calls fan out in that order;
/// a panicking or erroring plugin is contained and logged — it never
/// propagates out of the registry.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn OpenKitePlugin>>,
}

impl PluginRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Append a plugin.
    /// Consumed by `load_static` (Phase 2) and the plugin manager UI (OKT-19).
    #[allow(dead_code)]
    pub fn register(&mut self, plugin: Box<dyn OpenKitePlugin>) {
        self.plugins.push(plugin);
    }

    /// Immutable view of the registered plugins.
    pub fn plugins(&self) -> &[Box<dyn OpenKitePlugin>] {
        &self.plugins
    }

    /// Register the statically-linked plugins for this build, honoring the
    /// config's enable/disable state.
    ///
    /// Phase 2: feature-gated workspace members register here, e.g.
    /// ```ignore
    /// #[cfg(feature = "plugin-argocd")]
    /// if config.is_enabled("argocd") {
    ///     self.register(Box::new(openkite_plugin_argocd::ArgocdPlugin::new()));
    /// }
    /// ```
    pub fn load_static(&mut self, _config: &crate::config::OpenKiteConfig) {
        // No bundled plugins in Phase 1 — hook left for Phase 2.
    }

    /// Fan out cluster connect, in registration order. Errors and panics are
    /// logged per plugin; the app keeps running.
    pub fn on_cluster_connect(&mut self, ctx: &PluginContext) {
        for plugin in &mut self.plugins {
            let name = plugin.metadata().name.clone();
            let result = catch_unwind(AssertUnwindSafe(|| plugin.on_cluster_connect(ctx)));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    tracing::error!(plugin = %name, error = ?err, "on_cluster_connect failed")
                }
                Err(_) => tracing::error!(plugin = %name, "on_cluster_connect panicked"),
            }
        }
    }

    /// Fan out cluster disconnect.
    pub fn on_cluster_disconnect(&mut self) {
        for plugin in &mut self.plugins {
            let name = plugin.metadata().name.clone();
            let result = catch_unwind(AssertUnwindSafe(|| plugin.on_cluster_disconnect()));
            if result.is_err() {
                tracing::error!(plugin = %name, "on_cluster_disconnect panicked");
            }
        }
    }

    /// Fan out app-shutdown cleanup.
    pub fn on_unload(&mut self) {
        for plugin in &mut self.plugins {
            let name = plugin.metadata().name.clone();
            let result = catch_unwind(AssertUnwindSafe(|| plugin.on_unload()));
            if result.is_err() {
                tracing::error!(plugin = %name, "on_unload panicked");
            }
        }
    }

    /// Merge sidebar sections from all plugins, in registration order.
    pub fn sidebar_entries(&self) -> Vec<SidebarSection> {
        let mut all = Vec::new();
        for plugin in &self.plugins {
            let name = plugin.metadata().name.clone();
            match catch_unwind(AssertUnwindSafe(|| plugin.sidebar_entries())) {
                Ok(entries) => all.extend(entries),
                Err(_) => tracing::error!(plugin = %name, "sidebar_entries panicked"),
            }
        }
        all
    }

    /// Merge routes from all plugins, in registration order.
    pub fn routes(&self) -> Vec<PluginRoute> {
        let mut all = Vec::new();
        for plugin in &self.plugins {
            let name = plugin.metadata().name.clone();
            match catch_unwind(AssertUnwindSafe(|| plugin.routes())) {
                Ok(routes) => all.extend(routes),
                Err(_) => tracing::error!(plugin = %name, "routes panicked"),
            }
        }
        all
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openkite_plugin_sdk::anyhow;
    use openkite_plugin_sdk::{PluginIcon, PluginMeta};
    use std::sync::{Arc, Mutex};

    /// Records lifecycle events into a shared log.
    struct MockPlugin {
        name: String,
        log: Arc<Mutex<Vec<String>>>,
        panic_on_connect: bool,
        connect_error: bool,
    }

    impl MockPlugin {
        fn new(name: &str, log: Arc<Mutex<Vec<String>>>) -> Self {
            Self {
                name: name.into(),
                log,
                panic_on_connect: false,
                connect_error: false,
            }
        }

        fn record(&self, event: &str) {
            self.log
                .lock()
                .unwrap()
                .push(format!("{}-{}", self.name, event));
        }
    }

    impl OpenKitePlugin for MockPlugin {
        fn metadata(&self) -> PluginMeta {
            PluginMeta {
                name: self.name.clone(),
                display_name: self.name.clone(),
                version: "0.0.0".into(),
                author: "test".into(),
                icon: PluginIcon::BuiltIn("cube"),
                accent_color: None,
            }
        }

        fn on_cluster_connect(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
            self.record("connect");
            if self.panic_on_connect {
                panic!("simulated panic");
            }
            if self.connect_error {
                anyhow::bail!("simulated error");
            }
            Ok(())
        }

        fn on_cluster_disconnect(&mut self) {
            self.record("disconnect");
        }

        fn sidebar_entries(&self) -> Vec<SidebarSection> {
            self.record("sidebar");
            vec![]
        }

        fn routes(&self) -> Vec<PluginRoute> {
            vec![]
        }

        fn on_unload(&mut self) {
            self.record("unload");
        }
    }

    /// Lazy client — points at a non-existent cluster but never connects
    /// (mirrors the SDK's own test).
    fn test_context() -> PluginContext {
        let url: http::Uri = "http://127.0.0.1:1".parse().expect("uri");
        let config = kube::Config::new(url);
        let client = kube::Client::try_from(config).expect("client");
        PluginContext::new(
            client.clone(),
            kube::discovery::Discovery::new(client),
            Default::default(),
            Default::default(),
            tokio::runtime::Handle::current(),
        )
    }

    #[tokio::test]
    async fn lifecycle_fires_in_registration_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(MockPlugin::new("a", log.clone())));
        registry.register(Box::new(MockPlugin::new("b", log.clone())));

        let ctx = test_context();
        registry.on_cluster_connect(&ctx);
        registry.on_cluster_disconnect();
        registry.on_unload();

        let events = log.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                "a-connect".to_string(),
                "b-connect".to_string(),
                "a-disconnect".to_string(),
                "b-disconnect".to_string(),
                "a-unload".to_string(),
                "b-unload".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn panicking_and_erroring_plugins_are_contained() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut registry = PluginRegistry::new();

        let mut panicky = MockPlugin::new("panicky", log.clone());
        panicky.panic_on_connect = true;
        let mut erroring = MockPlugin::new("erroring", log.clone());
        erroring.connect_error = true;

        registry.register(Box::new(panicky));
        registry.register(Box::new(erroring));
        registry.register(Box::new(MockPlugin::new("ok", log.clone())));

        let ctx = test_context();
        registry.on_cluster_connect(&ctx); // must not unwind

        let events = log.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                "panicky-connect".to_string(),
                "erroring-connect".to_string(),
                "ok-connect".to_string(),
            ]
        );
    }

    #[test]
    fn sidebar_fan_out_never_unwinds() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(MockPlugin::new("a", log.clone())));
        registry.register(Box::new(MockPlugin::new("b", log.clone())));

        // Exercise both fan-out methods — no plugin panics here, but the
        // containment path is the same machinery the connect test covers.
        let sections = registry.sidebar_entries();
        let routes = registry.routes();
        assert!(sections.is_empty());
        assert!(routes.is_empty());

        let events = log.lock().unwrap().clone();
        assert_eq!(
            events,
            vec!["a-sidebar".to_string(), "b-sidebar".to_string()]
        );
    }
}
