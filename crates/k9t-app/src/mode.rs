#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    CommandPalette { query: String, index: usize },
    Search(String),
    Help,
    NamespacePicker,
    ContextPicker,
    ContainerPicker(ContainerPickerIntent),
    ConfirmAction(ConfirmContext),
    SetImageInput,
}

/// Why the container picker was opened — determines what happens on Enter.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerPickerIntent {
    Shell,
    Logs(bool), // true = previous logs
    SetImage,
}

/// Context for the confirmation dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmContext {
    KillPod { namespace: String, name: String },
    RestartDeployment { namespace: String, name: String },
}
