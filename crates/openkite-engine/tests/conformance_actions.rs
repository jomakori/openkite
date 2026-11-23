//! Conformance of the workload actions against a live cluster.
//!
//! Each row sends the request a console would send and then reads the cluster back:
//! scale changes `spec.replicas`, restart stamps the pod template, a CronJob trigger
//! leaves a Job behind, cordon flips `spec.unschedulable`, drain replaces the pods it
//! planned to evict, and a mistyped delete is refused while the object stays.
//!
//! The test needs a live cluster, so it is ignored by default and un-ignored by the
//! conformance workflow, which brings up a kind cluster first.
use std::time::{Duration, Instant};

use kube::api::{Api, ApiResource, DeleteParams, DynamicObject, ListParams, PostParams};
use kube::Client;
use openkite_engine::actions::{
    ActionError, Actions, DeleteRequest, DrainOptions, DrainStep, DrainTarget, Propagation,
};
use tokio::time::sleep;

/// Namespace the fixtures live in; created and removed by the test.
const NAMESPACE: &str = "okt-actions-conformance";
/// Ceiling for one wait, so a wedged cluster fails the row instead of hanging it.
const ROW_DEADLINE: Duration = Duration::from_secs(180);

fn resource(group: &str, version: &str, kind: &str, plural: &str) -> ApiResource {
    ApiResource {
        group: group.to_owned(),
        version: version.to_owned(),
        api_version: if group.is_empty() {
            version.to_owned()
        } else {
            format!("{group}/{version}")
        },
        kind: kind.to_owned(),
        plural: plural.to_owned(),
    }
}

fn deployments() -> ApiResource {
    resource("apps", "v1", "Deployment", "deployments")
}

fn pods() -> ApiResource {
    resource("", "v1", "Pod", "pods")
}

fn cronjobs() -> ApiResource {
    resource("batch", "v1", "CronJob", "cronjobs")
}

fn jobs() -> ApiResource {
    resource("batch", "v1", "Job", "jobs")
}

async fn until<F, Fut>(what: &str, mut check: F) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let deadline = Instant::now() + ROW_DEADLINE;
    loop {
        match check().await? {
            true => return Ok(()),
            false if Instant::now() >= deadline => {
                return Err(format!("timed out waiting for {what}"))
            }
            false => sleep(Duration::from_secs(2)).await,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a live cluster; run by the conformance workflow"]
async fn every_action_reaches_the_cluster() -> Result<(), String> {
    let client = Client::try_default().await.map_err(|e| e.to_string())?;
    let actions = Actions::new(client.clone());
    let namespaces: Api<DynamicObject> = Api::all_with(
        client.clone(),
        &resource("", "v1", "Namespace", "namespaces"),
    );
    let deployments_api: Api<DynamicObject> =
        Api::namespaced_with(client.clone(), NAMESPACE, &deployments());
    let pods_api: Api<DynamicObject> = Api::namespaced_with(client.clone(), NAMESPACE, &pods());
    let cronjobs_api: Api<DynamicObject> =
        Api::namespaced_with(client.clone(), NAMESPACE, &cronjobs());
    let jobs_api: Api<DynamicObject> = Api::namespaced_with(client.clone(), NAMESPACE, &jobs());

    // A namespace of our own, so nothing outside the test is touched.
    let body: DynamicObject = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": { "name": NAMESPACE },
    }))
    .map_err(|e| e.to_string())?;
    // A previous run's namespace may still be terminating; wait for it before reusing the name.
    if namespaces.get(NAMESPACE).await.is_ok() {
        let _ = namespaces.delete(NAMESPACE, &DeleteParams::default()).await;
        until("the previous namespace to finish deleting", || async {
            Ok(namespaces.get(NAMESPACE).await.is_err())
        })
        .await?;
    }
    namespaces
        .create(&PostParams::default(), &body)
        .await
        .map_err(|e| format!("create namespace: {e}"))?;

    let result = rows(
        &client,
        &actions,
        &deployments_api,
        &pods_api,
        &cronjobs_api,
        &jobs_api,
    )
    .await;

    // The fixtures go regardless of the outcome; the namespace takes everything with it.
    let _ = namespaces.delete(NAMESPACE, &DeleteParams::default()).await;
    result
}

