//! Workload actions: what the user asked for, and what is safe to send.
//!
//! The pure decisions live here — a typed name that must match, a drain that must
//! know what it is about to evict — so they can be tested without a cluster. The
//! calls that follow them are one thin layer over `kube`.

use std::time::Duration;

use kube::api::{
    Api, DeleteParams, DynamicObject, Patch, PatchParams, PostParams, PropagationPolicy,
};
use kube::ResourceExt;
use serde_json::json;

/// Everything that can stop an action before or during its request.
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// The apiserver refused or could not be reached.
    #[error("{0}")]
    Request(#[from] kube::Error),

    /// The operator typed something other than the object's name.
    #[error("type '{expected}' to confirm; got '{typed}'")]
    ConfirmationMismatch { typed: String, expected: String },

    /// The action needs a name and was given an empty one.
    #[error("a name is required")]
    MissingName,

    /// A request body the engine could not shape.
    #[error("{0}")]
    Malformed(String),

    /// The source object is missing the template the action needs.
    #[error("{kind} '{name}' has no {}", .missing)]
    MissingTemplate {
        kind: String,
        name: String,
        missing: String,
    },
}

/// What the apiserver does with the owned objects when a delete succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Propagation {
    /// Delete the owner first, then the owned objects.
    #[default]
    Background,
    /// Delete the owned objects first, then the owner.
    Foreground,
    /// Leave the owned objects behind.
    Orphan,
}

impl Propagation {
    fn as_kube(self) -> PropagationPolicy {
        match self {
            Propagation::Background => PropagationPolicy::Background,
            Propagation::Foreground => PropagationPolicy::Foreground,
            Propagation::Orphan => PropagationPolicy::Orphan,
        }
    }
}

/// A delete plus the two things that make it deliberate: a typed name and a policy.
#[derive(Debug, Clone)]
pub struct DeleteRequest {
    pub name: String,
    /// What the operator typed; must equal `name` exactly.
    pub typed: String,
    pub propagation: Propagation,
}

impl DeleteRequest {
    /// A request whose confirmation is the name itself.
    pub fn confirmed(name: impl Into<String>, propagation: Propagation) -> Self {
        let name = name.into();
        Self {
            typed: name.clone(),
            name,
            propagation,
        }
    }

    /// Whether the operator's confirmation matches the object they named.
    pub fn check(&self) -> Result<(), ActionError> {
        if self.name.trim().is_empty() {
            return Err(ActionError::MissingName);
        }
        if self.typed.trim() != self.name {
            return Err(ActionError::ConfirmationMismatch {
                typed: self.typed.trim().to_owned(),
                expected: self.name.clone(),
            });
        }
        Ok(())
    }
}

/// Who controls a pod, as far as a drain cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    pub kind: String,
}

/// What a drain needs to know about one pod before deciding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainTarget {
    pub name: String,
    pub namespace: String,
    pub controller: Option<Controller>,
    /// The pod has a `emptyDir` volume, so evicting it throws data away.
    pub empty_dir: bool,
    /// The pod mounts a `local` volume, which does not move with it.
    pub local_storage: bool,
    /// The pod is a static pod mirrored by the kubelet; it cannot be evicted.
    pub mirror: bool,
}

impl DrainTarget {
    /// A pod owned by a controller and holding nothing local.
    pub fn new(name: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: namespace.into(),
            controller: None,
            empty_dir: false,
            local_storage: false,
            mirror: false,
        }
    }

    /// The pod is managed by this controller kind.
    pub fn owned_by(mut self, kind: impl Into<String>) -> Self {
        self.controller = Some(Controller { kind: kind.into() });
        self
    }

    /// The pod keeps data in an `emptyDir` volume.
    pub fn with_empty_dir(mut self) -> Self {
        self.empty_dir = true;
        self
    }

    /// The pod mounts a `local` volume.
    pub fn with_local_storage(mut self) -> Self {
        self.local_storage = true;
        self
    }

    /// The pod is a static pod mirrored by the kubelet.
    pub fn as_mirror(mut self) -> Self {
        self.mirror = true;
        self
    }
}

/// The choices a drain exposes, mirroring `kubectl drain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainOptions {
    pub ignore_daemonsets: bool,
    pub delete_empty_dir_data: bool,
    pub force: bool,
    pub timeout: Duration,
}

impl Default for DrainOptions {
    fn default() -> Self {
        Self {
            ignore_daemonsets: true,
            delete_empty_dir_data: false,
            force: false,
            timeout: Duration::from_secs(120),
        }
    }
}

/// What a drain will do with one pod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainStep {
    Evict,
    Skip(SkipReason),
}

/// Why a pod is left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    DaemonSet,
    MirrorPod,
    EmptyDirData,
    LocalStorage,
}

