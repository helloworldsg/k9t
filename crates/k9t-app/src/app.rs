use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

use k9t_core::{ContainerDetail, PodInfo};

use crate::command::{Command, CommandItem};
use crate::config::{CommandTemplate, Commands, CustomCommand};
use crate::event::AppEvent;
use crate::mode::{ConfirmContext, ContainerAction, ContainerPickerIntent, Mode};

/// Sort configuration for the pod list.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SortConfig {
    #[default]
    NamespaceAsc,
    NamespaceDesc,
    NameAsc,
    NameDesc,
    AgeAsc,
    AgeDesc,
    StatusAsc,
    StatusDesc,
}

/// A flattened table row for the pod list view.
/// Pods appear as top-level rows; containers appear as indented sub-rows
/// when their parent pod is expanded.
#[derive(Debug, Clone)]
pub enum TableRow {
    /// A pod row — may be collapsed or expanded
    Pod { pod: PodInfo, expanded: bool },
    /// A container sub-row, indented under its parent pod
    Container {
        pod_index: usize, // Index into App::pods for the parent pod
        container: ContainerDetail,
        container_index: usize, // Index within the pod's container_details
        is_last: bool,          // Whether this is the last container in the pod
    },
}

impl TableRow {
    /// Returns the parent PodInfo for this row, if any.
    /// For Pod rows, returns the pod itself. For Container rows, returns None
    /// (caller must look up `pods[pod_index]`).
    pub fn pod(&self) -> Option<&PodInfo> {
        match self {
            TableRow::Pod { pod, .. } => Some(pod),
            TableRow::Container { .. } => None,
        }
    }

    /// Returns true if this row is a container sub-row.
    pub fn is_container(&self) -> bool {
        matches!(self, TableRow::Container { .. })
    }
}

/// An async Kubernetes action to execute after user confirmation.
#[derive(Debug, Clone, PartialEq)]
pub enum AsyncAction {
    KillPod { namespace: String, name: String },
    RestartDeployment { namespace: String, name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

pub struct NamespacePodFilter {
    namespace_regex: regex::Regex,
    pod_regex: regex::Regex,
}

impl NamespacePodFilter {
    pub fn parse(pattern: &str) -> anyhow::Result<Self> {
        let (ns_part, pod_part) = pattern.split_once('/').ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid filter '{pattern}': expected format 'namespace/pod_pattern' (e.g., 'plt/kong.*')"
            )
        })?;
        Ok(Self {
            namespace_regex: regex::Regex::new(ns_part)?,
            pod_regex: regex::Regex::new(pod_part)?,
        })
    }

    pub fn matches(&self, pod: &PodInfo) -> bool {
        self.namespace_regex.is_match(&pod.namespace) && self.pod_regex.is_match(&pod.name)
    }
}

pub struct App {
    pub mode: Mode,
    pub pods: Vec<PodInfo>,
    pub selected_index: usize,
    pub scroll_offset: u32,
    pub search_query: Option<String>,
    pub should_quit: bool,
    pub context_name: Option<String>,
    pub namespace_filter: String,
    pub tick_count: u64,
    pub namespace_pod_filters: Vec<NamespacePodFilter>,
    pub selected_namespaces: HashSet<String>,
    pub available_namespaces: Vec<String>,
    pub namespace_picker_index: usize,
    pub namespace_picker_search: String,
    pub search_active: bool,
    pub search_buffer: String,
    pub search_match_indices: Vec<usize>,
    pub search_match_cursor: usize,
    pub toast_message: Option<String>,
    pub toast_type: ToastType,
    /// Countdown (in ticks) until toast auto-dismisses. Set when toast is created.
    pub toast_ttl: u8,
    pub theme_index: usize,
    pub theme_count: usize,
    /// When set, the main loop should suspend the TUI and run a kubectl subcommand.
    pub pending_shell: Option<ShellCommand>,
    /// When set, the main loop should execute an async Kubernetes action (delete, restart).
    pub pending_async_action: Option<AsyncAction>,
    /// Container names for the currently selected pod (used by ContainerPicker).
    pub container_choices: Vec<String>,
    /// Index of the highlighted item in the container picker.
    pub container_picker_index: usize,
    /// User-defined custom commands loaded from config.
    pub custom_commands: Vec<CustomCommand>,
    /// All kubeconfig context names available for switching.
    pub available_contexts: Vec<String>,
    /// Index of the highlighted item in the context picker.
    pub context_picker_index: usize,
    /// Search filter for the context picker.
    pub context_picker_search: String,
    /// When set, the main loop should switch to this kube context.
    /// The main loop will re-create the client, refresh namespaces, and restart the reflector.
    pub pending_context_switch: Option<String>,
    /// Buffer for the set-image input mode.
    pub set_image_buffer: String,
    /// Namespace of the pod/deployment for set-image.
    pub set_image_namespace: String,
    /// Pod name for set-image (used to resolve deployment via ownerReferences).
    pub set_image_pod: String,
    /// Container name selected for set-image.
    pub set_image_container: String,
    /// Set of pod names that are currently expanded (showing containers).
    pub expanded_pods: HashSet<String>,
    /// Staging buffer for namespace picker — changes are applied on Enter, discarded on Esc.
    pub staged_namespaces: Option<HashSet<String>>,
    /// Buffer for port-forward input mode.
    pub port_forward_buffer: String,
    /// Namespace of the pod for port-forward.
    pub port_forward_namespace: String,
    /// Pod name for port-forward.
    pub port_forward_pod: String,
    /// Container name for port-forward (if specific container needed).
    pub port_forward_container: Option<String>,
    /// Current sort configuration for the pod list.
    pub sort_config: SortConfig,
    /// Actions list for the container actions dialog.
    pub container_actions: Vec<ContainerAction>,
    /// Selected index in the container actions dialog.
    pub container_actions_index: usize,
    /// Built-in command templates (logs, shell, describe, etc.) from config.
    pub commands_builtin: Commands,
}

/// A kubectl subcommand to run outside the TUI (suspend/resume pattern).
/// A shell command to run when the TUI is suspended.
/// Paged commands (logs, yaml, describe) include the pager in the command string itself
/// (e.g. `bash -c "kubectl ... | hl"`), so they're visible and overridable in config.
/// Interactive commands (exec, port-forward) are run directly so the terminal works properly.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
    /// For exec commands: fallback shell paths to try if the primary fails with exit code 126.
    pub fallback_commands: Vec<ShellCommand>,
}

impl ShellCommand {
    /// Wrap a rendered command string in `bash -c` so pipes, redirects, and shell features work.
    fn bash(template: &str) -> Self {
        Self {
            program: "bash".to_string(),
            args: vec!["-c".to_string(), template.to_string()],
            fallback_commands: vec![],
        }
    }
}

