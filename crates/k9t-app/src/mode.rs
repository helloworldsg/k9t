#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    CommandPalette { query: String, index: usize },
    Search(String),
    Help,
    NamespacePicker,
    ContextPicker,
    ContainerPicker(ContainerPickerIntent),
    ContainerActions,
    ConfirmAction(ConfirmContext),
    SetImageInput,
    PortForwardInput,
}

/// Why the container picker was opened — determines what happens on Enter.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerPickerIntent {
    Shell,
    Logs(bool), // true = previous logs
    SetImage,
    PortForward,
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
    Custom(CustomCommand),
}

impl ContainerAction {
    /// Label shown in the actions dialog.
    pub fn label(&self) -> String {
        match self {
            ContainerAction::Logs => "Logs (kubectl logs -f)".to_string(),
            ContainerAction::PreviousLogs => "Previous logs (kubectl logs --previous)".to_string(),
            ContainerAction::Shell => "Shell (kubectl exec)".to_string(),
            ContainerAction::Describe => "Describe (kubectl describe)".to_string(),
            ContainerAction::Yaml => "YAML (kubectl get -o yaml)".to_string(),
            ContainerAction::SetImage => "Set image (kubectl set image)".to_string(),
            ContainerAction::PortForward => "Port forward (kubectl port-forward)".to_string(),
            ContainerAction::Custom(cmd) => format!(
                ":{} {}",
                cmd.name,
                cmd.description.as_deref().unwrap_or(&cmd.command)
            ),
        }
    }

    /// Shortcut key shown in the dialog.
    pub fn shortcut(&self) -> char {
        match self {
            ContainerAction::Logs => 'l',
            ContainerAction::PreviousLogs => 'p',
            ContainerAction::Shell => 's',
            ContainerAction::Describe => 'd',
            ContainerAction::Yaml => 'y',
            ContainerAction::SetImage => 'i',
            ContainerAction::PortForward => 'f',
            ContainerAction::Custom(cmd) => {
                // Use first char of command name as shortcut
                cmd.name.chars().next().unwrap_or(' ')
            }
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