impl SkipReason {
    /// The sentence a dialog shows next to the pod.
    pub fn explanation(self) -> &'static str {
        match self {
            SkipReason::DaemonSet => "owned by a DaemonSet; it belongs on every node",
            SkipReason::MirrorPod => "mirrored by the kubelet; only the node can remove it",
            SkipReason::EmptyDirData => "holds emptyDir data that would be lost",
            SkipReason::LocalStorage => "mounts local storage that does not follow it",
        }
    }
}

/// Decides, for every pod on the node, whether it is evicted or left alone.
///
/// Order is the cluster's order, so a dialog and the action agree on what happens first.
pub fn plan_drain(pods: &[DrainTarget], options: &DrainOptions) -> Vec<(String, DrainStep)> {
    pods.iter()
        .map(|pod| {
            let step = if pod.mirror {
                DrainStep::Skip(SkipReason::MirrorPod)
            } else if pod
                .controller
                .as_ref()
                .is_some_and(|c| c.kind == "DaemonSet" && options.ignore_daemonsets)
            {
                DrainStep::Skip(SkipReason::DaemonSet)
            } else if pod.empty_dir && !options.delete_empty_dir_data {
                DrainStep::Skip(SkipReason::EmptyDirData)
            } else if pod.local_storage && !options.force {
                DrainStep::Skip(SkipReason::LocalStorage)
            } else {
                DrainStep::Evict
            };
            (format!("{}/{}", pod.namespace, pod.name), step)
        })
        .collect()
}

/// The actions a console can take on a workload.
pub struct Actions {
    client: kube::Client,
}

impl Actions {
    /// Actions that speak through this client.
    pub fn new(client: kube::Client) -> Self {
        Self { client }
    }

    /// Deletes one object once the typed confirmation matches.
    pub async fn delete(
        &self,
        resource: &kube::api::ApiResource,
        namespace: Option<&str>,
        request: &DeleteRequest,
    ) -> Result<(), ActionError> {
        request.check()?;
        let api: Api<DynamicObject> = match namespace {
            Some(namespace) => Api::namespaced_with(self.client.clone(), namespace, resource),
            None => Api::all_with(self.client.clone(), resource),
        };
        let params = DeleteParams {
            propagation_policy: Some(request.propagation.as_kube()),
            ..DeleteParams::default()
        };
        api.delete(&request.name, &params).await?;
        Ok(())
    }

