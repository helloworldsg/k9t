use kube::{Client, Config, config::KubeConfigOptions};

/// Create a Kubernetes client from the default kubeconfig.
///
/// If `context_name` is provided, use that context; otherwise use the current context.
pub async fn create_client(context_name: Option<&str>) -> anyhow::Result<Client> {
    let config = match context_name {
        Some(ctx) => {
            let opts = KubeConfigOptions {
                context: Some(ctx.to_string()),
                ..Default::default()
            };
            Config::from_kubeconfig(&opts).await?
        }
        None => Config::infer().await?,
    };
    let client = Client::try_from(config)?;
    Ok(client)
}

/// Resolve the current Kubernetes context name from kubeconfig.
///
/// If `context_override` is provided (e.g. from `--context` CLI flag), returns that directly.
/// Otherwise reads the current context from the default kubeconfig.
pub async fn resolve_context_name(context_override: Option<&str>) -> anyhow::Result<String> {
    // If the user explicitly passed --context, use that
    if let Some(ctx) = context_override {
        return Ok(ctx.to_string());
    }

    // Otherwise, read the current context from kubeconfig
    let kubeconfig = kube::config::Kubeconfig::read()?;
    kubeconfig
        .current_context
        .ok_or_else(|| anyhow::anyhow!("No current context set in kubeconfig"))
}