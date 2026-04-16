use futures::AsyncBufReadExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, LogParams};

pub async fn stream_pod_logs(
    client: kube::Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    tail_lines: Option<u64>,
) -> anyhow::Result<impl futures::Stream<Item = Result<String, std::io::Error>>> {
    let pods: Api<Pod> = Api::namespaced(client, namespace);

    let params = LogParams {
        follow: true,
        tail_lines: tail_lines.map(|n| n as i64),
        container: container.map(String::from),
        timestamps: true,
        ..Default::default()
    };

    let log_stream = pods.log_stream(pod_name, &params).await?;

    Ok(log_stream.lines())
}

pub async fn get_pod_logs(
    client: kube::Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    tail_lines: Option<u64>,
) -> anyhow::Result<String> {
    let pods: Api<Pod> = Api::namespaced(client, namespace);

    let params = LogParams {
        follow: false,
        tail_lines: tail_lines.map(|n| n as i64),
        container: container.map(String::from),
        timestamps: true,
        ..Default::default()
    };

    let logs = pods.logs(pod_name, &params).await?;
    Ok(logs)
}