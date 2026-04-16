use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams, AttachedProcess};

pub struct ShellSession {
    attached: Option<AttachedProcess>,
}

impl ShellSession {
    pub async fn exec(
        client: kube::Client,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        command: Vec<String>,
    ) -> anyhow::Result<Self> {
        let pods: Api<Pod> = Api::namespaced(client, namespace);

        let mut params = AttachParams::default()
            .stdin(true)
            .stdout(true)
            .stderr(true)
            .tty(true);

        if let Some(c) = container {
            params = params.container(c);
        }

        let attached = pods
            .exec(pod_name, command, &params)
            .await?;

        Ok(Self { attached: Some(attached) })
    }

    pub fn stdout(&mut self) -> Option<impl tokio::io::AsyncRead + Unpin> {
        self.attached.as_mut().and_then(|a| a.stdout())
    }

    pub fn stderr(&mut self) -> Option<impl tokio::io::AsyncRead + Unpin> {
        self.attached.as_mut().and_then(|a| a.stderr())
    }

    pub fn stdin(&mut self) -> Option<impl tokio::io::AsyncWrite + Unpin> {
        self.attached.as_mut().and_then(|a| a.stdin())
    }

    pub async fn join(&mut self) -> anyhow::Result<()> {
        if let Some(attached) = self.attached.take() {
            attached.join().await?;
        }
        Ok(())
    }
}