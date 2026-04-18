use std::sync::Arc;

use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Event, Node, Pod, Service},
};
use kube::ResourceExt;

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Pods,
    Deployments,
    Services,
    Nodes,
    Events,
}

impl ResourceType {
    pub fn title(&self) -> &'static str {
        match self {
            ResourceType::Pods => "Pods",
            ResourceType::Deployments => "Deployments",
            ResourceType::Services => "Services",
            ResourceType::Nodes => "Nodes",
            ResourceType::Events => "Events",
        }
    }
}

/// A simplified container port representation for display and port-forward suggestions.
#[derive(Debug, Clone)]
pub struct ContainerPortInfo {
    /// Port number (container_port from the spec).
    pub port: u16,
    /// Port name, if specified.
    pub name: Option<String>,
    /// Protocol (TCP, UDP, SCTP). Defaults to TCP.
    pub protocol: String,
}

impl std::fmt::Display for ContainerPortInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{}({}/{})", self.port, self.protocol, name),
            None => write!(f, "{}", self.port),
        }
    }
}

/// A simplified volume mount representation.
#[derive(Debug, Clone)]
pub struct VolumeMount {
    /// Name of the volume being mounted.
    pub name: String,
    /// Path where the volume is mounted inside the container.
    pub mount_path: String,
    /// Whether the volume is mounted read-only.
    pub read_only: bool,
}

// ContainerDetail — per-container status extracted from pod.status.container_statuses

#[derive(Debug, Clone)]
pub struct ContainerDetail {
    pub name: String,
    pub ready: bool,
    pub restart_count: u32,
    /// Current state: "Running", "Waiting", "Terminated", or "Unknown"
    pub status: String,
    /// Human-readable reason/message if the container is waiting or terminated
    pub reason: String,
    /// Container image
    pub image: String,
    /// Container ports from the pod spec.
    pub ports: Vec<ContainerPortInfo>,
    /// Volume mounts for this container.
    pub volume_mounts: Vec<VolumeMount>,
    /// True if this is an init container.
    pub is_init: bool,
}

// PodInfo

#[derive(Debug, Clone)]
pub struct PodInfo {
    pub namespace: String,
    pub name: String,
    pub ready: String,
    pub status: String,
    pub restarts: u32,
    pub age: String,
    pub pod_ip: String,
    pub node_name: String,
    pub cpu: String,
    pub memory: String,
    /// Container names from the pod spec (for exec/shell into specific containers).
    pub containers: Vec<String>,
    /// Per-container status details (for expanded container view).
    pub container_details: Vec<ContainerDetail>,
}