impl App {
    pub fn new(context_name: Option<String>) -> Self {
        Self::with_commands(context_name, Vec::new(), Commands::default())
    }

    pub fn with_commands(
        context_name: Option<String>,
        custom_commands: Vec<CustomCommand>,
        commands_builtin: Commands,
    ) -> Self {
        Self {
            mode: Mode::Normal,
            pods: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            search_query: None,
            should_quit: false,
            context_name,
            namespace_filter: "*".to_string(),
            tick_count: 0,
            namespace_pod_filters: Vec::new(),
            selected_namespaces: HashSet::new(),
            available_namespaces: Vec::new(),
            namespace_picker_index: 0,
            namespace_picker_search: String::new(),
            search_active: false,
            search_buffer: String::new(),
            search_match_indices: Vec::new(),
            search_match_cursor: 0,
            toast_message: None,
            toast_type: ToastType::Info,
            toast_ttl: 0,
            theme_index: 0,
            theme_count: 6,
            pending_shell: None,
            pending_async_action: None,
            container_choices: Vec::new(),
            container_picker_index: 0,
            custom_commands,
            available_contexts: Vec::new(),
            context_picker_index: 0,
            context_picker_search: String::new(),
            pending_context_switch: None,
            set_image_buffer: String::new(),
            set_image_namespace: String::new(),
            set_image_pod: String::new(),
            set_image_container: String::new(),
            expanded_pods: HashSet::new(),
            staged_namespaces: None,
            port_forward_buffer: String::new(),
            port_forward_namespace: String::new(),
            port_forward_pod: String::new(),
            port_forward_container: None,
            sort_config: SortConfig::default(),
            container_actions: Vec::new(),
            container_actions_index: 0,
            commands_builtin,
        }
    }

    // ── Command template helpers ──
    // Each method renders the built-in template from config.

    /// Build a logs command from the built-in template.
    fn render_builtin(
        &self,
        template: &str,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        extra: &[(&str, &str)],
    ) -> ShellCommand {
        let rendered = CommandTemplate {
            command: template.to_string(),
        }
        .render_with(
            namespace,
            pod_name,
            container,
            self.context_name.as_deref(),
            extra,
        );
        ShellCommand::bash(&rendered)
    }

    pub fn build_logs_cmd(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        previous: bool,
    ) -> ShellCommand {
        let template = if previous {
            &self.commands_builtin.previous_logs
        } else {
            &self.commands_builtin.logs
        };
        self.render_builtin(template, namespace, pod_name, container, &[])
    }

    pub fn build_shell_cmd(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
    ) -> ShellCommand {
        self.render_builtin(
            &self.commands_builtin.shell,
            namespace,
            pod_name,
            container,
            &[],
        )
    }

    pub fn build_describe_cmd(&self, namespace: &str, pod_name: &str) -> ShellCommand {
        self.render_builtin(
            &self.commands_builtin.describe,
            namespace,
            pod_name,
            None,
            &[],
        )
    }

    pub fn build_yaml_cmd(&self, namespace: &str, pod_name: &str) -> ShellCommand {
        self.render_builtin(&self.commands_builtin.yaml, namespace, pod_name, None, &[])
    }

    pub fn build_set_image_cmd(
        &self,
        namespace: &str,
        pod_name: &str,
        container: &str,
        image: &str,
    ) -> ShellCommand {
        self.render_builtin(
            &self.commands_builtin.set_image,
            namespace,
            pod_name,
            Some(container),
            &[("IMAGE", image)],
        )
    }

    pub fn build_port_forward_cmd(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        ports: &str,
    ) -> ShellCommand {
        self.render_builtin(
            &self.commands_builtin.port_forward,
            namespace,
            pod_name,
            container,
            &[("PORTS", ports)],
        )
    }

    pub fn build_debug_cmd(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
    ) -> ShellCommand {
        self.render_builtin(
            &self.commands_builtin.debug,
            namespace,
            pod_name,
            container,
            &[],
        )
    }

    /// Build the flattened table rows (pods + expanded containers) from the current pod list.
    /// This is the view the table widget renders and the selection index navigates.
    /// When a search filter is active, only matching pods (and their containers) are shown.
    /// The filter persists after exiting search mode (Enter commits, Esc clears).
    pub fn table_rows(&self) -> Vec<TableRow> {
        // Use search query from active search mode OR committed filter
        let search_query = match &self.mode {
            Mode::Search(q) if !q.is_empty() => Some(q.to_lowercase()),
            _ => self.search_query.as_ref().map(|q| q.to_lowercase()),
        };

        let mut rows = Vec::new();
        for (pod_index, pod) in self.pods.iter().enumerate() {
            // Filter by search query
            if let Some(ref query) = search_query {
                let matches_pod = pod.name.to_lowercase().contains(query)
                    || pod.namespace.to_lowercase().contains(query);
                let matches_container = pod.container_details.iter().any(|c| {
                    c.name.to_lowercase().contains(query)
                        || c.status.to_lowercase().contains(query)
                        || c.image.to_lowercase().contains(query)
                });

                if !matches_pod && !matches_container {
                    continue; // Skip this pod entirely
                }
            }

            let expanded = self.expanded_pods.contains(&pod.name);
            rows.push(TableRow::Pod {
                pod: pod.clone(),
                expanded,
            });
            if expanded {
                let detail_count = pod.container_details.len();
                for (ci, container) in pod.container_details.iter().enumerate() {
                    // If search is active and only the pod matches, still show all containers
                    // If only a container matches, only show that container's pod with it
                    rows.push(TableRow::Container {
                        pod_index,
                        container: container.clone(),
                        container_index: ci,
                        is_last: ci + 1 == detail_count,
                    });
                }
            }
        }
        rows
    }

    /// Convenience: return the selected TableRow, if any.
    pub fn selected_row(&self) -> Option<TableRow> {
        let rows = self.table_rows();
        rows.into_iter().nth(self.selected_index)
    }

    /// Convenience: return the pod that the current selection refers to.
    /// For Pod rows, returns the pod itself. For Container rows, returns the parent pod.
    /// Returns an owned PodInfo to avoid borrow checker issues.
    pub fn selected_pod_cloned(&self) -> Option<PodInfo> {
        let rows = self.table_rows();
        match rows.get(self.selected_index) {
            Some(TableRow::Pod { pod, .. }) => Some(pod.clone()),
            Some(TableRow::Container { pod_index, .. }) => self.pods.get(*pod_index).cloned(),
            None => None,
        }
    }

