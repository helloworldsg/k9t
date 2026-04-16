use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;

pub struct PortForward {
    pod_name: String,
    namespace: String,
    local_port: u16,
    remote_port: u16,
}

impl PortForward {
    pub fn new(
        namespace: String,
        pod_name: String,
        local_port: u16,
        remote_port: u16,
    ) -> Self {
        Self {
            pod_name,
            namespace,
            local_port,
            remote_port,
        }
    }

    pub fn pod_name(&self) -> &str {
        &self.pod_name
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn remote_port(&self) -> u16 {
        self.remote_port
    }

    pub async fn connect(
        &self,
        client: kube::Client,
    ) -> anyhow::Result<kube::api::Portforwarder> {
        let pods: Api<Pod> = Api::namespaced(client, &self.namespace);
        let pf = pods
            .portforward(&self.pod_name, &[self.remote_port])
            .await?;
        Ok(pf)
    }
}