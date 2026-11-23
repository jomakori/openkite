//! Authorization checks: what the connected identity may do to a resource.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use kube::api::{Api, PostParams};
use kube::Client;
use thiserror::Error;

/// How long a review answer is reused before it is asked again.
pub const DEFAULT_TTL: Duration = Duration::from_secs(300);

/// A review request that could not be answered.
#[derive(Debug, Error)]
pub enum AccessError {
    /// The apiserver rejected or failed the review request.
    #[error("authorization review failed: {0}")]
    Request(#[from] kube::Error),

    /// The apiserver answered without a decision.
    #[error("authorization review returned no decision")]
    NoDecision,
}

/// The five verbs a screen asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Verb {
    /// Read one object.
    Get,
    /// Read a collection.
    List,
    /// Create an object.
    Create,
    /// Modify an object.
    Update,
    /// Remove an object.
    Delete,
}

impl Verb {
    /// The verb as the apiserver spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::List => "list",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    /// Every verb, in the order a capability descriptor reports them.
    pub fn all() -> [Self; 5] {
        [
            Self::Get,
            Self::List,
            Self::Create,
            Self::Update,
            Self::Delete,
        ]
    }
}

/// One (verb, resource) question, scoped to a namespace when the resource is namespaced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Review {
    /// The verb being asked about.
    pub verb: Verb,
    /// API group, empty for the core group.
    pub group: String,
    /// Plural resource name, e.g. `deployments`.
    pub resource: String,
    /// Namespace the question applies to, `None` for cluster scope.
    pub namespace: Option<String>,
}

impl Review {
    /// A cluster-scoped question.
    pub fn new(verb: Verb, group: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            verb,
            group: group.into(),
            resource: resource.into(),
            namespace: None,
        }
    }

    /// The same question asked inside a namespace.
    pub fn in_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }
}

/// One answer from the apiserver: allowed, with the reason on a denial.
#[derive(Clone, Debug)]
pub(crate) struct Decision {
    pub(crate) allowed: bool,
    pub(crate) reason: Option<String>,
}

/// What the identity may do to one resource, with the apiserver's reason on every denial.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    decisions: HashMap<Verb, (bool, Option<String>)>,
}

impl Capabilities {
    /// Whether the verb is allowed; an unanswered verb is not allowed.
    pub fn allows(&self, verb: Verb) -> bool {
        matches!(self.decisions.get(&verb), Some((true, _)))
    }

    /// Why the verb was denied, when the apiserver gave a reason.
    pub fn reason(&self, verb: Verb) -> Option<&str> {
        self.decisions
            .get(&verb)
            .and_then(|(_, reason)| reason.as_deref())
    }

    /// Whether one object can be read.
    pub fn can_get(&self) -> bool {
        self.allows(Verb::Get)
    }

    /// Whether the collection can be read.
    pub fn can_list(&self) -> bool {
        self.allows(Verb::List)
    }

    /// Whether an object can be created.
    pub fn can_create(&self) -> bool {
        self.allows(Verb::Create)
    }

    /// Whether an object can be modified.
    pub fn can_update(&self) -> bool {
        self.allows(Verb::Update)
    }

    /// Whether an object can be removed.
    pub fn can_delete(&self) -> bool {
        self.allows(Verb::Delete)
    }

    /// Whether any write verb is allowed.
    pub fn can_write(&self) -> bool {
        self.can_create() || self.can_update() || self.can_delete()
    }
}

/// Ask the apiserver what the current identity may do, reusing answers for `ttl`.
pub struct Gate {
    ttl: Duration,
    cache: Mutex<HashMap<Review, (Decision, Instant)>>,
}

impl Gate {
    /// A cache that reuses each answer for `ttl`.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// A cache with the default lifetime.
    pub fn with_default_ttl() -> Self {
        Self::new(DEFAULT_TTL)
    }

    /// Drop every cached answer.
    pub fn invalidate(&self) {
        self.cache.lock().expect("authorization cache").clear();
    }

    /// Answer one question, from the cache when the answer is still fresh.
    pub async fn review(
        &self,
        client: &Client,
        review: &Review,
    ) -> Result<(bool, Option<String>), AccessError> {
        if let Some(hit) = self.cached(review) {
            return Ok((hit.allowed, hit.reason));
        }

        let attributes = ResourceAttributes {
            verb: Some(review.verb.as_str().to_owned()),
            group: Some(review.group.clone()),
            resource: Some(review.resource.clone()),
            namespace: review.namespace.clone(),
            ..Default::default()
        };
        let request = SelfSubjectAccessReview {
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(attributes),
                ..Default::default()
            },
            ..Default::default()
        };

        let api: Api<SelfSubjectAccessReview> = Api::all(client.clone());
        let answer = api.create(&PostParams::default(), &request).await?;
        let status = answer.status.ok_or(AccessError::NoDecision)?;
        let decision = Decision {
            allowed: status.allowed,
            reason: status.reason,
        };

        self.cache
            .lock()
            .expect("authorization cache")
            .insert(review.clone(), (decision.clone(), Instant::now()));
        Ok((decision.allowed, decision.reason))
    }

    /// Whether the verb is allowed.
    pub async fn allows(&self, client: &Client, review: &Review) -> Result<bool, AccessError> {
        Ok(self.review(client, review).await?.0)
    }

    /// Every verb's answer for one resource, asked in one pass.
    pub async fn capabilities(
        &self,
        client: &Client,
        group: &str,
        resource: &str,
        namespace: Option<&str>,
    ) -> Result<Capabilities, AccessError> {
        let mut capabilities = Capabilities::default();
        for verb in Verb::all() {
            let mut review = Review::new(verb, group, resource);
            if let Some(namespace) = namespace {
                review = review.in_namespace(namespace);
            }
            let (allowed, reason) = self.review(client, &review).await?;
            capabilities.decisions.insert(verb, (allowed, reason));
        }
        Ok(capabilities)
    }

    fn cached(&self, review: &Review) -> Option<Decision> {
        let cache = self.cache.lock().expect("authorization cache");
        let (decision, asked) = cache.get(review)?;
        (asked.elapsed() < self.ttl).then(|| decision.clone())
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::with_default_ttl()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_report_each_verb() {
        let mut capabilities = Capabilities::default();
        capabilities.decisions.insert(Verb::List, (true, None));
        capabilities.decisions.insert(
            Verb::Delete,
            (false, Some("pods \"web\" is forbidden".to_owned())),
        );

        assert!(capabilities.can_list());
        assert!(!capabilities.can_delete());
        assert_eq!(
            capabilities.reason(Verb::Delete),
            Some("pods \"web\" is forbidden")
        );
        assert!(!capabilities.can_write());
    }

    #[test]
    fn a_namespaced_review_is_a_distinct_question() {
        let cluster = Review::new(Verb::Get, "", "pods");
        let namespaced = cluster.clone().in_namespace("demo");
        assert_ne!(cluster, namespaced);
        assert_eq!(namespaced.namespace.as_deref(), Some("demo"));
    }
}
