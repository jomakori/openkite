//! Kubernetes client factory — kubeconfig loading, per-context client cache,
//! and live context switching (OKT-6).

use anyhow::{Context as _, Result};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Client, Config};
use openkite_plugin_sdk::{PluginContext, PluginUiHandle, ThemeReadHandle};
use std::collections::HashMap;

/// Active cluster connection: the kubeconfig's context list, the active
/// context, and a cached `kube::Client` per context.
///
/// Clients are cached so switching back to a previously-connected context is
/// instant; [`ClusterState::invalidate`] forces a rebuild (re-auth) on the
/// next connect.
#[derive(Default)]
pub struct ClusterState {
    /// Context names, in kubeconfig order.
    contexts: Vec<String>,
    /// Active context name.
    active: Option<String>,
    /// Cached clients, keyed by context name.
    clients: HashMap<String, Client>,
}

/// The query/switching API has no live consumer yet (OKT-7 context switcher,
/// OKT-9 reflector teardown); silence dead_code.
#[allow(dead_code)]
impl ClusterState {
    /// Load the default kubeconfig (`KUBECONFIG` env, else `~/.kube/config`).
    pub fn load() -> Result<Self> {
        let kubeconfig = Kubeconfig::read().context("read kubeconfig")?;
        Ok(Self::from_kubeconfig(kubeconfig))
    }

    /// Build from an already-parsed kubeconfig (testable, no file IO).
    pub fn from_kubeconfig(kubeconfig: Kubeconfig) -> Self {
        let contexts = kubeconfig.contexts.iter().map(|c| c.name.clone()).collect();
        Self {
            contexts,
            active: kubeconfig.current_context,
            clients: HashMap::new(),
        }
    }

    /// Context names in kubeconfig order.
    pub fn contexts(&self) -> &[String] {
        &self.contexts
    }

    /// Active context name.
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// The active client, if connected.
    pub fn client(&self) -> Option<&Client> {
        self.active.as_deref().and_then(|c| self.clients.get(c))
    }

    /// API discovery for the active client (lazy — probes nothing yet).
    pub fn discovery(&self) -> Option<kube::discovery::Discovery> {
        self.client()
            .map(|c| kube::discovery::Discovery::new(c.clone()))
    }

    /// Connect to a context: build + cache its client and mark it active.
    /// A cached client is reused on repeat switches (no re-auth).
    pub async fn connect(&mut self, context: &str) -> Result<Client> {
        if let Some(client) = self.clients.get(context) {
            self.active = Some(context.to_string());
            return Ok(client.clone());
        }
        let config = Config::from_kubeconfig(&KubeConfigOptions {
            context: Some(context.to_string()),
            ..Default::default()
        })
        .await
        .with_context(|| format!("load kubeconfig for context {context:?}"))?;
        let client = Client::try_from(config)
            .with_context(|| format!("build client for context {context:?}"))?;
        self.clients.insert(context.to_string(), client.clone());
        self.active = Some(context.to_string());
        Ok(client)
    }

    /// Drop the cached client for a context — next connect re-authenticates.
    pub fn invalidate(&mut self, context: &str) {
        self.clients.remove(context);
    }

    /// Disconnect: clear the active context (clients stay cached for fast
    /// re-switch).
    pub fn disconnect(&mut self) {
        self.active = None;
    }

    /// Verify the active cluster is reachable (cheap version probe).
    pub async fn ping(&self) -> Result<()> {
        let client = self.client().context("no active cluster")?;
        client
            .apiserver_version()
            .await
            .context("cluster unreachable")?;
        Ok(())
    }

    /// Build a `PluginContext` for the active cluster (plugin fan-out on
    /// connect).
    pub fn plugin_context(&self, runtime: tokio::runtime::Handle) -> Option<PluginContext> {
        let client = self.client()?.clone();
        let discovery = kube::discovery::Discovery::new(client.clone());
        Some(PluginContext::new(
            client,
            discovery,
            ThemeReadHandle::default(),
            PluginUiHandle::default(),
            runtime,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("openkite-cluster-{}", std::process::id()))
    }

    fn write_kubeconfig(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("config");
        std::fs::create_dir_all(dir).unwrap();
        let yaml = r#"apiVersion: v1
kind: Config
clusters:
- name: test-cluster
  cluster:
    server: http://127.0.0.1:1
contexts:
- name: test-context
  context:
    cluster: test-cluster
    user: test-user
current-context: test-context
users:
- name: test-user
  user: {}
"#;
        std::fs::write(&path, yaml).unwrap();
        path
    }

    #[test]
    fn parses_contexts_and_current_context() {
        let dir = temp_dir();
        let path = write_kubeconfig(&dir);
        let kubeconfig = Kubeconfig::read_from(&path).unwrap();
        let state = ClusterState::from_kubeconfig(kubeconfig);

        assert_eq!(state.contexts(), &["test-context".to_string()]);
        assert_eq!(state.active(), Some("test-context"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn connect_reuses_cached_client() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let url: http::Uri = "http://127.0.0.1:1".parse().unwrap();
        let client = Client::try_from(Config::new(url)).unwrap();

        let mut state = ClusterState {
            contexts: vec!["test-context".into()],
            active: None,
            clients: HashMap::from([("test-context".into(), client)]),
        };

        state.connect("test-context").await.unwrap();
        assert_eq!(
            state.clients.len(),
            1,
            "cached client must be reused, not rebuilt"
        );
        assert_eq!(state.active.as_deref(), Some("test-context"));
    }

    #[tokio::test]
    async fn ping_fails_when_unreachable() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let url: http::Uri = "http://127.0.0.1:1".parse().unwrap();
        let client = Client::try_from(Config::new(url)).unwrap();

        let state = ClusterState {
            contexts: vec!["test-context".into()],
            active: Some("test-context".into()),
            clients: HashMap::from([("test-context".into(), client)]),
        };

        assert!(
            state.ping().await.is_err(),
            "unreachable cluster must error"
        );
    }
}
