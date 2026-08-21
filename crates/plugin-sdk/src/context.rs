use kube::Client;

/// Context handed to plugins on cluster connect. `kube_client` is the same
/// client the core app uses — plugins get direct K8s access scoped to RBAC.
pub struct PluginContext {
    pub kube_client: Client,
    /// API discovery — used to probe CRDs (e.g. `argoproj.io`).
    pub discovery: kube::discovery::Discovery,
    /// Read the active theme's CSS variables.
    pub theme: ThemeReadHandle,
    /// UI affordances: toast notifications, modals.
    pub ui: PluginUiHandle,
    /// Spawn async work (tonic clients, watchers). Plugins own their task
    /// handles and MUST abort them in `on_cluster_disconnect`.
    pub runtime: tokio::runtime::Handle,
}

/// Read-only view of the current theme.
#[derive(Clone)]
pub struct ThemeReadHandle {
    // Filled in by core during PluginContext construction.
    pub(crate) values: std::collections::HashMap<String, String>,
}

impl ThemeReadHandle {
    pub fn get(&self, var: &str) -> Option<&str> {
        self.values.get(var).map(|s| s.as_str())
    }
}

/// Host UI affordances for plugins.
#[derive(Clone)]
pub struct PluginUiHandle {
    // Filled in by core (toast/notification sink).
    pub(crate) toast_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
}

impl PluginUiHandle {
    /// Surface a toast notification in the host UI.
    pub fn toast(&self, msg: &str) {
        if let Some(tx) = &self.toast_tx {
            let _ = tx.send(msg.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn constructs_with_runtime_handle() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let handle = tokio::runtime::Handle::current();

        // Lazy client — points at a non-existent cluster but never connects.
        let url: http::Uri = "http://127.0.0.1:1".parse().expect("uri");
        let config = kube::Config::new(url);
        let client = kube::Client::try_from(config).expect("client");
        let discovery = kube::discovery::Discovery::new(client.clone());

        let ctx = PluginContext {
            kube_client: client,
            discovery,
            theme: ThemeReadHandle {
                values: std::collections::HashMap::new(),
            },
            ui: PluginUiHandle { toast_tx: None },
            runtime: handle.clone(),
        };

        // Runtime handle is usable.
        let _guard = handle.spawn(async { 1 });

        assert_eq!(ctx.theme.get("--bg-0"), None);
        assert!(ctx.ui.toast_tx.is_none());
    }
}
