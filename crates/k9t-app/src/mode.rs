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
    Custom(CustomCommand),
}

impl ContainerAction {
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
            ContainerAction::Custom(cmd) => format!(
                ":{} {}",
                cmd.name,
                cmd.description.as_deref().unwrap_or(&cmd.command)
            ),
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
