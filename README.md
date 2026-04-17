<div align="center">

# k9t

**A faster, simpler Kubernetes terminal UI**

[![CI](https://github.com/helloworldsg/k9t/actions/workflows/ci.yml/badge.svg)](https://github.com/helloworldsg/k9t/actions/workflows/ci.yml)
[![Release](https://github.com/helloworldsg/k9t/actions/workflows/release.yml/badge.svg)](https://github.com/helloworldsg/k9t/actions/workflows/release.yml)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

`k9t` is a terminal UI for Kubernetes that's fast to start, easy to learn, and stays out of your way. It gives you the pod-level visibility and actions you actually use — logs, shell, describe, YAML, kill, restart — without the complexity of a full resource hierarchy browser.

Think of it as the essential 20% of `k9s`, built in Rust.

## Why k9t?

|                    | k9s                                       | k9t                                                   |
|--------------------|-------------------------------------------|-------------------------------------------------------|
| Startup            | seconds                                   | instant                                               |
| Resource hierarchy | full tree (pods, deploys, nodes, events…) | pods, focused                                         |
| Custom commands    | aliases & plugins                         | `~/.config/k9t.yaml`, template variables              |
| Container picker   | dialog box                                | drill-down                                            |
| Logs               | built-in pager                            | pipes through `hl`, configurable via YAML             |
| Built-in commands  | hardcoded                                 | all templated in config — override anything            |
| Binary             | Go, ~35MB                                 | Rust, ~4MB                                            |

## Features

- **Live pod list** — watched via Kubernetes reflector, updates in real time, sortable by namespace/name/age/status
- **Init container visibility** — init containers shown with `Ⓘ` indicator; completed init containers styled as muted (not errors)
- **Smart logs** — `l` tails logs piped through `hl` for highlighting; override with any command in config
- **Shell into pods** — `s` exec into containers; command template is configurable
- **Describe & YAML** — `d` and `y`, piped through `bat --language yaml --style=changes` for syntax highlighting
- **Kill & restart** — `K` and `R` with confirmation dialogs
- **Set container image** — `i` to change a container image in-place; pre-filled with current image for easy editing
- **Port forward** — `f` to set up port forwarding for a pod/container
- **Filter pods** — `/` to filter pods by name, namespace, or container name
- **Regex filters** — `--regex-namespace-pods "plt/api-.*"` from CLI
- **Multi-namespace** — `n` to pick namespaces; `a` to select all; Enter to apply, Esc to cancel
- **Hot context switching** — `x` to switch kube context without restarting
- **Command palette** — `:` to run built-in and custom commands with fuzzy search
- **6 color themes** — Tokyo Night, Nord, Dracula, Gruvbox, Catppuccin Mocha, Light + Monochrome (with `NO_COLOR=1`)
- **Custom commands** — define `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}` templates in config
- **All commands configurable** — built-in commands (logs, shell, describe, yaml, etc.) are template strings in YAML — override or replace any of them
- **Action dialog with type-to-filter** — type to quickly filter the actions list; no shortcut keys to remember
- **Toast notifications** — success/error feedback for async operations

## Install

### Homebrew (macOS / Linux)

```bash
brew tap helloworldsg/tap
brew install k9t
```

### Pre-built binaries

Download from the [latest release](https://github.com/helloworldsg/k9t/releases):

### From source (requires Rust 1.88+)

```bash
git clone https://github.com/helloworldsg/k9t.git
cd k9t
cargo build --release
sudo mv target/release/k9t /usr/local/bin/
```

## Usage

```bash
# Connect to current context, all namespaces
k9t

# Connect to a specific context
k9t --context my-production

# Start in a specific namespace
k9t --namespace monitoring

# Filter pods by regex (namespace/pod_pattern)
k9t --regex-namespace-pods "plt/api-.*" --regex-namespace-pods "prod/.*"

# Use a specific kubeconfig
k9t --kubeconfig /path/to/kubeconfig

# All namespaces (explicit)
k9t --all-namespaces
```

## Configuration

k9t loads config from the first file found (in order):

1. `~/.config/k9t.yaml`
2. `~/Library/Application Support/k9t.yaml` (macOS)
3. `~/.config/k9t/config.yaml`
4. `~/Library/Application Support/k9t/config.yaml` (macOS)

### Example `~/.config/k9t.yaml`

```yaml
theme: tokyo_night
refresh_rate_ms: 1000
borderless: true
filters:
  - "plt/kong.*"
  - "prod/.*"

# Built-in command templates — override or replace any of these
commands_builtin:
  logs: "kubectl logs -f -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | hl"
  previous_logs: "kubectl logs --previous -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | hl"
  shell: "kubectl exec -it -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh"
  describe: "kubectl describe -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes"
  yaml: "kubectl get -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes"
  set_image: "kubectl set image pod/{{POD}} -n {{NAMESPACE}} {{CONTAINER}}={{IMAGE}} --context {{CONTEXT}}"
  port_forward: "kubectl port-forward -n {{NAMESPACE}} {{POD}} {{PORTS}} --context {{CONTEXT}}"

# Custom commands — appear in the action dialog and command palette
commands:
  - name: stern
    match_pattern: "prod/api-.*"
    command: "stern {{NAMESPACE}}/{{POD}} --context {{CONTEXT}} -c {{CONTAINER}}"
    description: "Tail logs with stern"
```

### Custom command fields

| Field           | Description                                                                                         |
|-----------------|-----------------------------------------------------------------------------------------------------|
| `name`          | Command name (invoked with `:name`)                                                                 |
| `match_pattern` | `namespace/pod_regex/container_regex` filter. Omit to match all.                                   |
| `command`       | Shell template with `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`                      |
| `description`   | Short help text shown in the command palette                                                        |

### Match patterns

Patterns support three formats:

| Pattern                    | Matches                                            |
|----------------------------|----------------------------------------------------|
| `.*/.*`                    | All pods in all namespaces                         |
| `plt/api-.*`              | Pods matching `api-.*` in namespace `plt`          |
| `plt/api-.*/sidecar`      | Container `sidecar` in pods matching `api-.*` in `plt` |
| `api-.*`                   | Pods matching `api-.*` in any namespace            |

All pattern parts are regex. When a container row is selected, container matching applies.

## Architecture

```
crates/k9t-core    — Kubernetes client, reflector, pod actions, resource types
crates/k9t-app     — Application state, key handling, modes, commands, config
crates/k9t-ui      — Ratatui widgets, themes, layout
crates/k9t          — Binary entry point, event loop, rendering
```

Built with [ratatui](https://github.com/ratatrat/ratatui), [kube-rs](https://github.com/kube-rs/kube), and [tokio](https://github.com/tokio-rs/tokio).

## License

[MIT](LICENSE)