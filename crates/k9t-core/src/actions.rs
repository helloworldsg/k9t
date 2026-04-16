use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::Node,
};
use kube::api::{Api, DeleteParams, Patch, PatchParams};

pub async fn delete_pod(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> anyhow::Result<()> {
    let pods: Api<k8s_openapi::api::core::v1::Pod> =
        Api::namespaced(client.clone(), namespace);
    pods.delete(name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn restart_deployment(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> anyhow::Result<()> {
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let now = jiff::Timestamp::now().to_string();
    let patch = serde_json::json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": now
                    }
                }
            }
        }
    });
    deployments
        .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

pub async fn scale_deployment(
    client: &kube::Client,
    namespace: &str,
    name: &str,
    replicas: i32,
) -> anyhow::Result<()> {
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let patch = serde_json::json!({
        "spec": {
            "replicas": replicas
        }
    });
    deployments
        .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

pub async fn cordon_node(client: &kube::Client, name: &str) -> anyhow::Result<()> {
    let nodes: Api<Node> = Api::all(client.clone());
    let patch = serde_json::json!({
        "spec": {
            "unschedulable": true
        }
    });
    nodes
        .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

pub async fn drain_node(client: &kube::Client, name: &str) -> anyhow::Result<()> {
    cordon_node(client, name).await
}