    /// Convenience: if a container row is selected, return its name.
    pub fn selected_container_name(&self) -> Option<String> {
        let rows = self.table_rows();
        match rows.get(self.selected_index) {
            Some(TableRow::Container { container, .. }) => Some(container.name.clone()),
            _ => None,
        }
    }

    /// Convenience: return the suggested port-forward string for the current selection.
    /// Takes the first container port and formats it as "local:remote" (using the same port).
    pub fn suggested_port_forward(&self) -> String {
        let rows = self.table_rows();
        match rows.get(self.selected_index) {
            Some(TableRow::Container { container, .. }) => {
                if let Some(port_info) = container.ports.first() {
                    format!("{}:{}", port_info.port, port_info.port)
                } else {
                    String::new()
                }
            }
            Some(TableRow::Pod { pod, .. }) => {
                // For a pod row, look at the first container's first port
                pod.container_details
                    .first()
                    .and_then(|c| c.ports.first())
                    .map(|p| format!("{}:{}", p.port, p.port))
                    .unwrap_or_default()
            }
            None => String::new(),
        }
    }

    pub fn update(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Tick => {
                self.tick_count += 1;
                // Auto-dismiss toast after TTL expires
                if self.toast_ttl > 0 {
                    self.toast_ttl -= 1;
                    if self.toast_ttl == 0 {
                        self.toast_message = None;
                    }
                }
            }
            AppEvent::Resize(_, _) => {}
            AppEvent::PodsUpdated => {}
            AppEvent::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return self.should_quit;
                }
                self.handle_key(key);
            }
            AppEvent::Mouse(mouse) => {
                self.handle_mouse(mouse);
            }
        }
        self.should_quit
    }

    /// Handle mouse events for click-to-select, click-to-activate, and scroll.
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                let total = self.table_rows().len();
                self.selected_index = (self.selected_index + 1).min(total.saturating_sub(1));
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) if mouse.row >= 2 => {
                let clicked_row = (mouse.row - 2) as usize;
                let total_rows = self.table_rows().len();
                if clicked_row < total_rows {
                    if clicked_row == self.selected_index {
                        let rows = self.table_rows();
                        if let Some(row) = rows.get(clicked_row) {
                            match row {
                                TableRow::Pod { .. } => {
                                    self.toggle_expand_selected();
                                }
                                TableRow::Container { .. } => {
                                    self.open_container_actions();
                                }
                            }
                        }
                    } else {
                        self.selected_index = clicked_row;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match &self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::CommandPalette { .. } => self.handle_command_palette_key(key),
            Mode::ConfirmAction(_) => self.handle_confirm_key(key),
            Mode::Search(_) => self.handle_search_key(key),
            Mode::Help => self.handle_help_key(key),
            Mode::NamespacePicker => self.handle_namespace_picker_key(key),
            Mode::ContextPicker => self.handle_context_picker_key(key),
            Mode::ContainerActions { .. } => self.handle_container_actions_key(key),
            Mode::ContainerPicker(_) => self.handle_container_picker_key(key),
            Mode::SetImageInput => self.handle_set_image_key(key),
            Mode::PortForwardInput => self.handle_port_forward_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        let total_rows = self.table_rows().len().max(1);

        match (key.modifiers, key.code) {
            // ── Navigation (k9s: j/k/g/G) ──
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.selected_index = (self.selected_index + 1).min(total_rows.saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::Char('g')) | (KeyModifiers::NONE, KeyCode::Home) => {
                self.selected_index = 0;
            }
            (KeyModifiers::NONE, KeyCode::Char('G')) | (KeyModifiers::NONE, KeyCode::End) => {
                self.selected_index = total_rows.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::PageDown)
            | (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
                let page = 10.min(total_rows);
                self.selected_index =
                    (self.selected_index + page).min(total_rows.saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::PageUp) | (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                let page = 10.min(total_rows);
                self.selected_index = self.selected_index.saturating_sub(page);
            }
            // ── Expand/Collapse (Enter) ──
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // On pod row: expand/collapse. On container row: show action menu.
                let rows = self.table_rows();
                if let Some(row) = rows.get(self.selected_index) {
                    match row {
                        TableRow::Pod { .. } => {
                            self.toggle_expand_selected();
                        }
                        TableRow::Container { .. } => {
                            self.open_container_actions();
                        }
                    }
                }
            }
            // ── Quit ──
            (KeyModifiers::NONE, KeyCode::Esc) if self.search_query.is_some() => {
                self.search_query = None;
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {}
            // ── Help (k9s: ?) ──
            (KeyModifiers::NONE, KeyCode::Char('?')) => {
                self.mode = Mode::Help;
            }
            // ── Search / Filter (k9s: /) ──
            (KeyModifiers::NONE, KeyCode::Char('/')) => {
                // Start search with existing committed query so user can refine
                let initial = self.search_query.clone().unwrap_or_default();
                self.mode = Mode::Search(initial);
                self.search_match_indices.clear();
            }
            // ── Command palette (k9s: :) ──
            (KeyModifiers::NONE, KeyCode::Char(':')) => {
                self.mode = Mode::CommandPalette {
                    query: String::new(),
                    index: 0,
                };
            }
            // ── Actions: Logs (k9s: l) → kubectl logs -f ──
            (KeyModifiers::NONE, KeyCode::Char('l')) => {
                if let Some(row) = self.selected_row() {
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.pending_shell = Some(self.build_logs_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    Some(&container.name),
                                    false,
                                ));
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode =
                                    Mode::ContainerPicker(ContainerPickerIntent::Logs(false));
                            } else {
                                let container = pod.containers.first().cloned();
                                self.pending_shell = Some(self.build_logs_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    container.as_deref(),
                                    false,
                                ));
                            }
                        }
                    }
                }
            }
            // ── Actions: Previous logs (k9s: p) → kubectl logs --previous ──
            (KeyModifiers::NONE, KeyCode::Char('p')) => {
                if let Some(row) = self.selected_row() {
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.pending_shell = Some(self.build_logs_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    Some(&container.name),
                                    true,
                                ));
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode =
                                    Mode::ContainerPicker(ContainerPickerIntent::Logs(true));
                            } else {
                                let container = pod.containers.first().cloned();
                                self.pending_shell = Some(self.build_logs_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    container.as_deref(),
                                    true,
                                ));
                            }
                        }
                    }
                }
            }
            // ── Actions: Describe (k9s: d) → kubectl describe ──
            // Always describes the pod (even if container row selected)
            (KeyModifiers::NONE, KeyCode::Char('d')) => {
                if let Some(pod) = self.selected_pod_cloned() {
                    self.pending_shell = Some(self.build_describe_cmd(&pod.namespace, &pod.name));
                }
            }
            // ── Actions: Debug (D) → kubectl debug ──
            (KeyModifiers::SHIFT, KeyCode::Char('D')) => {
                if let Some(row) = self.selected_row() {
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.pending_shell = Some(self.build_debug_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    Some(&container.name),
                                ));
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode = Mode::ContainerPicker(ContainerPickerIntent::Debug);
                            } else {
                                let container = pod.containers.first().cloned();
                                self.pending_shell = Some(self.build_debug_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    container.as_deref(),
                                ));
                            }
                        }
                    }
                }
            }
            // ── Actions: Shell (k9s: s) → kubectl exec ──
            (KeyModifiers::NONE, KeyCode::Char('s')) => {
                if let Some(row) = self.selected_row() {
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.pending_shell = Some(self.build_shell_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    Some(&container.name),
                                ));
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode = Mode::ContainerPicker(ContainerPickerIntent::Shell);
                            } else {
                                let container = pod.containers.first().cloned();
                                self.pending_shell = Some(self.build_shell_cmd(
                                    &pod.namespace,
                                    &pod.name,
                                    container.as_deref(),
                                ));
                            }
                        }
                    }
                }
            }
            // ── Actions: YAML (k9s: y) → kubectl get -o yaml ──
            // Always shows YAML for the pod
            (KeyModifiers::NONE, KeyCode::Char('y')) => {
                if let Some(pod) = self.selected_pod_cloned() {
                    self.pending_shell = Some(self.build_yaml_cmd(&pod.namespace, &pod.name));
                }
            }
            // ── Actions: Set image (i) → select container, then type new image ──
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                if let Some(row) = self.selected_row() {
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.set_image_namespace = pod.namespace.clone();
                                self.set_image_pod = pod.name.clone();
                                self.set_image_container = container.name.clone();
                                self.set_image_buffer = container.image.clone();
                                self.mode = Mode::SetImageInput;
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode = Mode::ContainerPicker(ContainerPickerIntent::SetImage);
                            } else {
                                self.set_image_namespace = pod.namespace.clone();
                                self.set_image_pod = pod.name.clone();
                                self.set_image_container =
                                    pod.containers.first().cloned().unwrap_or_default();
                                self.set_image_buffer = pod
                                    .container_details
                                    .iter()
                                    .find(|c| c.name == self.set_image_container)
                                    .map(|c| c.image.clone())
                                    .unwrap_or_default();
                                self.mode = Mode::SetImageInput;
                            }
                        }
                    }
                }
            }
            // ── Actions: Port forward (f) → kubectl port-forward ──
            (KeyModifiers::NONE, KeyCode::Char('f')) => {
                if let Some(row) = self.selected_row() {
                    let suggested = self.suggested_port_forward();
                    match &row {
                        TableRow::Container {
                            pod_index,
                            container,
                            ..
                        } => {
                            if let Some(pod) = self.pods.get(*pod_index) {
                                self.port_forward_namespace = pod.namespace.clone();
                                self.port_forward_pod = pod.name.clone();
                                self.port_forward_container = Some(container.name.clone());
                                self.port_forward_buffer = suggested;
                                self.mode = Mode::PortForwardInput;
                            }
                        }
                        TableRow::Pod { pod, .. } => {
                            if pod.containers.len() > 1 {
                                self.container_choices = pod.containers.clone();
                                self.container_picker_index = 0;
                                self.mode =
                                    Mode::ContainerPicker(ContainerPickerIntent::PortForward);
                            } else {
                                self.port_forward_namespace = pod.namespace.clone();
                                self.port_forward_pod = pod.name.clone();
                                self.port_forward_container = pod.containers.first().cloned();
                                self.port_forward_buffer = suggested;
                                self.mode = Mode::PortForwardInput;
                            }
                        }
                    }
                }
            }
            // ── Actions: Kill pod (Shift+K) → confirm then delete ──
            // Always kills the pod (even if container row selected)
            (KeyModifiers::SHIFT, KeyCode::Char('K')) => {
                if let Some(pod) = self.selected_pod_cloned() {
                    self.mode = Mode::ConfirmAction(ConfirmContext::KillPod {
                        namespace: pod.namespace.clone(),
                        name: pod.name.clone(),
                    });
                }
            }
            // ── Actions: Restart deployment (Shift+R) → confirm then rollout restart ──
            // Always restarts the pod's deployment (even if container row selected)
            (KeyModifiers::SHIFT, KeyCode::Char('R')) => {
                if let Some(pod) = self.selected_pod_cloned() {
                    self.mode = Mode::ConfirmAction(ConfirmContext::RestartDeployment {
                        namespace: pod.namespace.clone(),
                        name: pod.name.clone(),
                    });
                }
            }
            // ── Namespace picker ──
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                // Stage current namespaces so changes can be applied on Enter or discarded on Esc
                self.staged_namespaces = Some(self.selected_namespaces.clone());
                self.mode = Mode::NamespacePicker;
                self.namespace_picker_index = 0;
                self.namespace_picker_search = String::new();
            }
            // ── Context picker ──
            (KeyModifiers::NONE, KeyCode::Char('x')) => {
                self.mode = Mode::ContextPicker;
                self.context_picker_index = 0;
                self.context_picker_search = String::new();
            }
            // ── UI toggles ──
            (KeyModifiers::SHIFT, KeyCode::Char('T')) => {
                self.theme_index = (self.theme_index + 1) % self.theme_count;
            }
            // ── Sort cycle ──
            (KeyModifiers::NONE, KeyCode::Char(',')) => {
                self.sort_config = match self.sort_config {
                    SortConfig::NamespaceAsc => SortConfig::NamespaceDesc,
                    SortConfig::NamespaceDesc => SortConfig::NameAsc,
                    SortConfig::NameAsc => SortConfig::NameDesc,
                    SortConfig::NameDesc => SortConfig::AgeAsc,
                    SortConfig::AgeAsc => SortConfig::AgeDesc,
                    SortConfig::AgeDesc => SortConfig::StatusAsc,
                    SortConfig::StatusAsc => SortConfig::StatusDesc,
                    SortConfig::StatusDesc => SortConfig::NamespaceAsc,
                };
            }
            (KeyModifiers::NONE, KeyCode::Char('q')) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    /// Toggle expand/collapse for the selected pod row.
    /// If a container row is selected, collapse its parent pod.
    /// Open the container actions dialog for the currently selected container.
    fn open_container_actions(&mut self) {
        let rows = self.table_rows();
        if let Some(TableRow::Container {
            pod_index,
            container: _,
            ..
        }) = rows.get(self.selected_index).cloned()
        {
            if let Some(pod) = self.pods.get(pod_index) {
                let mut actions = vec![
                    ContainerAction::Logs,
                    ContainerAction::PreviousLogs,
                    ContainerAction::Shell,
                    ContainerAction::Describe,
                    ContainerAction::Yaml,
                    ContainerAction::SetImage,
                    ContainerAction::PortForward,
                    ContainerAction::Debug,
                ];
                for cmd in &self.custom_commands {
                    if cmd.matches(&pod.namespace, &pod.name, None) {
                        actions.push(ContainerAction::Custom(cmd.clone()));
                    }
                }
                self.container_actions = actions;
                self.container_actions_index = 0;
                self.mode = Mode::ContainerActions {
                    query: String::new(),
                    index: 0,
                };
            }
        }
    }

    fn handle_container_actions_key(&mut self, key: KeyEvent) {
        // Get the current query from the mode
        let (query, index) = match &self.mode {
            Mode::ContainerActions { query, index } => (query.clone(), *index),
            _ => unreachable!(),
        };

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Select from the filtered list
                let filtered = self.filtered_container_actions(&query);
                if let Some(action) = filtered.get(index).cloned() {
                    // Map filtered index back to the full actions list
                    let full_index = self
                        .container_actions
                        .iter()
                        .position(|a| a == &action)
                        .unwrap_or(index);
                    self.container_actions_index = full_index;
                    self.execute_selected_container_action();
                } else {
                    self.mode = Mode::Normal;
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                let filtered = self.filtered_container_actions(&query);
                let new_index = (index + 1).min(filtered.len().saturating_sub(1));
                self.mode = Mode::ContainerActions {
                    query,
                    index: new_index,
                };
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                let new_index = index.saturating_sub(1);
                self.mode = Mode::ContainerActions {
                    query,
                    index: new_index,
                };
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let mut new_query = query;
                new_query.pop();
                self.mode = Mode::ContainerActions {
                    query: new_query,
                    index: 0,
                };
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                // Clear query
                self.mode = Mode::ContainerActions {
                    query: String::new(),
                    index: 0,
                };
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                let mut new_query = query;
                new_query.push(c);
                self.mode = Mode::ContainerActions {
                    query: new_query,
                    index: 0,
                };
            }
            _ => {}
        }
    }

    /// Filter container actions by query string.
    pub fn filtered_container_actions(&self, query: &str) -> Vec<ContainerAction> {
        if query.is_empty() {
            return self.container_actions.clone();
        }
        let q = query.to_lowercase();
        self.container_actions
            .iter()
            .filter(|a| a.label().to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// Execute the container action at the current container_actions_index.
    fn execute_selected_container_action(&mut self) {
        if self.container_actions.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        let action = self
            .container_actions
            .get(self.container_actions_index)
            .cloned();
        if action.is_none() {
            self.mode = Mode::Normal;
            return;
        }
        let action = action.unwrap();
        // We need the selected row info before switching mode
        let rows = self.table_rows();
        let selected = rows.get(self.selected_index).cloned();
        self.mode = Mode::Normal;

        if let Some(TableRow::Container {
            pod_index,
            container,
            ..
        }) = selected
        {
            if let Some(pod) = self.pods.get(pod_index).cloned() {
                match action {
                    ContainerAction::Logs => {
                        self.pending_shell = Some(self.build_logs_cmd(
                            &pod.namespace,
                            &pod.name,
                            Some(&container.name),
                            false,
                        ));
                    }
                    ContainerAction::PreviousLogs => {
                        self.pending_shell = Some(self.build_logs_cmd(
                            &pod.namespace,
                            &pod.name,
                            Some(&container.name),
                            true,
                        ));
                    }
                    ContainerAction::Shell => {
                        self.pending_shell = Some(self.build_shell_cmd(
                            &pod.namespace,
                            &pod.name,
                            Some(&container.name),
                        ));
                    }
                    ContainerAction::Describe => {
                        self.pending_shell =
                            Some(self.build_describe_cmd(&pod.namespace, &pod.name));
                    }
                    ContainerAction::Yaml => {
                        self.pending_shell = Some(self.build_yaml_cmd(&pod.namespace, &pod.name));
                    }
                    ContainerAction::SetImage => {
                        self.set_image_namespace = pod.namespace.clone();
                        self.set_image_pod = pod.name.clone();
                        self.set_image_container = container.name.clone();
                        self.set_image_buffer = container.image.clone();
                        self.mode = Mode::SetImageInput;
                    }
                    ContainerAction::PortForward => {
                        self.port_forward_namespace = pod.namespace.clone();
                        self.port_forward_pod = pod.name.clone();
                        self.port_forward_container = Some(container.name.clone());
                        self.port_forward_buffer = pod
                            .container_details
                            .iter()
                            .find(|c| c.name == container.name)
                            .and_then(|c| c.ports.first())
                            .map(|p| format!("{}:{}", p.port, p.port))
                            .unwrap_or_default();
                        self.mode = Mode::PortForwardInput;
                    }
                    ContainerAction::Debug => {
                        self.pending_shell = Some(self.build_debug_cmd(
                            &pod.namespace,
                            &pod.name,
                            Some(&container.name),
                        ));
                    }
                    ContainerAction::Custom(cmd) => {
                        let rendered = cmd.render(
                            &pod.namespace,
                            &pod.name,
                            Some(&container.name),
                            self.context_name.as_deref(),
                        );
                        self.pending_shell = Some(ShellCommand::bash(&rendered));
                    }
                }
            }
        }
    }

    fn toggle_expand_selected(&mut self) {
        let rows = self.table_rows();
        if let Some(row) = rows.get(self.selected_index) {
            let pod_name = match row {
                TableRow::Pod { pod, .. } => pod.name.clone(),
                TableRow::Container { pod_index, .. } => {
                    // Container selected → collapse the parent pod
                    self.pods
                        .get(*pod_index)
                        .map(|p| p.name.clone())
                        .unwrap_or_default()
                }
            };

            if self.expanded_pods.contains(&pod_name) {
                self.expanded_pods.remove(&pod_name);
                // Adjust selection: if we collapsed, the selected index may now point
                // to a different pod. Clamp it.
                let new_total = self.table_rows().len();
                self.selected_index = self.selected_index.min(new_total.saturating_sub(1));
            } else {
                self.expanded_pods.insert(pod_name);
            }
        }
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if let Mode::CommandPalette { index, .. } = &mut self.mode {
                    *index = index.saturating_sub(1);
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                let filtered = self.filtered_command_items();
                if let Mode::CommandPalette { index, .. } = &mut self.mode {
                    *index = (*index + 1).min(filtered.len().saturating_sub(1));
                }
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let filtered = self.filtered_command_items();
                if let Mode::CommandPalette { index, .. } = &mut self.mode {
                    let page = 6.min(filtered.len());
                    *index = (*index + page).min(filtered.len().saturating_sub(1));
                }
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                if let Mode::CommandPalette { index, .. } = &mut self.mode {
                    let page = 6;
                    *index = index.saturating_sub(page);
                }
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let cmd = if let Mode::CommandPalette { index, .. } = &self.mode {
                    let filtered = self.filtered_command_items();
                    filtered.get(*index).map(|item| item.command.clone())
                } else {
                    None
                };
                if let Some(cmd) = cmd {
                    self.execute_command(cmd);
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Mode::CommandPalette { query, index, .. } = &mut self.mode {
                    query.pop();
                    *index = 0;
                }
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                if let Mode::CommandPalette { query, index, .. } = &mut self.mode {
                    query.push(c);
                    *index = 0;
                }
            }
            _ => {}
        }
    }

    /// Return the command palette items filtered by the current query.
    pub fn filtered_command_items(&self) -> Vec<CommandItem> {
        let query = match &self.mode {
            Mode::CommandPalette { query, .. } => query.as_str(),
            _ => "",
        };
        CommandItem::build_list(&self.custom_commands)
            .into_iter()
            .filter(|item| item.fuzzy_matches(query))
            .collect()
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Char('y')) | (KeyModifiers::NONE, KeyCode::Char('Y')) => {
                if let Mode::ConfirmAction(ref ctx) = self.mode {
                    match ctx {
                        ConfirmContext::KillPod { namespace, name } => {
                            self.pending_async_action = Some(AsyncAction::KillPod {
                                namespace: namespace.clone(),
                                name: name.clone(),
                            });
                        }
                        ConfirmContext::RestartDeployment { namespace, name } => {
                            self.pending_async_action = Some(AsyncAction::RestartDeployment {
                                namespace: namespace.clone(),
                                name: name.clone(),
                            });
                        }
                    }
                }
                self.mode = Mode::Normal;
            }
            // Any other key cancels
            _ => {
                self.mode = Mode::Normal;
            }
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    fn handle_namespace_picker_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Discard staged changes
                self.staged_namespaces = None;
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Apply staged changes
                if let Some(staged) = self.staged_namespaces.take() {
                    self.selected_namespaces = staged;
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Char('a')) => {
                // Select all = clear the staging set (empty = all)
                if let Some(ref mut staged) = self.staged_namespaces {
                    staged.clear();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.namespace_picker_index = (self.namespace_picker_index + 1).min(
                    self.filtered_namespaces_for_picker()
                        .len()
                        .saturating_sub(1),
                );
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.namespace_picker_index = self.namespace_picker_index.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let page = 10.min(self.filtered_namespaces_for_picker().len());
                self.namespace_picker_index = (self.namespace_picker_index + page).min(
                    self.filtered_namespaces_for_picker()
                        .len()
                        .saturating_sub(1),
                );
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                let page = 10.min(self.filtered_namespaces_for_picker().len());
                self.namespace_picker_index = self.namespace_picker_index.saturating_sub(page);
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.namespace_picker_index = 0;
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.namespace_picker_index = self
                    .filtered_namespaces_for_picker()
                    .len()
                    .saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                if let Some(ns) = self
                    .filtered_namespaces_for_picker()
                    .get(self.namespace_picker_index)
                    .cloned()
                {
                    if let Some(ref mut staged) = self.staged_namespaces {
                        if staged.contains(&ns) {
                            staged.remove(&ns);
                        } else {
                            staged.insert(ns);
                        }
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.namespace_picker_search.pop();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.namespace_picker_search.push(c);
            }
            _ => {}
        }
    }

    fn handle_context_picker_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(ctx) = self
                    .filtered_contexts()
                    .get(self.context_picker_index)
                    .cloned()
                {
                    // Only switch if the context is different from the current one
                    if Some(&ctx) != self.context_name.as_ref() {
                        self.pending_context_switch = Some(ctx);
                    }
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.context_picker_index = (self.context_picker_index + 1)
                    .min(self.filtered_contexts().len().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.context_picker_index = self.context_picker_index.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let page = 10.min(self.filtered_contexts().len());
                self.context_picker_index = (self.context_picker_index + page)
                    .min(self.filtered_contexts().len().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                let page = 10.min(self.filtered_contexts().len());
                self.context_picker_index = self.context_picker_index.saturating_sub(page);
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.context_picker_index = 0;
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.context_picker_index = self.filtered_contexts().len().saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.context_picker_search.pop();
                self.context_picker_index = 0;
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.context_picker_search.push(c);
                self.context_picker_index = 0;
            }
            _ => {}
        }
    }

    fn handle_container_picker_key(&mut self, key: KeyEvent) {
        // Capture the intent before matching (borrow checker workaround)
        let intent = match &self.mode {
            Mode::ContainerPicker(intent) => intent.clone(),
            _ => unreachable!(),
        };

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(container) = self
                    .container_choices
                    .get(self.container_picker_index)
                    .cloned()
                    && let Some(pod) = self.selected_pod_cloned()
                {
                    match intent {
                        ContainerPickerIntent::Shell => {
                            self.pending_shell = Some(self.build_shell_cmd(
                                &pod.namespace,
                                &pod.name,
                                Some(&container),
                            ));
                        }
                        ContainerPickerIntent::Logs(previous) => {
                            self.pending_shell = Some(self.build_logs_cmd(
                                &pod.namespace,
                                &pod.name,
                                Some(&container),
                                previous,
                            ));
                        }
                        ContainerPickerIntent::SetImage => {
                            self.set_image_namespace = pod.namespace.clone();
                            self.set_image_pod = pod.name.clone();
                            self.set_image_container = container.clone();
                            self.set_image_buffer = pod
                                .container_details
                                .iter()
                                .find(|c| c.name == container)
                                .map(|c| c.image.clone())
                                .unwrap_or_default();
                            self.mode = Mode::SetImageInput;
                            return;
                        }
                        ContainerPickerIntent::PortForward => {
                            self.port_forward_namespace = pod.namespace.clone();
                            self.port_forward_pod = pod.name.clone();
                            self.port_forward_container = Some(container.clone());
                            // Auto-fill with the selected container's first port
                            self.port_forward_buffer = pod
                                .container_details
                                .iter()
                                .find(|c| c.name == container)
                                .and_then(|c| c.ports.first())
                                .map(|p| format!("{}:{}", p.port, p.port))
                                .unwrap_or_default();
                            self.mode = Mode::PortForwardInput;
                            return;
                        }
                        ContainerPickerIntent::Debug => {
                            self.pending_shell = Some(self.build_debug_cmd(
                                &pod.namespace,
                                &pod.name,
                                Some(&container),
                            ));
                        }
                    }
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.container_picker_index = (self.container_picker_index + 1)
                    .min(self.container_choices.len().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                self.container_picker_index = self.container_picker_index.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let page = 10.min(self.container_choices.len());
                self.container_picker_index = (self.container_picker_index + page)
                    .min(self.container_choices.len().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                let page = 10.min(self.container_choices.len());
                self.container_picker_index = self.container_picker_index.saturating_sub(page);
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.container_picker_index = 0;
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.container_picker_index = self.container_choices.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    fn handle_port_forward_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let ports = self.port_forward_buffer.trim().to_string();
                if !ports.is_empty() {
                    self.pending_shell = Some(self.build_port_forward_cmd(
                        &self.port_forward_namespace,
                        &self.port_forward_pod,
                        self.port_forward_container.as_deref(),
                        &ports,
                    ));
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.port_forward_buffer.pop();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.port_forward_buffer.clear();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.port_forward_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_set_image_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let image = self.set_image_buffer.trim().to_string();
                if !image.is_empty() {
                    self.pending_shell = Some(self.build_set_image_cmd(
                        &self.set_image_namespace,
                        &self.set_image_pod,
                        &self.set_image_container,
                        &image,
                    ));
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.set_image_buffer.pop();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                // Clear input line
                self.set_image_buffer.clear();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.set_image_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                // Esc clears the search and returns to unfiltered view
                self.search_query = None;
                self.mode = Mode::Normal;
                self.search_match_indices.clear();
                self.search_active = false;
                self.search_buffer.clear();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Enter commits the search — keeps the filter active in Normal mode
                if let Mode::Search(ref input) = self.mode {
                    if !input.is_empty() {
                        self.search_query = Some(input.clone());
                    } else {
                        self.search_query = None;
                    }
                }
                self.mode = Mode::Normal;
                self.search_match_indices.clear();
                self.search_active = false;
                self.search_buffer.clear();
                // Clamp selection to visible rows after filtering
                let total = self.table_rows().len();
                if self.selected_index >= total && total > 0 {
                    self.selected_index = total - 1;
                }
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if let Mode::Search(ref mut input) = self.mode {
                    input.pop();
                    if input.is_empty() {
                        // If search becomes empty, clear the committed filter too
                        self.search_query = None;
                    }
                    self.update_search_matches();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                if let Mode::Search(ref mut input) = self.mode {
                    input.push(c);
                    self.update_search_matches();
                }
            }
            _ => {}
        }
    }

    fn update_search_matches(&mut self) {
        let query = match &self.mode {
            Mode::Search(q) => q.to_lowercase(),
            _ => return,
        };
        // Search matches only pod rows (not container sub-rows)
        self.search_match_indices = self
            .table_rows()
            .iter()
            .enumerate()
            .filter(|(_, row)| match row {
                TableRow::Pod { pod, .. } => {
                    pod.name.to_lowercase().contains(&query)
                        || pod.namespace.to_lowercase().contains(&query)
                }
                TableRow::Container { container, .. } => {
                    container.name.to_lowercase().contains(&query)
                }
            })
            .map(|(i, _)| i)
            .collect();
        self.search_match_cursor = 0;
        if let Some(&first) = self.search_match_indices.first() {
            self.selected_index = first;
        }
    }

    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => {
                self.should_quit = true;
            }
            Command::Custom(custom_cmd) => {
                if let Some(pod) = self.selected_pod_cloned() {
                    if !custom_cmd.matches(
                        &pod.namespace,
                        &pod.name,
                        self.selected_container_name().as_deref(),
                    ) {
                        self.show_toast(
                            format!(
                                "Command '{}' does not match {}/{}",
                                custom_cmd.name, pod.namespace, pod.name
                            ),
                            ToastType::Warning,
                            8,
                        );
                        return;
                    }

                    // If a container row is selected, use its name; otherwise use the first container
                    let container = self
                        .selected_container_name()
                        .or_else(|| pod.containers.first().cloned());
                    let rendered = custom_cmd.render(
                        &pod.namespace,
                        &pod.name,
                        container.as_deref(),
                        self.context_name.as_deref(),
                    );

                    // Wrap with bash -c so pipes, redirects, and shell features work
                    self.pending_shell = Some(ShellCommand::bash(&rendered));
                } else {
                    self.show_toast("No pod selected", ToastType::Warning, 8);
                }
            }
            Command::Unknown(input) => {
                self.show_toast(
                    format!("Unknown command: :{}", input),
                    ToastType::Warning,
                    8,
                );
            }
        }
    }

    pub fn clear_toast(&mut self) {
        self.toast_message = None;
        self.toast_ttl = 0;
    }

    /// Show a toast message that auto-dismisses after `ttl` ticks (~ttl × refresh_rate_ms).
    pub fn show_toast(&mut self, message: impl Into<String>, toast_type: ToastType, ttl: u8) {
        self.toast_message = Some(message.into());
        self.toast_type = toast_type;
        self.toast_ttl = ttl;
    }

    pub fn set_pods(&mut self, pods: Vec<PodInfo>) {
        self.pods = pods
            .into_iter()
            .filter(|pod| self.is_namespace_selected(&pod.namespace))
            .filter(|pod| {
                if self.namespace_pod_filters.is_empty() {
                    true
                } else {
                    self.namespace_pod_filters.iter().any(|f| f.matches(pod))
                }
            })
            .collect();
        self.apply_sort();
        // Clean up expanded_pods for pods that no longer exist
        let current_pod_names: HashSet<String> = self.pods.iter().map(|p| p.name.clone()).collect();
        self.expanded_pods
            .retain(|name| current_pod_names.contains(name));
        // Clamp selected_index to the new table row count
        let total_rows = self.table_rows().len();
        self.selected_index = self.selected_index.min(total_rows.saturating_sub(1));
    }

    /// Apply the current sort_config to self.pods.
    fn apply_sort(&mut self) {
        match &self.sort_config {
            SortConfig::NamespaceAsc => {
                self.pods
                    .sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
            }
            SortConfig::NamespaceDesc => {
                self.pods
                    .sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
                self.pods.reverse();
            }
            SortConfig::NameAsc => {
                self.pods.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortConfig::NameDesc => {
                self.pods.sort_by(|a, b| a.name.cmp(&b.name));
                self.pods.reverse();
            }
            SortConfig::AgeAsc => {
                // Parse age strings for comparison — oldest first
                self.pods.sort_by(|a, b| a.age.cmp(&b.age));
            }
            SortConfig::AgeDesc => {
                self.pods.sort_by(|a, b| a.age.cmp(&b.age));
                self.pods.reverse();
            }
            SortConfig::StatusAsc => {
                self.pods.sort_by(|a, b| a.status.cmp(&b.status));
            }
            SortConfig::StatusDesc => {
                self.pods.sort_by(|a, b| a.status.cmp(&b.status));
                self.pods.reverse();
            }
        }
    }

    /// Get a human-readable label for the current sort config.
    pub fn sort_label(&self) -> &'static str {
        match self.sort_config {
            SortConfig::NamespaceAsc => "ns↑",
            SortConfig::NamespaceDesc => "ns↓",
            SortConfig::NameAsc => "name↑",
            SortConfig::NameDesc => "name↓",
            SortConfig::AgeAsc => "age↑",
            SortConfig::AgeDesc => "age↓",
            SortConfig::StatusAsc => "status↑",
            SortConfig::StatusDesc => "status↓",
        }
    }

    /// Get the table title including sort and active filter info.
    pub fn table_title(&self) -> String {
        let sort = self.sort_label();
        match &self.mode {
            Mode::Search(q) if !q.is_empty() => format!("Pods [{}] filter:{}", sort, q),
            _ => {
                if let Some(ref q) = self.search_query {
                    format!("Pods [{}] filter:{}", sort, q)
                } else {
                    format!("Pods [{}]", sort)
                }
            }
        }
    }

    pub fn add_namespace_pod_filter(&mut self, pattern: &str) -> anyhow::Result<()> {
        let filter = NamespacePodFilter::parse(pattern)?;
        self.namespace_pod_filters.push(filter);
        Ok(())
    }

    pub fn set_available_namespaces(&mut self, ns: Vec<String>) {
        self.available_namespaces = ns;
    }

    pub fn toggle_namespace(&mut self, ns: &str) {
        if self.selected_namespaces.contains(ns) {
            self.selected_namespaces.remove(ns);
        } else {
            self.selected_namespaces.insert(ns.to_string());
        }
    }

    pub fn select_all_namespaces(&mut self) {
        self.selected_namespaces.clear();
    }

    pub fn is_namespace_selected(&self, ns: &str) -> bool {
        self.selected_namespaces.is_empty() || self.selected_namespaces.contains(ns)
    }

    /// Return namespaces that are relevant to the current view.
    /// When namespace/pod filters are active, only namespaces with matching pods are shown.
    /// When no filters are active, all available namespaces are shown.
    pub fn active_namespaces(&self) -> Vec<String> {
        if self.namespace_pod_filters.is_empty() {
            self.available_namespaces.clone()
        } else {
            let mut ns_set: Vec<String> = self
                .pods
                .iter()
                .map(|p| p.namespace.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            ns_set.sort();
            ns_set
        }
    }

    pub fn filtered_namespaces(&self) -> Vec<String> {
        let base = self.active_namespaces();
        if self.namespace_picker_search.is_empty() {
            base
        } else {
            base.iter()
                .filter(|ns| {
                    ns.to_lowercase()
                        .contains(&self.namespace_picker_search.to_lowercase())
                })
                .cloned()
                .collect()
        }
    }

    /// Filtered namespaces for the picker UI — uses the staging buffer for display.
    pub fn filtered_namespaces_for_picker(&self) -> Vec<String> {
        let base = self.active_namespaces();
        if self.namespace_picker_search.is_empty() {
            base
        } else {
            base.iter()
                .filter(|ns| {
                    ns.to_lowercase()
                        .contains(&self.namespace_picker_search.to_lowercase())
                })
                .cloned()
                .collect()
        }
    }

    /// Get the currently effective selected namespaces for the picker.
    /// Returns the staged set if we're in picker mode, otherwise the actual set.
    pub fn effective_selected_namespaces(&self) -> &HashSet<String> {
        self.staged_namespaces
            .as_ref()
            .unwrap_or(&self.selected_namespaces)
    }

    /// Get a description of available container ports for the port-forward placeholder.
    pub fn port_forward_suggestion(&self) -> String {
        // Find the pod for port forward
        let pod = self.pods.iter().find(|p| p.name == self.port_forward_pod);
        if let Some(pod) = pod {
            if let Some(ref container_name) = self.port_forward_container {
                if let Some(detail) = pod
                    .container_details
                    .iter()
                    .find(|c| &c.name == container_name)
                {
                    if !detail.ports.is_empty() {
                        let ports: Vec<String> = detail
                            .ports
                            .iter()
                            .map(|p| format!("{}:{}", p.port, p.port))
                            .collect();
                        return format!("ports: {}", ports.join(", "));
                    }
                }
            } else if let Some(detail) = pod.container_details.first() {
                if !detail.ports.is_empty() {
                    let ports: Vec<String> = detail
                        .ports
                        .iter()
                        .map(|p| format!("{}:{}", p.port, p.port))
                        .collect();
                    return format!("ports: {}", ports.join(", "));
                }
            }
        }
        "<local:remote>  e.g. 8080:80".to_string()
    }

    pub fn command_palette_query(&self) -> Option<&str> {
        match &self.mode {
            Mode::CommandPalette { query, .. } => Some(query),
            _ => None,
        }
    }

    pub fn command_palette_index(&self) -> Option<usize> {
        match &self.mode {
            Mode::CommandPalette { index, .. } => Some(*index),
            _ => None,
        }
    }

    pub fn search_input(&self) -> Option<&str> {
        match &self.mode {
            Mode::Search(input) => Some(input),
            _ => None,
        }
    }

    pub fn set_available_contexts(&mut self, contexts: Vec<String>) {
        self.available_contexts = contexts;
    }

    /// Return contexts filtered by the context picker search query.
    pub fn filtered_contexts(&self) -> Vec<String> {
        if self.context_picker_search.is_empty() {
            self.available_contexts.clone()
        } else {
            self.available_contexts
                .iter()
                .filter(|ctx| {
                    ctx.to_lowercase()
                        .contains(&self.context_picker_search.to_lowercase())
                })
                .cloned()
                .collect()
        }
    }

    /// Apply a context switch: update context_name and clear namespace/pod state
    /// so the main loop can re-create the client and refresh data.
    pub fn apply_context_switch(&mut self, new_context: String) {
        self.context_name = Some(new_context);
        self.selected_namespaces.clear();
        self.available_namespaces.clear();
        self.pods.clear();
        self.selected_index = 0;
    }
}