fn container_detail_from_status(
    cs: &k8s_openapi::api::core::v1::ContainerStatus,
    spec_containers: &[&k8s_openapi::api::core::v1::Container],
    is_init: bool,
) -> ContainerDetail {
    let (state_str, reason_str) = cs
        .state
        .as_ref()
        .map(|s| {
            if let Some(_running) = &s.running {
                ("Running".to_string(), String::new())
            } else if let Some(waiting) = &s.waiting {
                let reason = waiting.reason.as_deref().unwrap_or("Waiting").to_string();
                (reason.clone(), reason)
            } else if let Some(terminated) = &s.terminated {
                let reason = terminated
                    .reason
                    .as_deref()
                    .unwrap_or("Terminated")
                    .to_string();
                (reason.clone(), reason)
            } else {
                ("Unknown".to_string(), String::new())
            }
        })
        .unwrap_or(("Unknown".to_string(), String::new()));

    let ports: Vec<ContainerPortInfo> = spec_containers
        .iter()
        .find(|sc| sc.name == cs.name)
        .map(|sc| {
            sc.ports
                .as_ref()
                .map(|ps| {
                    ps.iter()
                        .map(|p| ContainerPortInfo {
                            port: p.container_port as u16,
                            name: p.name.clone(),
                            protocol: p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let volume_mounts: Vec<VolumeMount> = spec_containers
        .iter()
        .find(|sc| sc.name == cs.name)
        .map(|sc| {
            sc.volume_mounts
                .as_ref()
                .map(|vms| {
                    vms.iter()
                        .map(|vm| VolumeMount {
                            name: vm.name.clone(),
                            mount_path: vm.mount_path.clone(),
                            read_only: vm.read_only.unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    ContainerDetail {
        name: cs.name.clone(),
        ready: cs.ready,
        restart_count: cs.restart_count as u32,
        status: state_str,
        reason: reason_str,
        image: cs.image.clone(),
        ports,
        volume_mounts,
        is_init,
    }
}

impl From<&Pod> for PodInfo {
    fn from(pod: &Pod) -> Self {
        let namespace = pod.namespace().unwrap_or_default();
        let name = pod.name_any();

        let spec_containers = pod.spec.as_ref().map(|s| s.containers.len()).unwrap_or(0);

        let containers: Vec<String> = pod
            .spec
            .as_ref()
            .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();

        let (ready_count, restarts) = pod
            .status
            .as_ref()
            .and_then(|s| s.container_statuses.as_ref())
            .map(|statuses| {
                let ready = statuses.iter().filter(|cs| cs.ready).count();
                let restarts: u32 = statuses.iter().map(|cs| cs.restart_count as u32).sum();
                (ready, restarts)
            })
            .unwrap_or((0, 0));

        let spec_containers_list: Vec<&k8s_openapi::api::core::v1::Container> = pod
            .spec
            .as_ref()
            .map(|s| s.containers.iter().collect())
            .unwrap_or_default();

        let spec_init_containers_list: Vec<&k8s_openapi::api::core::v1::Container> = pod
            .spec
            .as_ref()
            .and_then(|s| s.init_containers.as_ref())
            .map(|ics| ics.iter().collect())
            .unwrap_or_default();

        let main_details: Vec<ContainerDetail> = pod
            .status
            .as_ref()
            .and_then(|s| s.container_statuses.as_ref())
            .map(|statuses| {
                statuses
                    .iter()
                    .map(|cs| container_detail_from_status(cs, &spec_containers_list, false))
                    .collect()
            })
            .unwrap_or_default();

        let init_details: Vec<ContainerDetail> = pod
            .status
            .as_ref()
            .and_then(|s| s.init_container_statuses.as_ref())
            .map(|statuses| {
                statuses
                    .iter()
                    .map(|cs| container_detail_from_status(cs, &spec_init_containers_list, true))
                    .collect()
            })
            .unwrap_or_default();

        let container_details: Vec<ContainerDetail> =
            init_details.into_iter().chain(main_details).collect();

        let ready = if spec_containers == 0 {
            "0/0".to_string()
        } else {
            format!("{ready_count}/{spec_containers}")
        };

        let status = derive_pod_status(pod);

        let age = pod
            .metadata
            .creation_timestamp
            .as_ref()
            .map(format_age)
            .unwrap_or_else(|| "Unknown".to_string());

        let pod_ip = pod
            .status
            .as_ref()
            .and_then(|status| status.pod_ip.clone())
            .unwrap_or_else(|| "-".to_string());

        let node_name = pod
            .spec
            .as_ref()
            .and_then(|spec| spec.node_name.clone())
            .unwrap_or_else(|| "-".to_string());

        let resources = extract_pod_resources(pod);

        Self {
            namespace,
            name,
            ready,
            status,
            restarts,
            age,
            pod_ip,
            node_name,
            cpu: resources.cpu,
            memory: resources.memory,
            containers,
            container_details,
        }
    }
}

fn extract_pod_resources(pod: &Pod) -> PodResources {
    let mut cpu_requests: Vec<String> = Vec::new();
    let mut memory_requests: Vec<String> = Vec::new();
    let mut cpu_limits: Vec<String> = Vec::new();
    let mut memory_limits: Vec<String> = Vec::new();

    if let Some(spec) = pod.spec.as_ref() {
        for container in &spec.containers {
            if let Some(resources) = container.resources.as_ref() {
                if let Some(cpu) = resources.requests.as_ref().and_then(|r| r.get("cpu")) {
                    cpu_requests.push(cpu.0.clone());
                }
                if let Some(mem) = resources.requests.as_ref().and_then(|r| r.get("memory")) {
                    memory_requests.push(mem.0.clone());
                }
                if let Some(cpu) = resources.limits.as_ref().and_then(|l| l.get("cpu")) {
                    cpu_limits.push(cpu.0.clone());
                }
                if let Some(mem) = resources.limits.as_ref().and_then(|l| l.get("memory")) {
                    memory_limits.push(mem.0.clone());
                }
            }
        }
    }

    let format_cpu = |req: Option<String>, limit: Option<String>| -> String {
        let req_str = req.unwrap_or_else(|| "∅".to_string());
        let limit_str = limit.unwrap_or_else(|| "∞".to_string());
        format!("{} - {}", req_str, limit_str)
    };

    let format_memory = |req: Option<String>, limit: Option<String>| -> String {
        let req_str = req.unwrap_or_else(|| "∅".to_string());
        let limit_str = limit.unwrap_or_else(|| "∞".to_string());
        format!("{} - {}", req_str, limit_str)
    };

    let combine =
        |requests: Vec<String>, limits: Vec<String>| -> (Option<String>, Option<String>) {
            if requests.is_empty() && limits.is_empty() {
                return (None, None);
            }
            let same_request = if requests.is_empty() {
                None
            } else if requests.iter().all(|v| v == &requests[0]) {
                Some(requests[0].clone())
            } else {
                Some("mixed".to_string())
            };
            let same_limit = if limits.is_empty() {
                None
            } else if limits.iter().all(|v| v == &limits[0]) {
                Some(limits[0].clone())
            } else {
                Some("mixed".to_string())
            };
            (same_request, same_limit)
        };

    let (cpu_req, cpu_lim) = combine(cpu_requests.clone(), cpu_limits.clone());
    let (mem_req, mem_lim) = combine(memory_requests.clone(), memory_limits.clone());

    PodResources {
        cpu: format_cpu(cpu_req, cpu_lim),
        memory: format_memory(mem_req, mem_lim),
    }
}

struct PodResources {
    cpu: String,
    memory: String,
}

fn derive_pod_status(pod: &Pod) -> String {
    let phase = pod
        .status
        .as_ref()
        .and_then(|s| s.phase.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    if phase != "Running" {
        return phase;
    }

    let all_statuses: Vec<&k8s_openapi::api::core::v1::ContainerStatus> = pod
        .status
        .as_ref()
        .map(|s| {
            let mut v: Vec<&k8s_openapi::api::core::v1::ContainerStatus> = Vec::new();
            if let Some(ref cs) = s.container_statuses {
                v.extend(cs.iter());
            }
            if let Some(ref ics) = s.init_container_statuses {
                v.extend(ics.iter());
            }
            v
        })
        .unwrap_or_default();

    let has_crash_loop = all_statuses.iter().any(|cs| {
        cs.state
            .as_ref()
            .and_then(|s| s.waiting.as_ref())
            .is_some_and(|w| w.reason.as_ref().is_some_and(|r| r == "CrashLoopBackOff"))
    });

    if has_crash_loop {
        return "CrashLoopBackOff".to_string();
    }

    phase
}

impl From<&Arc<Pod>> for PodInfo {
    fn from(pod: &Arc<Pod>) -> Self {
        PodInfo::from(pod.as_ref())
    }
}

// NodeInfo

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub name: String,
    pub status: String,
    pub capacity_cpu: String,
    pub capacity_mem: String,
    pub age: String,
}

impl From<&Node> for NodeInfo {
    fn from(node: &Node) -> Self {
        let name = node.name_any();

        let status = node
            .status
            .as_ref()
            .and_then(|s| s.conditions.as_ref())
            .and_then(|conditions| {
                conditions.iter().find(|c| c.type_ == "Ready").map(|c| {
                    if c.status == "True" {
                        "Ready"
                    } else {
                        "NotReady"
                    }
                })
            })
            .unwrap_or("Unknown")
            .to_string();

        let (capacity_cpu, capacity_mem) = node
            .status
            .as_ref()
            .and_then(|s| s.capacity.as_ref())
            .map(|cap| {
                let cpu = cap.get("cpu").map(|q| q.0.clone()).unwrap_or_default();
                let mem = cap.get("memory").map(|q| q.0.clone()).unwrap_or_default();
                (cpu, mem)
            })
            .unwrap_or_default();

        let age = node
            .metadata
            .creation_timestamp
            .as_ref()
            .map(format_age)
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            name,
            status,
            capacity_cpu,
            capacity_mem,
            age,
        }
    }
}

impl From<&Arc<Node>> for NodeInfo {
    fn from(node: &Arc<Node>) -> Self {
        NodeInfo::from(node.as_ref())
    }
}

// DeploymentInfo

#[derive(Debug, Clone)]
pub struct DeploymentInfo {
    pub namespace: String,
    pub name: String,
    pub ready: String,
    pub up_to_date: i32,
    pub available: i32,
    pub age: String,
}

impl From<&Deployment> for DeploymentInfo {
    fn from(deploy: &Deployment) -> Self {
        let namespace = deploy.namespace().unwrap_or_default();
        let name = deploy.name_any();

        let replicas = deploy.status.as_ref().and_then(|s| s.replicas).unwrap_or(0);
        let ready_replicas = deploy
            .status
            .as_ref()
            .and_then(|s| s.ready_replicas)
            .unwrap_or(0);
        let up_to_date = deploy
            .status
            .as_ref()
            .and_then(|s| s.updated_replicas)
            .unwrap_or(0);
        let available = deploy
            .status
            .as_ref()
            .and_then(|s| s.available_replicas)
            .unwrap_or(0);

        let ready = format!("{ready_replicas}/{replicas}");

        let age = deploy
            .metadata
            .creation_timestamp
            .as_ref()
            .map(format_age)
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            namespace,
            name,
            ready,
            up_to_date,
            available,
            age,
        }
    }
}

impl From<&Arc<Deployment>> for DeploymentInfo {
    fn from(deploy: &Arc<Deployment>) -> Self {
        DeploymentInfo::from(deploy.as_ref())
    }
}

// ServiceInfo

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub namespace: String,
    pub name: String,
    pub service_type: String,
    pub cluster_ip: String,
    pub ports: String,
    pub age: String,
}

impl From<&Service> for ServiceInfo {
    fn from(svc: &Service) -> Self {
        let namespace = svc.namespace().unwrap_or_default();
        let name = svc.name_any();

        let service_type = svc
            .spec
            .as_ref()
            .and_then(|s| s.type_.clone())
            .unwrap_or_else(|| "ClusterIP".to_string());

        let cluster_ip = svc
            .spec
            .as_ref()
            .and_then(|s| s.cluster_ip.clone())
            .unwrap_or_default();

        let ports = svc
            .spec
            .as_ref()
            .and_then(|s| s.ports.as_ref())
            .map(|ports| {
                ports
                    .iter()
                    .map(|p| {
                        let port = p.port;
                        let proto = p.protocol.as_deref().unwrap_or("TCP");
                        format!("{port}/{proto}")
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        let age = svc
            .metadata
            .creation_timestamp
            .as_ref()
            .map(format_age)
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            namespace,
            name,
            service_type,
            cluster_ip,
            ports,
            age,
        }
    }
}

impl From<&Arc<Service>> for ServiceInfo {
    fn from(svc: &Arc<Service>) -> Self {
        ServiceInfo::from(svc.as_ref())
    }
}

// EventInfo

#[derive(Debug, Clone)]
pub struct EventInfo {
    pub namespace: String,
    pub event_type: String,
    pub reason: String,
    pub message: String,
    pub involved_object: String,
    pub time: String,
}

impl From<&Event> for EventInfo {
    fn from(ev: &Event) -> Self {
        let namespace = ev.namespace().unwrap_or_default();

        let event_type = ev.type_.clone().unwrap_or_else(|| "Normal".to_string());

        let reason = ev.reason.clone().unwrap_or_default();

        let message = ev.message.clone().unwrap_or_default();

        let obj = &ev.involved_object;
        let kind = obj.kind.as_deref().unwrap_or("Unknown").to_lowercase();
        let name = obj.name.as_deref().unwrap_or("unknown");
        let involved_object = format!("{kind}/{name}");

        let time = ev
            .last_timestamp
            .as_ref()
            .map(format_age)
            .or_else(|| ev.event_time.as_ref().map(format_micro_time_age))
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            namespace,
            event_type,
            reason,
            message,
            involved_object,
            time,
        }
    }
}

impl From<&Arc<Event>> for EventInfo {
    fn from(ev: &Arc<Event>) -> Self {
        EventInfo::from(ev.as_ref())
    }
}

// Helpers

fn format_timestamp_age(ts: jiff::Timestamp) -> String {
    let now = jiff::Timestamp::now();
    let span = match now.since(ts) {
        Ok(s) => s,
        Err(_) => return "Unknown".to_string(),
    };

    let total_hours = span.total(jiff::Unit::Hour).unwrap_or(0.0) as u64;
    let total_days = total_hours / 24;
    let total_minutes = span.total(jiff::Unit::Minute).unwrap_or(0.0) as u64;

    if total_days >= 1 {
        format!("{}d{}h", total_days, total_hours % 24)
    } else if total_hours >= 1 {
        format!("{}h{}m", total_hours, total_minutes % 60)
    } else {
        format!("{}m", total_minutes)
    }
}

fn format_age(time: &k8s_openapi::apimachinery::pkg::apis::meta::v1::Time) -> String {
    format_timestamp_age(time.0)
}

fn format_micro_time_age(
    time: &k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime,
) -> String {
    format_timestamp_age(time.0)
}
