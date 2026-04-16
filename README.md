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
| Custom commands    | aliases & plugins                         | `~/.config/k9t.json`, template variables              |
| Container picker   | dialog box                                | drill-down                                            |
| Logs               | built-in pager                            | pipes through modern terminal tools, e.g. `bat`, `jq` |
| Binary             | Go, ~35MB                                 | Rust, ~4MB                                            |

## Features

- **Live pod list** — watched via Kubernetes reflector, updates in real time, sortable by namespace/name/age/status
- **Smart logs** — `l` tails logs through `less`; JSON lines auto-pretty-printed with `jq` + `bat`
- **Shell into pods** — `s` exec with automatic `sh` → `/bin/sh` → `/bin/bash` fallback
- **Describe & YAML** — `d` and `y`, paged through `less`
- **Kill & restart** — `K` and `R` with confirmation dialogs
- **Set container image** — `i` to change a container image in-place (dialog input)
- **Port forward** — `f` to set up port forwarding for a pod/container
- **Filter pods** — `/` to filter pods by name, namespace, or container name
- **Regex filters** — `--regex-namespace-pods "plt/api-.*"` from CLI
- **Multi-namespace** — `n` to pick namespaces; `a` to select all; Enter to apply, Esc to cancel
- **Hot context switching** — `x` to switch kube context without restarting
- **Command palette** — `:` to run built-in and custom commands with fuzzy search
- **6 color themes** — Tokyo Night, Nord, Dracula, Gruvbox, Catppuccin Mocha, Light + Monochrome (with `NO_COLOR=1`)
- **Custom commands** — define `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}` templates in config
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

1. `~/.config/k9t.json`
2. `~/Library/Application Support/k9t.json` (macOS)

### Example `~/.config/k9t.json`

```json
{
  "theme": "tokyo_night",
  "refresh_rate_ms": 1000,
  "borderless": true,
  "filters": ["plt/kong.*", "prod/.*"],
  "commands": [
    {
      "name": "pf",
      "match_pattern": ".*/.*",
      "command": "kubectl port-forward -n {{NAMESPACE}} {{POD}} 8080:8080 --context {{CONTEXT}}",
      "description": "Port-forward pod 8080"
    },
    {
      "name": "logs-json",
      "match_pattern": "prod/api-.*",
      "command": "kubectl logs -n {{NAMESPACE}} {{POD}} --context {{CONTEXT}} | jq .",
      "description": "Pretty-print JSON logs"
    }
  ],
  "overrides": {
    "logs": {
      "command": "stern {{NAMESPACE}}/{{POD}} --context {{CONTEXT}} -c {{CONTAINER}}"
    },
    "shell": {
      "command": "kubectl exec -it -n {{NAMESPACE}} {{POD}} -c {{CONTAINER}} --context {{CONTEXT}} -- /bin/bash"
    },
    "port-forward": {
      "command": "kubectl port-forward -n {{NAMESPACE}} {{POD}} {{PORTS}} --context {{CONTEXT}}"
    }
  }
}
```

### Custom command fields

| Field           | Description                                                                    |
|-----------------|--------------------------------------------------------------------------------|
| `name`          | Command name (invoked with `:name`)                                            |
| `match_pattern` | `namespace/pod_regex` filter. Omit to match all pods.                          |
| `command`       | Shell template with `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}` |
| `description`   | Short help text shown in the command palette                                   |

## Architecture

```
crates/k9t-core    — Kubernetes client, reflector, pod actions, config
crates/k9t-app     — Application state, key handling, modes, commands
crates/k9t-ui      — Ratatui widgets, themes, layout
crates/k9t          — Binary entry point, event loop, rendering
```

Built with [ratatui](https://github.com/ratatui/ratatui), [kube-rs](https://github.com/kube-rs/kube), and [tokio](https://github.com/tokio-rs/tokio).

## License

[MIT](LICENSE)