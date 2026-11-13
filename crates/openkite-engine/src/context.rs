//! Context resolution: pick a named context from a kubeconfig and build its `kube::Config`.
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::Config;

use crate::error::EngineError;

/// Holds a loaded kubeconfig and resolves named contexts against it on demand.
#[derive(Debug, Clone)]
pub struct ContextSwitcher {
    kubeconfig: Kubeconfig,
}

impl ContextSwitcher {
    /// Wrap an already-loaded kubeconfig for context resolution.
    pub fn new(kubeconfig: Kubeconfig) -> Self {
        Self { kubeconfig }
    }

    /// The context name to use: the given name, or the kubeconfig's `current-context`.
    pub fn canonical_name(&self, name: Option<&str>) -> Result<String, EngineError> {
        match name {
            Some(n) => Ok(n.to_owned()),
            None => self
                .kubeconfig
                .current_context
                .clone()
                .ok_or(EngineError::CurrentContextNotSet),
        }
    }

    /// Resolve `name` (or the current context) against the kubeconfig and build its `Config`.
    pub async fn resolve(&self, name: Option<&str>) -> Result<Config, EngineError> {
        let context_name = self.canonical_name(name)?;
        validate(&self.kubeconfig, &context_name)?;

        let options = KubeConfigOptions {
            context: Some(context_name),
            cluster: None,
            user: None,
        };
        Config::from_custom_kubeconfig(self.kubeconfig.clone(), &options)
            .await
            .map_err(EngineError::from_config_error)
    }
}

fn validate(config: &Kubeconfig, context_name: &str) -> Result<(), EngineError> {
    let context = config
        .contexts
        .iter()
        .find(|named| named.name == context_name)
        .and_then(|named| named.context.as_ref())
        .ok_or_else(|| EngineError::UnknownContext(context_name.to_owned()))?;

    let cluster_known = config
        .clusters
        .iter()
        .any(|named| named.name == context.cluster);
    if !cluster_known {
        return Err(EngineError::MissingClusterRef {
            context: context_name.to_owned(),
            cluster: context.cluster.clone(),
        });
    }

    if let Some(user_name) = &context.user {
        let user_known = config
            .auth_infos
            .iter()
            .any(|named| &named.name == user_name);
        if !user_known {
            return Err(EngineError::MissingUserRef {
                context: context_name.to_owned(),
                user: user_name.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use kube::config::{Cluster, Context, NamedAuthInfo, NamedCluster, NamedContext};

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
            auth_infos: vec![NamedAuthInfo {
                name: "alpha-user".to_owned(),
                auth_info: Some(Default::default()),
                other: Default::default(),
            }],
            contexts: vec![NamedContext {
                name: "alpha".to_owned(),
                context: Some(Context {
                    cluster: "alpha-cluster".to_owned(),
                    user: Some("alpha-user".to_owned()),
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
    fn canonical_name_falls_back_to_current_context() {
        let switcher = ContextSwitcher::new(fixture());
        assert_eq!(switcher.canonical_name(None).unwrap(), "alpha");
    }

    #[test]
    fn canonical_name_fails_typed_without_current_context() {
        let mut config = fixture();
        config.current_context = None;
        let switcher = ContextSwitcher::new(config);
        assert!(matches!(
            switcher.canonical_name(None),
            Err(EngineError::CurrentContextNotSet)
        ));
    }

    #[test]
    fn validate_accepts_fully_defined_context() {
        assert!(validate(&fixture(), "alpha").is_ok());
    }

    #[test]
    fn validate_fails_typed_for_unknown_context() {
        let err = validate(&fixture(), "ghost").unwrap_err();
        assert!(matches!(err, EngineError::UnknownContext(name) if name == "ghost"));
    }

    #[test]
    fn validate_fails_typed_for_missing_cluster_ref() {
        let mut config = fixture();
        config.clusters.clear();
        let err = validate(&config, "alpha").unwrap_err();
        assert!(matches!(
            err,
            EngineError::MissingClusterRef { cluster, .. } if cluster == "alpha-cluster"
        ));
    }

    #[test]
    fn validate_fails_typed_for_missing_user_ref() {
        let mut config = fixture();
        config.auth_infos.clear();
        let err = validate(&config, "alpha").unwrap_err();
        assert!(matches!(
            err,
            EngineError::MissingUserRef { user, .. } if user == "alpha-user"
        ));
    }

    #[tokio::test]
    async fn resolve_builds_config_for_named_context() {
        let switcher = ContextSwitcher::new(fixture());
        let config = switcher.resolve(Some("alpha")).await.expect("resolves");
        assert_eq!(
            config.cluster_url.to_string(),
            "https://example.invalid:6443/"
        );
    }
}
