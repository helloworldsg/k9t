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
    /// Match pattern in `namespace/pod_regex` format (e.g. `plt/api-.*`).
    /// Pod must match both namespace and pod regex to be applicable.
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
    /// Returns true when this command is applicable to the given pod.
    pub fn matches(&self, namespace: &str, pod_name: &str) -> bool {
        match &self.match_pattern {
            Some(pattern) if !pattern.is_empty() => {
                if let Some((ns_re, pod_re)) = pattern.split_once('/') {
                    let ns_ok = regex::Regex::new(ns_re)
                        .map(|re| re.is_match(namespace))
                        .unwrap_or(true);
                    let pod_ok = regex::Regex::new(pod_re)
                        .map(|re| re.is_match(pod_name))
                        .unwrap_or(true);
                    ns_ok && pod_ok
                } else {
                    // No '/' — treat entire pattern as pod-only match (any namespace)
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(pod_name))
                        .unwrap_or(true)
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
    ) -> String {
        let mut cmd = self.command.clone();
        cmd = cmd.replace("{{NAMESPACE}}", namespace);
        cmd = cmd.replace("{{POD}}", pod_name);
        cmd = cmd.replace("{{CONTAINER}}", container.unwrap_or(""));
        cmd = cmd.replace("{{CONTEXT}}", context.unwrap_or(""));
        cmd
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_refresh_rate_ms")]
    pub refresh_rate_ms: u64,
    #[serde(default)]
    pub namespace: Option<String>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            refresh_rate_ms: default_refresh_rate_ms(),
            namespace: None,
            borderless: default_borderless(),
            layout: LayoutPreset::default(),
            filters: Vec::new(),
            commands: Vec::new(),
        }
    }
}

impl Config {
    /// Load config, searching multiple locations in priority order:
    ///
    /// 1. `~/.config/k9t.json` (XDG-style, same location as k9s)
    /// 2. `~/Library/Application Support/k9t.json` (macOS native via `directories` crate)
    /// 3. `~/.config/k9t/config.toml` (legacy TOML)
    /// 4. `~/Library/Application Support/k9t/config.toml` (legacy TOML, macOS)
    ///
    /// If none found, returns defaults.
    pub fn load() -> Result<Self> {
        // Search all candidate paths in priority order
        for path in Self::config_candidates() {
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            if path.extension().is_some_and(|ext| ext == "json") {
                let config: Config = serde_json::from_str(&content)?;
                return Ok(config);
            } else {
                let config: Config = toml::from_str(&content)?;
                return Ok(config);
            }
        }

        Ok(Config::default())
    }

