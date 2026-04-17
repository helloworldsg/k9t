<div align="center">

# k9t

**A faster, simpler Kubernetes terminal UI**

[![CI](https://github.com/helloworldsg/k9t/actions/workflows/ci.yml/badge.svg)](https://github.com/helloworldsg/k9t/actions/workflows/ci.yml)
[![Release](https://github.com/helloworldsg/k9t/actions/workflows/release.yml/badge.svg)](https://github.com/helloworldsg/k9t/actions/workflows/release.yml)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

`k9t` is a terminal UI for Kubernetes that's fast to start, easy to learn, and stays out of your way. It gives you the pod-level visibility and actions you actually use — logs, shell, describe, YAML, debug, kill, restart — without the complexity of a full resource hierarchy browser.

Think of it as the essential 20% of `k9s`, built in Rust.

## Why k9t?

|                    | k9s                                       | k9t                                            |
|--------------------|-------------------------------------------|------------------------------------------------|
| Startup            | seconds                                   | instant                                        |
| Resource hierarchy | full tree (pods, deploys, nodes, events…) | pods, focused                                  |
| Custom commands    | aliases & plugins                         | `~/.config/k9t.yaml`, template variables       |
| Container picker   | dialog box                                | drill-down                                     |
| Logs               | built-in pager                            | configurable                                   |
| Built-in commands  | hardcoded                                 | all configurable in config — override anything |
| Binary             | Go, ~35MB                                 | Rust, ~4MB                                     |

## Features

- **Live pod list** — watched via Kubernetes reflector, updates in real time, sortable by namespace/name/age/status
- **Wide pod columns** — toggle `w` to show pod IP and node placement when you have the terminal width for it
- **6 color themes** — Tokyo Night, Nord, Dracula, Gruvbox, Catppuccin Mocha, Light + Monochrome (with `NO_COLOR=1`)
- **All commands configurable** — built-in commands (logs, shell, describe, yaml, debug, etc.) are template strings in YAML — override or replace any of them
- **Action dialog with type-to-filter** — type to quickly filter the actions list; no shortcut keys to remember

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
```

## Configuration

k9t loads config from the first file found (in order):

1. `~/.config/k9t.yaml`
2. `~/Library/Application Support/k9t.yaml` (macOS)

### Example `~/.config/k9t.yaml`

```yaml
theme: tokyo_night
wide_pod_columns: false
borderless: true
filters:
  - "plt/kong.*"
  - "prod/.*"

# Built-in command templates — override or replace any of these
commands_builtin:
  logs: "kubectl logs -f -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl"
  previous_logs: "kubectl logs --previous -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} | hl"
  shell: "kubectl exec -it -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh"
  describe: "kubectl describe pod -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes"
  yaml: "kubectl get pod -o yaml -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | bat --language yaml --style=changes"
  set_image: "kubectl set image pod/{{POD}} -n {{NAMESPACE}} {{CONTAINER}}={{IMAGE}} --context {{CONTEXT}}"
  port_forward: "kubectl port-forward -n {{NAMESPACE}} {{POD}} {{PORTS}} --context {{CONTEXT}}"
  debug: "kubectl debug -it {{POD}} --container={{CONTAINER}} --image=alpine --share-processes --copy-to={{POD}}-debug --context {{CONTEXT}} -- sh; kubectl delete pod {{POD}}-debug --context {{CONTEXT}}"
  list_volumes: "kubectl exec -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- sh -c 'for m in {{VOLUMES}}; do echo \"=== $m ===\"; find \"$m\" -maxdepth 3 -exec ls -l \"{}\" \\; 2>/dev/null | head -100; done' | less"

# Custom commands — appear in the action dialog and command palette
commands:
  - name: stern
    match_pattern: "prod/api-.*"
    command: "stern {{NAMESPACE}}/{{POD}} --context {{CONTEXT}} -c {{CONTAINER}}"
    description: "Tail logs with stern"
```

### Custom command fields

| Field           | Description                                                                                              |
|-----------------|----------------------------------------------------------------------------------------------------------|
| `name`          | Command name (invoked with `:name`)                                                                      |
| `match_pattern` | `namespace/pod_regex/container_regex` filter. Omit to match all.                                       |
| `command`       | Shell template with `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`, `{{VOLUMES}}`           |
| `description`   | Short help text shown in the command palette                                                             |

The `{{VOLUMES}}` variable expands to space-separated mount paths from the container's volume mounts (e.g., `/data /config /logs`).

### Match patterns

Patterns support three formats:

| Pattern              | Matches                                                |
|----------------------|--------------------------------------------------------|
| `.*/.*`              | All pods in all namespaces                             |
| `plt/api-.*`         | Pods matching `api-.*` in namespace `plt`              |
| `plt/api-.*/sidecar` | Container `sidecar` in pods matching `api-.*` in `plt` |
| `api-.*`             | Pods matching `api-.*` in any namespace                |

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