    /// Sets a workload's replica count.
    pub async fn scale(
        &self,
        resource: &kube::api::ApiResource,
        namespace: &str,
        name: &str,
        replicas: u32,
    ) -> Result<(), ActionError> {
        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), namespace, resource);
        let patch = Patch::Merge(json!({ "spec": { "replicas": replicas } }));
        api.patch(name, &PatchParams::default(), &patch).await?;
        Ok(())
    }

    /// Rolls a workload by stamping its pod template, the way `kubectl rollout restart` does.
    pub async fn restart(
        &self,
        resource: &kube::api::ApiResource,
        namespace: &str,
        name: &str,
    ) -> Result<(), ActionError> {
        let api: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), namespace, resource);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let patch = Patch::Merge(json!({
            "spec": { "template": { "metadata": { "annotations": {
                "kubectl.kubernetes.io/restartedAt": stamp.to_string()
            }}}}
        }));
        api.patch(name, &PatchParams::default(), &patch).await?;
        Ok(())
    }

    /// Starts a one-off Job from a CronJob's template, the way `kubectl create job --from` does.
    pub async fn trigger(
        &self,
        cronjob: &kube::api::ApiResource,
        namespace: &str,
        name: &str,
        job_name: &str,
    ) -> Result<String, ActionError> {
        let api: Api<DynamicObject> = Api::namespaced_with(self.client.clone(), namespace, cronjob);
        let source = api.get(name).await?;
        let template = source
            .data
            .pointer("/spec/jobTemplate/spec")
            .cloned()
            .ok_or_else(|| ActionError::MissingTemplate {
                kind: "CronJob".to_owned(),
                name: name.to_owned(),
                missing: "spec.jobTemplate.spec".to_owned(),
            })?;
        let job = json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": {
                "name": job_name,
                "namespace": namespace,
                "annotations": { "cronjob.kubernetes.io/instantiate": "manual" },
            },
            "spec": template,
        });
        let job_api: Api<DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &kube::api::ApiResource {
                group: "batch".to_owned(),
                version: "v1".to_owned(),
                api_version: "batch/v1".to_owned(),
                kind: "Job".to_owned(),
                plural: "jobs".to_owned(),
            },
        );
        let body: DynamicObject = serde_json::from_value(job)
            .map_err(|error| ActionError::Malformed(error.to_string()))?;
        let created = job_api.create(&PostParams::default(), &body).await?;
        Ok(created.name_any())
    }

    /// Marks a node unschedulable, or schedulable again.
    pub async fn cordon(&self, node: &str, unschedulable: bool) -> Result<(), ActionError> {
        let api: Api<DynamicObject> = Api::all_with(
            self.client.clone(),
            &kube::api::ApiResource {
                group: String::new(),
                version: "v1".to_owned(),
                api_version: "v1".to_owned(),
                kind: "Node".to_owned(),
                plural: "nodes".to_owned(),
            },
        );
        let patch = Patch::Merge(json!({ "spec": { "unschedulable": unschedulable } }));
        api.patch(node, &PatchParams::default(), &patch).await?;
        Ok(())
    }

    /// Evicts what the plan says to evict; the watch stream reports when the node is empty.
    pub async fn drain(
        &self,
        pods: &[DrainTarget],
        options: &DrainOptions,
    ) -> Result<Vec<(String, DrainStep)>, ActionError> {
        let plan = plan_drain(pods, options);
        for (key, step) in &plan {
            if *step != DrainStep::Evict {
                continue;
            }
            let (namespace, name) = key.split_once('/').unwrap_or_default();
            self.evict(namespace, name, options.timeout).await?;
        }
        Ok(plan)
    }

    /// Evicts one pod through the `eviction` subresource.
    async fn evict(&self, namespace: &str, name: &str, grace: Duration) -> Result<(), ActionError> {
        let api: Api<DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &kube::api::ApiResource {
                group: String::new(),
                version: "v1".to_owned(),
                api_version: "v1".to_owned(),
                kind: "Pod".to_owned(),
                plural: "pods".to_owned(),
            },
        );
        let eviction = json!({
            "apiVersion": "policy/v1",
            "kind": "Eviction",
            "metadata": { "name": name, "namespace": namespace },
            "deleteOptions": { "gracePeriodSeconds": grace.as_secs() },
        });
        let body: DynamicObject = serde_json::from_value(eviction)
            .map_err(|error| ActionError::Malformed(error.to_string()))?;
        api.create_subresource::<DynamicObject, DynamicObject>(
            "eviction",
            name,
            &PostParams::default(),
            &body,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mistyped_confirmation_stops_the_delete() {
        let request = DeleteRequest {
            name: "api-7d9".to_owned(),
            typed: "api-79d".to_owned(),
            propagation: Propagation::Background,
        };
        let error = request.check().expect_err("mismatch is refused");
        assert!(matches!(error, ActionError::ConfirmationMismatch { .. }));
    }

    #[test]
    fn the_name_typed_with_stray_spaces_still_confirms() {
        let request = DeleteRequest {
            name: "api-7d9".to_owned(),
            typed: "  api-7d9 ".to_owned(),
            propagation: Propagation::Foreground,
        };
        assert!(request.check().is_ok());
    }

    #[test]
    fn an_empty_name_is_refused_before_a_request_is_made() {
        let request = DeleteRequest::confirmed("   ", Propagation::Background);
        let error = request.check().expect_err("empty name is refused");
        assert!(matches!(error, ActionError::MissingName));
    }

    #[test]
    fn a_drain_keeps_daemonsets_and_mirror_pods() {
        let pods = vec![
            DrainTarget::new("coredns-1", "kube-system").owned_by("DaemonSet"),
            DrainTarget::new("kube-proxy-abc", "kube-system")
                .owned_by("Node")
                .as_mirror(),
            DrainTarget::new("api-7d9", "prod").owned_by("ReplicaSet"),
        ];
        let plan = plan_drain(&pods, &DrainOptions::default());

        assert_eq!(plan[0].1, DrainStep::Skip(SkipReason::DaemonSet));
        assert_eq!(plan[1].1, DrainStep::Skip(SkipReason::MirrorPod));
        assert_eq!(plan[2].1, DrainStep::Evict);
    }

    #[test]
    fn a_drain_refuses_to_drop_empty_dir_data_unless_told_to() {
        let pods = vec![DrainTarget::new("cache-1", "prod").with_empty_dir()];

        let careful = plan_drain(&pods, &DrainOptions::default());
        assert_eq!(careful[0].1, DrainStep::Skip(SkipReason::EmptyDirData));

        let reckless = plan_drain(
            &pods,
            &DrainOptions {
                delete_empty_dir_data: true,
                ..DrainOptions::default()
            },
        );
        assert_eq!(reckless[0].1, DrainStep::Evict);
    }

    #[test]
    fn a_drain_leaves_local_storage_alone_without_force() {
        let pods = vec![DrainTarget::new("db-0", "prod").with_local_storage()];
        assert_eq!(
            plan_drain(&pods, &DrainOptions::default())[0].1,
            DrainStep::Skip(SkipReason::LocalStorage)
        );
        assert_eq!(
            plan_drain(
                &pods,
                &DrainOptions {
                    force: true,
                    ..DrainOptions::default()
                }
            )[0]
            .1,
            DrainStep::Evict
        );
    }

    #[test]
    fn a_daemonset_is_evicted_when_the_operator_asks_for_it() {
        let pods = vec![DrainTarget::new("fluentbit-1", "logs").owned_by("DaemonSet")];
        assert_eq!(
            plan_drain(
                &pods,
                &DrainOptions {
                    ignore_daemonsets: false,
                    ..DrainOptions::default()
                }
            )[0]
            .1,
            DrainStep::Evict
        );
    }
}
