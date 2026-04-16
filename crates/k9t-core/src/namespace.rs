use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, ResourceExt};

pub async fn discover_namespaces(client: &Client) -> anyhow::Result<Vec<String>> {
    let api: Api<Namespace> = Api::all(client.clone());
    let namespaces = api.list(&Default::default()).await?;
    let mut names: Vec<String> = namespaces.iter().map(|ns| ns.name_any()).collect();
    names.sort();
    Ok(names)
}

/// Discover all kubeconfig context names from the default kubeconfig file.
pub fn discover_contexts() -> anyhow::Result<Vec<String>> {
    let kubeconfig = kube::config::Kubeconfig::read()?;
    let mut contexts: Vec<String> = kubeconfig
        .contexts
        .iter()
        .map(|ctx| ctx.name.clone())
        .collect();
    contexts.sort();
    Ok(contexts)
}