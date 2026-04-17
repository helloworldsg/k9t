use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A user-defined custom command that runs a shell command against a matched pod.
///
/// Template variables available in `command`:
/// - `{{NAMESPACE}}` — pod's namespace
/// - `{{POD}}` — pod's name
/// - `{{CONTAINER}}` — first container name (or the selected one)
/// - `{{CONTEXT}}` — current kubectl context
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct CustomCommand {
    /// Display name shown in the command bar and help screen.
    pub name: String,
    /// Match pattern in `namespace/pod_regex/container_regex` format (e.g. `plt/api-.*/sidecar`).
    /// Pod must match namespace, pod, and container regex to be applicable.
    /// Also supports `namespace/pod_regex` format (container matches all).
    /// Empty or absent means match all pods.
    #[serde(default)]
    pub match_pattern: Option<String>,
    /// Shell command template with `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`.
    pub command: String,
    /// Short description shown in help. Defaults to the command template if absent.
    #[serde(default)]
    pub description: Option<String>,
}

impl CustomCommand {
    /// Returns true when this command is applicable to the given pod/container.
    /// Pattern formats:
    /// - `namespace/pod_regex/container_regex` — all three must match
    /// - `namespace/pod_regex` — namespace and pod must match, container matches all
    /// - `pod_regex` (no `/`) — pod name must match in any namespace
    /// - Empty or absent — match everything
    pub fn matches(&self, namespace: &str, pod_name: &str, container: Option<&str>) -> bool {
        match &self.match_pattern {
            Some(pattern) if !pattern.is_empty() => {
                let parts: Vec<&str> = pattern.splitn(3, '/').collect();
                match parts.len() {
                    3 => {
                        let ns_ok = regex::Regex::new(parts[0])
                            .map(|re| re.is_match(namespace))
                            .unwrap_or(true);
                        let pod_ok = regex::Regex::new(parts[1])
                            .map(|re| re.is_match(pod_name))
                            .unwrap_or(true);
                        let container_ok = match container {
                            Some(c) => regex::Regex::new(parts[2])
                                .map(|re| re.is_match(c))
                                .unwrap_or(true),
                            None => true,
                        };
                        ns_ok && pod_ok && container_ok
                    }
                    2 => {
                        let ns_ok = regex::Regex::new(parts[0])
                            .map(|re| re.is_match(namespace))
                            .unwrap_or(true);
                        let pod_ok = regex::Regex::new(parts[1])
                            .map(|re| re.is_match(pod_name))
                            .unwrap_or(true);
                        ns_ok && pod_ok
                    }
                    _ => regex::Regex::new(pattern)
                        .map(|re| re.is_match(pod_name))
                        .unwrap_or(true),
                }
            }
            _ => true,
        }
    }

    /// Render the command template by substituting known variables.
    pub fn render(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        context: Option<&str>,
        volumes: Option<&str>,
    ) -> String {
        let mut cmd = self.command.clone();
        cmd = cmd.replace("{{NAMESPACE}}", namespace);
        cmd = cmd.replace("{{POD}}", pod_name);
        cmd = cmd.replace("{{CONTAINER}}", container.unwrap_or(""));
        cmd = cmd.replace("{{CONTEXT}}", context.unwrap_or(""));
        cmd = cmd.replace("{{VOLUMES}}", volumes.unwrap_or(""));
        cmd
    }
}

/// A command template with variable substitution.
/// Used for both built-in commands (with defaults) and user overrides.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct CommandTemplate {
    /// Shell command template with `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`,
    /// `{{IMAGE}}` (set-image only), `{{PORTS}}` (port-forward only).
    pub command: String,
}

impl CommandTemplate {
    /// Render the command template by substituting known variables.
    pub fn render(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        context: Option<&str>,
    ) -> String {
        self.render_with(namespace, pod_name, container, context, &[])
    }

    /// Render the command template with additional variable substitutions.
    ///
    /// `extra` is a list of `(placeholder, value)` pairs, e.g.
    /// `[("IMAGE", "nginx:latest"), ("PORTS", "8080:80")]`.
    pub fn render_with(
        &self,
        namespace: &str,
        pod_name: &str,
        container: Option<&str>,
        context: Option<&str>,
        extra: &[(&str, &str)],
    ) -> String {
        let mut cmd = self.command.clone();
        cmd = cmd.replace("{{NAMESPACE}}", namespace);
        cmd = cmd.replace("{{POD}}", pod_name);
        cmd = cmd.replace("{{CONTAINER}}", container.unwrap_or(""));
        cmd = cmd.replace("{{CONTEXT}}", context.unwrap_or(""));
        for (key, value) in extra {
            cmd = cmd.replace(&format!("{{{{{}}}}}", key), value);
        }
        cmd
    }
}

