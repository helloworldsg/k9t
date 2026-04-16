use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Event, Node, Pod, Service},
};
use kube::api::Api;

use crate::resource::ResourceType;

pub async fn describe_resource(
    client: &kube::Client,
    namespace: &str,
    resource_type: &ResourceType,
    name: &str,
) -> anyhow::Result<String> {
    let json = match resource_type {
        ResourceType::Pods => {
            let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
            let resource = api.get(name).await?;
            serde_json::to_string_pretty(&resource)?
        }
        ResourceType::Deployments => {
            let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
            let resource = api.get(name).await?;
            serde_json::to_string_pretty(&resource)?
        }
        ResourceType::Services => {
            let api: Api<Service> = Api::namespaced(client.clone(), namespace);
            let resource = api.get(name).await?;
            serde_json::to_string_pretty(&resource)?
        }
        ResourceType::Nodes => {
            let api: Api<Node> = Api::all(client.clone());
            let resource = api.get(name).await?;
            serde_json::to_string_pretty(&resource)?
        }
        ResourceType::Events => {
            let api: Api<Event> = Api::namespaced(client.clone(), namespace);
            let resource = api.get(name).await?;
            serde_json::to_string_pretty(&resource)?
        }
    };
    Ok(json)
}
