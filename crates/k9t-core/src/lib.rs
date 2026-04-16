pub mod actions;
pub mod client;
pub mod describe;
pub mod logs;
pub mod namespace;
pub mod port_forward;
pub mod reflector;
pub mod resource;
pub mod shell;

pub use actions::{cordon_node, delete_pod, drain_node, restart_deployment, scale_deployment};
pub use client::{create_client, resolve_context_name};
pub use describe::describe_resource;
pub use logs::{get_pod_logs, stream_pod_logs};
pub use namespace::{discover_contexts, discover_namespaces};
pub use port_forward::PortForward;
pub use reflector::K9sReflector;
pub use reflector::PodReflector;
pub use resource::{
    ContainerDetail, ContainerPortInfo, DeploymentInfo, EventInfo, NodeInfo, PodInfo, ResourceType,
    ServiceInfo,
};
pub use shell::ShellSession;