/// Built-in command templates. Each has a sensible default that can be overridden in config.
///
/// Template variables: `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`.
/// Additional: `{{IMAGE}}` for set-image, `{{PORTS}}` for port-forward, `{{VOLUMES}}` for list-volumes.
///
/// Example config:
/// ```yaml
/// commands:
///   logs: "kubectl logs -f -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl"
///   yaml: "kubectl get pod -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes"
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Commands {
    /// Tail logs. Default: `kubectl logs -f -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl`
    pub logs: String,
    /// Previous logs. Default: `kubectl logs --previous -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl`
    pub previous_logs: String,
    /// Shell into pod. Default: `kubectl exec -it -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh`
    /// Fallback shells (/bin/sh, /bin/bash) are tried automatically.
    pub shell: String,
    /// Describe resource. Default: `kubectl describe pod -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes`
    pub describe: String,
    /// View YAML. Default: `kubectl get pod -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes`
    pub yaml: String,
    /// Set container image. Default: `kubectl set image pod/{{POD}} -n {{NAMESPACE}} {{CONTAINER}}={{IMAGE}} --context {{CONTEXT}}`
    pub set_image: String,
    /// Port forward. Default: `kubectl port-forward -n {{NAMESPACE}} {{POD}} {{PORTS}} --context {{CONTEXT}}`
    pub port_forward: String,
    /// Debug pod. Default: `kubectl debug -it {{POD}} --container={{CONTAINER}} --image=alpine --share-processes --copy-to={{POD}}-debug --context {{CONTEXT}} -- sh; kubectl delete pod {{POD}}-debug --context {{CONTEXT}}`
    pub debug: String,
    /// List volumes. Default: lists files in all mounted volumes using `{{VOLUMES}}`
    pub list_volumes: String,
    /// List configmaps. Default: fetches and displays all ConfigMaps in namespace
    pub list_configmaps: String,
    /// List secrets. Default: fetches and displays all Secrets in namespace
    pub list_secrets: String,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            logs: "kubectl logs -f -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl".to_string(),
            previous_logs: "kubectl logs --previous -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl".to_string(),
            shell: "kubectl exec -it -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh".to_string(),
            describe: "kubectl describe pod -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes".to_string(),
            yaml: "kubectl get pod -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes".to_string(),
            set_image: "kubectl set image pod/{{POD}} -n {{NAMESPACE}} {{CONTAINER}}={{IMAGE}} --context {{CONTEXT}}".to_string(),
            port_forward: "kubectl port-forward -n {{NAMESPACE}} {{POD}} {{PORTS}} --context {{CONTEXT}}".to_string(),
            debug: "kubectl debug -it {{POD}} --container={{CONTAINER}} --image=alpine --share-processes --copy-to={{POD}}-debug --context {{CONTEXT}} -- sh; kubectl delete pod {{POD}}-debug --context {{CONTEXT}}".to_string(),
            list_volumes: "kubectl exec -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh -c 'for m in {{VOLUMES}}; do echo \"=== $m ===\"; find \"$m\" -maxdepth 3 -exec ls -l \"{}\" \\; 2>/dev/null | head -100; done' | less".to_string(),
            list_configmaps: "bash -c 'cms=$(kubectl get pod {{POD}} -n {{NAMESPACE}} --context {{CONTEXT}} -o jsonpath=\"{range .spec.volumes[*]}{.configMap.name}{\\\" \\\"}{end}{range .spec.containers[*].envFrom[*]}{.configMapRef.name}{\\\" \\\"}{end}\"); [ -n \"$cms\" ] && kubectl get cm $cms -n {{NAMESPACE}} -o yaml --context {{CONTEXT}} || echo \"No configmaps found\"' | bat --language=yaml --style=changes".to_string(),
            list_secrets: "bash -c \"secrets=\\$(kubectl get pod {{POD}} -n {{NAMESPACE}} --context {{CONTEXT}} -o jsonpath='{range .spec.volumes[*]}{.secret.secretName}{\\\" \\\"}{end}'); [ -n \\\"\\$secrets\\\" ] && for s in \\$secrets; do echo \\\"=== \\$s ===\\\" && kubectl get secret \\\"\\$s\\\" -n {{NAMESPACE}} --context {{CONTEXT}} -o json | jq -r '.data | to_entries[] | \\\"\\(.key)=\\(.value | @base64d)\\\"'; done || echo \\\"No secrets found\\\"\" | hl".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub wide_pod_columns: bool,
    #[serde(default = "default_borderless")]
    pub borderless: bool,
    #[serde(default)]
    pub layout: LayoutPreset,
    /// Namespace/pod regex filters, e.g. `["plt/kong.*", "prod/.*"]`.
    #[serde(default)]
    pub filters: Vec<String>,
    /// User-defined custom commands.
    #[serde(default)]
    pub commands: Vec<CustomCommand>,
    /// Built-in command templates (logs, shell, describe, etc.).
    #[serde(default)]
    pub commands_builtin: Commands,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            namespace: None,
            wide_pod_columns: false,
            borderless: default_borderless(),
            layout: LayoutPreset::default(),
            filters: Vec::new(),
            commands: Vec::new(),
            commands_builtin: Commands::default(),
        }
    }
}