async fn rows(
    client: &Client,
    actions: &Actions,
    deployments_api: &Api<DynamicObject>,
    pods_api: &Api<DynamicObject>,
    cronjobs_api: &Api<DynamicObject>,
    jobs_api: &Api<DynamicObject>,
) -> Result<(), String> {
    let deployment: DynamicObject = serde_json::from_value(serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": "web", "namespace": NAMESPACE },
        "spec": {
            "replicas": 1,
            "selector": { "matchLabels": { "app": "web" } },
            "template": {
                "metadata": { "labels": { "app": "web" } },
                "spec": {
                    "containers": [{
                        "name": "web",
                        "image": "busybox:1.36",
                        "command": ["sleep", "3600"],
                    }],
                },
            },
        },
    }))
    .map_err(|e| e.to_string())?;
    deployments_api
        .create(&PostParams::default(), &deployment)
        .await
        .map_err(|e| format!("create deployment: {e}"))?;
    until("the deployment to become ready", || async {
        let current = deployments_api
            .get("web")
            .await
            .map_err(|e| e.to_string())?;
        let ready = current
            .data
            .pointer("/status/availableReplicas")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        Ok(ready >= 1)
    })
    .await?;

    // Scale: the replica count the console asked for is the one the cluster holds.
    actions
        .scale(&deployments(), NAMESPACE, "web", 2)
        .await
        .map_err(|e| format!("scale: {e}"))?;
    until("the cluster to hold two replicas", || async {
        let current = deployments_api
            .get("web")
            .await
            .map_err(|e| e.to_string())?;
        let replicas = current
            .data
            .pointer("/spec/replicas")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        Ok(replicas == 2)
    })
    .await?;

    // Restart: the stamp `kubectl rollout restart` writes is on the pod template.
    actions
        .restart(&deployments(), NAMESPACE, "web")
        .await
        .map_err(|e| format!("restart: {e}"))?;
    let stamped = deployments_api
        .get("web")
        .await
        .map_err(|e| e.to_string())?;
    if stamped
        .data
        .pointer("/spec/template/metadata/annotations")
        .and_then(|annotations| annotations.get("kubectl.kubernetes.io/restartedAt"))
        .is_none()
    {
        return Err("restart left no stamp on the pod template".to_owned());
    }

    // A CronJob, then the one-off Job a "trigger" asks for.
    let cronjob: DynamicObject = serde_json::from_value(serde_json::json!({
        "apiVersion": "batch/v1",
        "kind": "CronJob",
        "metadata": { "name": "tick", "namespace": NAMESPACE },
        "spec": {
            "schedule": "*/5 * * * *",
            "jobTemplate": {
                "spec": {
                    "template": {
                        "spec": {
                            "restartPolicy": "Never",
                            "containers": [{
                                "name": "tick",
                                "image": "busybox:1.36",
                                "command": ["true"],
                            }],
                        },
                    },
                },
            },
        },
    }))
    .map_err(|e| e.to_string())?;
    cronjobs_api
        .create(&PostParams::default(), &cronjob)
        .await
        .map_err(|e| format!("create cronjob: {e}"))?;
    let triggered = actions
        .trigger(&cronjobs(), NAMESPACE, "tick", "tick-manual")
        .await
        .map_err(|e| format!("trigger: {e}"))?;
    jobs_api
        .get(&triggered)
        .await
        .map_err(|e| format!("the triggered job is not there: {e}"))?;

    // Cordon: the node refuses new pods, then takes them again.
    let node_name = pods_api
        .list(&ListParams::default())
        .await
        .map_err(|e| e.to_string())?
        .items
        .iter()
        .find_map(|pod| {
            pod.data
                .pointer("/spec/nodeName")?
                .as_str()
                .map(str::to_owned)
        })
        .ok_or_else(|| "no pod on a node to cordon".to_owned())?;
    let nodes: Api<DynamicObject> =
        Api::all_with(client.clone(), &resource("", "v1", "Node", "nodes"));
    actions
        .cordon(&node_name, true)
        .await
        .map_err(|e| format!("cordon: {e}"))?;
    let node = nodes.get(&node_name).await.map_err(|e| e.to_string())?;
    if node.data.pointer("/spec/unschedulable") != Some(&serde_json::Value::Bool(true)) {
        return Err("cordon did not mark the node unschedulable".to_owned());
    }
    actions
        .cordon(&node_name, false)
        .await
        .map_err(|e| format!("uncordon: {e}"))?;

    // Drain: the pods we name are evicted, and their replacements appear.
    // Only the Deployment's pods: the triggered Job's pod is meant to outlive a drain.
    let params = ListParams::default().labels("app=web");
    let before: Vec<String> = pods_api
        .list(&params)
        .await
        .map_err(|e| e.to_string())?
        .items
        .iter()
        .filter(|pod| pod.metadata.deletion_timestamp.is_none())
        .map(|pod| pod.metadata.name.clone().unwrap_or_default())
        .collect();
    if before.is_empty() {
        return Err("no pods to drain".to_owned());
    }
    let targets: Vec<DrainTarget> = before
        .iter()
        .map(|name| DrainTarget::new(name.clone(), NAMESPACE).owned_by("ReplicaSet"))
        .collect();
    let plan = actions
        .drain(&targets, &DrainOptions::default())
        .await
        .map_err(|e| format!("drain: {e}"))?;
    if plan.iter().any(|(_, step)| *step != DrainStep::Evict) {
        return Err(format!(
            "drain skipped a pod it should have evicted: {plan:?}"
        ));
    }
    let deadline = Instant::now() + ROW_DEADLINE;
    loop {
        let pods = pods_api.list(&params).await.map_err(|e| e.to_string())?;
        let live: Vec<String> = pods
            .items
            .iter()
            .filter(|pod| pod.metadata.deletion_timestamp.is_none())
            .map(|pod| pod.metadata.name.clone().unwrap_or_default())
            .collect();
        if live.iter().filter(|name| !before.contains(name)).count() >= before.len() {
            break;
        }
        if Instant::now() >= deadline {
            let seen: Vec<String> = pods
                .items
                .iter()
                .map(|pod| {
                    let phase = pod
                        .data
                        .pointer("/status/phase")
                        .and_then(|phase| phase.as_str())
                        .unwrap_or("?");
                    format!("{}/{phase}", pod.metadata.name.clone().unwrap_or_default())
                })
                .collect();
            return Err(format!(
                "drained pods were not replaced; drained {before:?}, pods now {seen:?}"
            ));
        }
        sleep(Duration::from_secs(2)).await;
    }

    // Delete: a typo never reaches the apiserver, the right name removes the workload.
    let typo = DeleteRequest {
        name: "web".to_owned(),
        typed: "wbe".to_owned(),
        propagation: Propagation::Foreground,
    };
    match actions.delete(&deployments(), Some(NAMESPACE), &typo).await {
        Err(ActionError::ConfirmationMismatch { .. }) => {}
        other => return Err(format!("a mistyped delete was not refused: {other:?}")),
    }
    deployments_api
        .get("web")
        .await
        .map_err(|e| format!("the refused delete removed the workload: {e}"))?;
    let confirmed = DeleteRequest::confirmed("web", Propagation::Foreground);
    actions
        .delete(&deployments(), Some(NAMESPACE), &confirmed)
        .await
        .map_err(|e| format!("delete: {e}"))?;
    // Foreground deletion is asynchronous: the object is marked and then collected.
    until("the confirmed delete to mark the deployment", || async {
        match deployments_api.get("web").await {
            Err(kube::Error::Api(response)) if response.code == 404 => Ok(true),
            Ok(current) => Ok(current.metadata.deletion_timestamp.is_some()),
            Err(e) => Err(format!("delete read back: {e}")),
        }
    })
    .await?;
    Ok(())
}
