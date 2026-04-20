#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    CommandPalette { query: String, index: usize },
    Search(String),
    Help,
    NamespacePicker,
    ContextPicker,
    ContainerPicker(ContainerPickerIntent),
    ContainerActions { query: String, index: usize },
    ConfirmAction(ConfirmContext),
    SetImageInput,
    PortForwardInput,
}

/// Why the container picker was opened — determines what happens on Enter.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerPickerIntent {
    Shell,
    Logs(bool),
    SetImage,
    PortForward,
    Debug,
}

/// An action available in the container actions dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerAction {
    Logs,
    PreviousLogs,
    Shell,
    Describe,
    Yaml,
    SetImage,
    PortForward,
    Debug,
    ListVolumes,
    ListConfigmaps,
    ListSecrets,
    ListEvents,
    ListRoutes,
    ListNetpol,
    Custom { name: String, cmd: CustomCommand },
}

impl ContainerAction {
    /// Label shown in the actions dialog.
    /// Returns true if this is a user-defined custom command.
    pub fn is_custom(&self) -> bool {
        matches!(self, ContainerAction::Custom { .. })
    }

    /// Label shown in the actions dialog.
    pub fn label(&self) -> String {
        match self {
            ContainerAction::Logs => "Logs".to_string(),
            ContainerAction::PreviousLogs => "Previous logs".to_string(),
            ContainerAction::Shell => "Shell".to_string(),
            ContainerAction::Describe => "Describe".to_string(),
            ContainerAction::Yaml => "YAML".to_string(),
            ContainerAction::SetImage => "Set image".to_string(),
            ContainerAction::PortForward => "Port forward".to_string(),
            ContainerAction::Debug => "Debug".to_string(),
            ContainerAction::ListVolumes => "View volumes".to_string(),
            ContainerAction::ListConfigmaps => "View configmaps".to_string(),
            ContainerAction::ListSecrets => "View secrets".to_string(),
            ContainerAction::ListEvents => "View events".to_string(),
            ContainerAction::ListRoutes => "View routes".to_string(),
            ContainerAction::ListNetpol => "View network policies".to_string(),
            ContainerAction::Custom { name: _, cmd } => cmd
                .description
                .clone()
                .unwrap_or_else(|| cmd.command.clone()),
        }
    }
}

use crate::config::CustomCommand;

/// Context for the confirmation dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmContext {
    KillPod { namespace: String, name: String },
    RestartDeployment { namespace: String, name: String },
}

/// Which button is focused in the confirm dialog.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ConfirmFocus {
    #[default]
    Yes,
    No,
}
