//! Collection engine: what to watch, what to keep, and the deltas leaving it.
pub mod auth;
pub mod catalogue;
pub mod context;
pub mod discovery;
pub mod error;
pub mod kubeconfig;

use std::sync::Arc;

use kube::config::{Config, Kubeconfig};
use kube::Client;

use crate::auth::AuthKind;
use crate::catalogue::{CatalogueCache, CatalogueSnapshot};
use crate::context::ContextSwitcher;
use crate::error::EngineError;

/// The context name reported when credentials come from the pod environment.
const IN_CLUSTER_CONTEXT: &str = "in-cluster";

/// A connected engine: a kube client together with the context and auth kind it was built from.
pub struct Engine {
    client: Client,
    context: String,
    auth: AuthKind,
    catalogue: CatalogueCache,
}

impl Engine {
    /// Connect with in-cluster credentials when present, otherwise with the given or current context.
    pub async fn connect(context: Option<&str>) -> Result<Self, EngineError> {
        if auth::is_in_cluster() {
            let config = Config::incluster()?;
            let client = Client::try_from(config)?;
            return Ok(Self {
                client,
                context: IN_CLUSTER_CONTEXT.to_owned(),
                auth: AuthKind::InCluster,
                catalogue: CatalogueCache::new(),
            });
        }

        let kubeconfig = kubeconfig::load()?;
        let switcher = ContextSwitcher::new(kubeconfig.clone());
        let name = switcher.canonical_name(context)?;
        let config = switcher.resolve(context).await?;
        let auth = auth_kind_for(&kubeconfig, &name);
        let client = Client::try_from(config)?;
        Ok(Self {
            client,
            context: name,
            auth,
            catalogue: CatalogueCache::new(),
        })
    }

    /// The client every request this engine makes goes through.
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    /// The context the client was built from.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// The credential mechanism the context uses.
    pub fn auth_kind(&self) -> &AuthKind {
        &self.auth
    }

    /// The served API surface for this context, refetched when the cached copy is stale.
    pub async fn catalogue(&self) -> Result<Arc<CatalogueSnapshot>, EngineError> {
        self.catalogue
            .get_or_refresh(&self.context, &self.client)
            .await
    }

    /// Drop the cached API surface so the next `catalogue` call fetches again.
    pub async fn invalidate_catalogue(&self) {
        self.catalogue.invalidate(&self.context).await;
    }
}

/// Classify the auth info the named context points at, `None` when it names no known user.
fn auth_kind_for(config: &Kubeconfig, context_name: &str) -> AuthKind {
    let user_name = config
        .contexts
        .iter()
        .find(|named| named.name == context_name)
        .and_then(|named| named.context.as_ref())
        .and_then(|context| context.user.as_deref());

    let Some(user_name) = user_name else {
        return AuthKind::None;
    };

    config
        .auth_infos
        .iter()
        .find(|named| named.name == user_name)
        .and_then(|named| named.auth_info.as_ref())
        .map(auth::classify)
        .unwrap_or(AuthKind::None)
}

#[cfg(test)]
mod tests {
    use kube::config::{AuthInfo, Cluster, Context, NamedAuthInfo, NamedCluster, NamedContext};

    use super::*;

    fn fixture(token_file: Option<&str>) -> Kubeconfig {
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
                auth_info: Some(AuthInfo {
                    token_file: token_file.map(str::to_owned),
                    ..Default::default()
                }),
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
    fn auth_kind_follows_the_context_user() {
        assert_eq!(
            auth_kind_for(&fixture(Some("abc")), "alpha"),
            AuthKind::StaticToken
        );
    }

    #[test]
    fn auth_kind_is_none_without_a_user() {
        let mut config = fixture(None);
        config.contexts[0].context.as_mut().unwrap().user = None;
        assert_eq!(auth_kind_for(&config, "alpha"), AuthKind::None);
    }

    #[test]
    fn auth_kind_is_none_for_an_unknown_context() {
        assert_eq!(
            auth_kind_for(&fixture(Some("abc")), "ghost"),
            AuthKind::None
        );
    }
}