    /// Save config as JSON to `~/.config/k9t.json` (creating the directory if needed).
    pub fn save(&self) -> Result<()> {
        let config_path = Self::xdg_config_json();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// All candidate config paths, searched in priority order.
    fn config_candidates() -> Vec<PathBuf> {
        vec![
            Self::xdg_config_json(),
            Self::native_config_json(),
            Self::xdg_config_toml(),
            Self::native_config_toml(),
        ]
    }

    /// `~/.config/k9t.json` — XDG-style path (matches k9s convention, works on all platforms).
    fn xdg_config_json() -> PathBuf {
        Self::home_dir().join(".config").join("k9t.json")
    }

    /// `~/.config/k9t/config.toml` — legacy XDG TOML path.
    fn xdg_config_toml() -> PathBuf {
        Self::home_dir()
            .join(".config")
            .join("k9t")
            .join("config.toml")
    }

    /// `~/Library/Application Support/k9t.json` — macOS native path via `directories` crate.
    fn native_config_json() -> PathBuf {
        Self::native_config_dir()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("k9t.json")
    }

    /// `~/Library/Application Support/k9t/config.toml` — legacy macOS native TOML path.
    fn native_config_toml() -> PathBuf {
        Self::native_config_dir().join("config.toml")
    }

    /// Native config directory from the `directories` crate.
    /// On macOS: `~/Library/Application Support/k9t/`
    /// On Linux: `~/.config/k9t/`
    fn native_config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "k9t")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".config/k9t"))
    }

    /// Resolve home directory, falling back to `/tmp` if unset.
    fn home_dir() -> PathBuf {
        directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

fn default_theme() -> String {
    "tokyo_night".to_string()
}

fn default_refresh_rate_ms() -> u64 {
    1000
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let config = Config::default();
        assert_eq!(config.theme, "tokyo_night");
        assert_eq!(config.refresh_rate_ms, 1000);
        assert!(config.namespace.is_none());
        assert!(config.borderless);
        assert!(config.filters.is_empty());
        assert!(config.commands.is_empty());
    }

    #[test]
    fn config_json_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, config.theme);
        assert_eq!(parsed.refresh_rate_ms, config.refresh_rate_ms);
    }

    #[test]
    fn custom_command_matches_all_when_no_regex() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: None,
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("default", "my-pod"));
    }

    #[test]
    fn custom_command_matches_with_pattern() {
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: Some("plt/api-.*".to_string()),
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("plt", "api-deploy-1234"));
        assert!(!cmd.matches("prod", "api-deploy-1234"));
        assert!(!cmd.matches("plt", "web-deploy-5678"));
    }

    #[test]
    fn custom_command_matches_pod_only() {
        // Pattern without '/' matches pod name in any namespace
        let cmd = CustomCommand {
            name: "test".to_string(),
            match_pattern: Some("api-.*".to_string()),
            command: "echo {{POD}}".to_string(),
            description: None,
        };
        assert!(cmd.matches("default", "api-1234"));
        assert!(cmd.matches("prod", "api-5678"));
        assert!(!cmd.matches("prod", "web-1234"));
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
        let rendered = cmd.render("plt", "api-1234", Some("api"), Some("my-cluster"));
        assert_eq!(
            rendered,
            "kubectl port-forward -n plt api-1234 8080:8080 --context my-cluster"
        );
    }

    #[test]
    fn config_json_with_filters_and_commands() {
        let json = r#"{
            "theme": "nord",
            "refresh_rate_ms": 500,
            "borderless": false,
            "filters": ["plt/kong.*", "prod/.*"],
            "commands": [
                {
                    "name": "port-forward-api",
                    "match_pattern": "plt/api-.*",
                    "command": "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080",
                    "description": "Port-forward API pod"
                }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, "nord");
        assert_eq!(config.refresh_rate_ms, 500);
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
        // Exact reproduction of user's ~/.config/k9t.json
        let json = r#"{
            "theme": "tokyo_night",
            "refresh_rate_ms": 1000,
            "borderless": true,
            "filters": [".*/.*"],
            "commands": [
                {
                    "name": "pf",
                    "match_pattern": ".*/.*",
                    "command": "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080 --context {{CONTEXT}}",
                    "description": "Port-forward API pod"
                }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.commands.len(), 1);

        let cmd = &config.commands[0];
        assert_eq!(cmd.name, "pf");
        assert!(cmd.matches("default", "my-pod-abc"));
        assert!(cmd.matches("production", "web-1234"));

        // Verify Command::parse finds it
        let parsed = crate::command::Command::parse("pf", &config.commands);
        match parsed {
            crate::command::Command::Custom(c) => {
                assert_eq!(c.name, "pf");
                let rendered = c.render("default", "my-pod", None, Some("my-ctx"));
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
        // When custom_commands is empty, "pf" should be unknown
        let parsed = crate::command::Command::parse("pf", &[]);
        assert!(matches!(parsed, crate::command::Command::Unknown(_)));
    }

    #[test]
    fn config_load_actual_file() {
        // Try loading the actual config file to verify path resolution
        match Config::load() {
            Ok(config) => {
                // If any candidate file exists, commands should be non-empty (assuming the user's file)
                let any_exists = Config::config_candidates().iter().any(|p| p.exists());
                if any_exists {
                    assert!(
                        !config.commands.is_empty(),
                        "Config file exists but commands is empty — JSON may be missing 'commands' key"
                    );
                }
            }
            Err(e) => {
                // If config file(s) exist but failed to load, that's a bug
                let any_exists = Config::config_candidates().iter().any(|p| p.exists());
                if any_exists {
                    panic!("Config file exists but failed to load: {}", e);
                }
            }
        }
    }
}
