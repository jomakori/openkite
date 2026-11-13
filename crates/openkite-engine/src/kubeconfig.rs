//! Locate, load, and merge kubeconfig files from `KUBECONFIG` or the default path.
use kube::config::Kubeconfig;

use crate::error::EngineError;

/// A named context's cluster and server, without resolving auth.
#[derive(Debug, Clone)]
pub struct ContextSummary {
    pub name: String,
    pub cluster: String,
    pub server: Option<String>,
}

/// Load and merge kubeconfig files per `KUBECONFIG` (colon-separated, first file wins on
/// conflicting keys) or fall back to `~/.kube/config`.
pub fn load() -> Result<Kubeconfig, EngineError> {
    Kubeconfig::read().map_err(EngineError::from_config_error)
}

/// List every context defined in a loaded kubeconfig, with its cluster and server.
pub fn list_contexts(config: &Kubeconfig) -> Vec<ContextSummary> {
    config
        .contexts
        .iter()
        .filter_map(|named| {
            let ctx = named.context.as_ref()?;
            let server = config
                .clusters
                .iter()
                .find(|named_cluster| named_cluster.name == ctx.cluster)
                .and_then(|named_cluster| named_cluster.cluster.as_ref())
                .and_then(|cluster| cluster.server.clone());
            Some(ContextSummary {
                name: named.name.clone(),
                cluster: ctx.cluster.clone(),
                server,
            })
        })
        .collect()
}

/// The kubeconfig's `current-context`, if set.
pub fn current_context(config: &Kubeconfig) -> Option<&str> {
    config.current_context.as_deref()
}

#[cfg(test)]
mod tests {
    use kube::config::{Cluster, Context, NamedCluster, NamedContext};

    use super::*;

    fn fixture() -> Kubeconfig {
        Kubeconfig {
            current_context: Some("alpha".to_owned()),
            clusters: vec![NamedCluster {
                name: "alpha-cluster".to_owned(),
                cluster: Some(Cluster {
                    server: Some("https://example.invalid:6443".to_owned()),
                    ..Default::default()
                }),
                other: Default::default(),
            }],
            contexts: vec![NamedContext {
                name: "alpha".to_owned(),
                context: Some(Context {
                    cluster: "alpha-cluster".to_owned(),
                    user: None,
                    namespace: None,
                    extensions: None,
                    other: Default::default(),
                }),
                other: Default::default(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn list_contexts_resolves_server_from_cluster_ref() {
        let summaries = list_contexts(&fixture());
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "alpha");
        assert_eq!(summaries[0].cluster, "alpha-cluster");
        assert_eq!(
            summaries[0].server.as_deref(),
            Some("https://example.invalid:6443")
        );
    }

    #[test]
    fn current_context_reads_through() {
        assert_eq!(current_context(&fixture()), Some("alpha"));
    }
}