impl Config {
    /// Load config, searching multiple locations in priority order:
    ///
    /// 1. `~/.config/k9t.yaml` (XDG-style, same location as k9s)
    /// 2. `~/Library/Application Support/k9t.yaml` (macOS native via `directories` crate)
    /// 3. `~/.config/k9t/config.yaml` (XDG YAML subdirectory)
    /// 4. `~/Library/Application Support/k9t/config.yaml` (macOS native YAML subdirectory)
    ///
    /// If none found, returns defaults.
    pub fn load() -> Result<Self> {
        for path in Self::config_candidates() {
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let config: Config = serde_yaml::from_str(&content)?;
            return Ok(config);
        }

        Ok(Config::default())
    }

    /// Save config as YAML to `~/.config/k9t.yaml` (creating the directory if needed).
    pub fn save(&self) -> Result<()> {
        let config_path = Self::xdg_config_yaml();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// All candidate config paths, searched in priority order.
    fn config_candidates() -> Vec<PathBuf> {
        vec![
            Self::xdg_config_yaml(),
            Self::native_config_yaml(),
            Self::xdg_config_subdir_yaml(),
            Self::native_config_subdir_yaml(),
        ]
    }

    fn xdg_config_yaml() -> PathBuf {
        Self::home_dir().join(".config").join("k9t.yaml")
    }

    fn xdg_config_subdir_yaml() -> PathBuf {
        Self::home_dir()
            .join(".config")
            .join("k9t")
            .join("config.yaml")
    }

    fn native_config_yaml() -> PathBuf {
        Self::native_config_dir()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("k9t.yaml")
    }

    fn native_config_subdir_yaml() -> PathBuf {
        Self::native_config_dir().join("config.yaml")
    }

    fn native_config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "k9t")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".config/k9t"))
    }

    fn home_dir() -> PathBuf {
        directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

fn default_theme() -> String {
    "tokyo_night".to_string()
}

fn default_borderless() -> bool {
    true
}

fn default_widget_visibility() -> [bool; 4] {
    [true, true, true, true]
}

fn default_view() -> String {
    "dashboard".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LayoutPreset {
    #[serde(default = "default_widget_visibility")]
    pub widget_visibility: [bool; 4],
    #[serde(default = "default_view")]
    pub default_view: String,
}

impl Default for LayoutPreset {
    fn default() -> Self {
        Self {
            widget_visibility: default_widget_visibility(),
            default_view: default_view(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let config = Config::default();
        assert_eq!(config.theme, "tokyo_night");
        assert!(config.namespace.is_none());
        assert!(!config.wide_pod_columns);
        assert!(config.borderless);
        assert!(config.filters.is_empty());
        assert!(config.commands.is_empty());
    }

    #[test]
    fn config_yaml_roundtrip() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.theme, config.theme);
    }

    #[test]
    fn custom_command_matches_all_when_no_regex() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: None,
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("default", "my-pod", None));
    }

    #[test]
    fn custom_command_matches_with_pattern() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: Some("plt/api-.*".to_string()),
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("plt", "api-deploy-1234", None));
        assert!(!cmd.matches("prod", "api-deploy-1234", None));
        assert!(!cmd.matches("plt", "web-deploy-5678", None));
    }

    #[test]
    fn custom_command_matches_pod_only() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: Some("api-.*".to_string()),
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("default", "api-1234", None));
        assert!(cmd.matches("prod", "api-5678", None));
        assert!(!cmd.matches("prod", "web-1234", None));
    }

    #[test]
    fn custom_command_matches_with_container_pattern() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: Some("plt/api-.*/sidecar".to_string()),
            command: "echo {{POD}} {{CONTAINER}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("plt", "api-deploy-1234", Some("sidecar")));
        assert!(!cmd.matches("plt", "api-deploy-1234", Some("main")));
        assert!(!cmd.matches("plt", "web-deploy-5678", Some("sidecar")));
    }

    #[test]
    fn custom_command_render_template() {
        let cmd = CustomCommand {
            name: "pf".to_string(),
            match_pattern: None,
            command:
                "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080 --context {{CONTEXT}}"
                    .to_string(),
            description: None,
        };
        let rendered = cmd.render("plt", "api-1234", Some("api"), Some("my-cluster"), None);
        assert_eq!(
            rendered,
            "kubectl port-forward -n plt api-1234 8080:8080 --context my-cluster"
        );
    }

    #[test]
    fn config_yaml_with_filters_and_commands() {
        let yaml = r#"
theme: nord
wide_pod_columns: true
borderless: false
filters:
  - "plt/kong.*"
  - "prod/.*"
commands:
  - name: port-forward-api
    match_pattern: "plt/api-.*"
    command: "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080"
    description: "Port-forward API pod"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.theme, "nord");
        assert!(config.wide_pod_columns);
        assert!(!config.borderless);
        assert_eq!(config.filters.len(), 2);
        assert_eq!(config.commands.len(), 1);
        assert_eq!(config.commands[0].name, "port-forward-api");
        assert_eq!(
            config.commands[0].match_pattern.as_deref(),
            Some("plt/api-.*")
        );
    }

    #[test]
    fn user_config_pf_command() {
        let yaml = r#"
theme: tokyo_night
borderless: true
filters:
  - ".*/.*"
commands:
  - name: pf
    match_pattern: ".*/.*"
    command: "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080 --context {{CONTEXT}}"
    description: "Port-forward API pod"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.commands.len(), 1);

        let cmd = &config.commands[0];
        assert_eq!(cmd.name, "pf");
        assert!(cmd.matches("default", "my-pod-abc", None));
        assert!(cmd.matches("production", "web-1234", None));

        let parsed = crate::command::Command::parse("pf", &config.commands);
        match parsed {
            crate::command::Command::Custom(c) => {
                assert_eq!(c.name, "pf");
                let rendered = c.render("default", "my-pod", None, Some("my-ctx"), None);
                assert_eq!(
                    rendered,
                    "kubectl port-forward -n default my-pod 8080:8080 --context my-ctx"
                );
            }
            other => panic!("Expected Command::Custom, got {:?}", other),
        }
    }

    #[test]
    fn command_parse_unknown_without_custom_commands() {
        let parsed = crate::command::Command::parse("pf", &[]);
        assert!(matches!(parsed, crate::command::Command::Unknown(_)));
    }

    #[test]
    fn config_load_actual_file() {
        match Config::load() {
            Ok(_config) => {}
            Err(e) => {
                let any_exists = Config::config_candidates().iter().any(|p| p.exists());
                if any_exists {
                    panic!("Config file exists but failed to load: {}", e);
                }
            }
        }
    }

    #[test]
    fn builtin_commands_defaults() {
        let cmd = Commands::default();
        assert!(cmd.logs.contains("hl"));
        assert!(cmd.yaml.contains("bat"));
        assert!(cmd.describe.contains("bat"));
        assert!(cmd.shell.contains("exec"));
    }

    #[test]
    fn builtin_commands_template_render() {
        let cmd = Commands::default();
        let rendered = cmd
            .logs
            .replace("{{NAMESPACE}}", "default")
            .replace("{{POD}}", "my-pod")
            .replace("{{CONTAINER}}", "my-container")
            .replace("{{CONTEXT}}", "my-cluster");
        assert!(rendered.contains("kubectl logs -f"));
        assert!(rendered.contains("default"));
    }

    #[test]
    fn builtin_commands_yaml_override() {
        let yaml = r#"
commands_builtin:
  logs: "stern {{NAMESPACE}}/{{POD}} --context {{CONTEXT}}"
  yaml: "kubectl get -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | less"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.commands_builtin.logs.contains("stern"));
        assert!(config.commands_builtin.yaml.contains("less"));
        // Non-overridden fields keep defaults
        assert!(config.commands_builtin.describe.contains("bat"));
        assert!(config.commands_builtin.shell.contains("exec"));
    }

    #[test]
    fn builtin_commands_port_forward_template() {
        let cmd = Commands::default();
        let rendered = cmd
            .port_forward
            .replace("{{NAMESPACE}}", "default")
            .replace("{{POD}}", "my-pod")
            .replace("{{PORTS}}", "8080:80")
            .replace("{{CONTEXT}}", "my-cluster");
        assert_eq!(
            rendered,
            "kubectl port-forward -n default my-pod 8080:80 --context my-cluster"
        );
    }
}
